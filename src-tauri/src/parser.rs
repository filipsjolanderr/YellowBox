use crate::models::{MemoryItem, ProcessingState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Debug, Deserialize, Serialize)]
pub struct RawSnapchatMemory {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Media Type")]
    pub media_type: String,
    #[serde(rename = "Location")]
    pub location: Option<Value>,
    #[serde(rename = "Download Link")]
    pub download_link: Option<String>,
    #[serde(rename = "Media Download Url")]
    pub media_download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SnapchatExport {
    #[serde(rename = "Saved Media")]
    pub saved_media: Vec<RawSnapchatMemory>,
}

const SEGMENT_GAP_SECONDS: i64 = 10;

pub fn parse_memories_json(json_content: &str) -> Vec<MemoryItem> {
    let export: SnapchatExport = match serde_json::from_str(json_content) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse JSON: {}", e);
            return Vec::new();
        }
    };

    let mut video_entries = Vec::new();
    let mut image_entries = Vec::new();

    for mem in export.saved_media {
        let link = mem.download_link.or(mem.media_download_url);
        let link = match link {
            Some(l) if !l.is_empty() => l,
            _ => continue,
        };

        let url = match Url::parse(&link) {
            Ok(u) => u,
            Err(_) => continue,
        };

        // Try to extract ID from multiple sources:
        // 1. Query params (most common for web download links)
        // 2. URL path segments (common for direct CDN links)
        let mut candidate_ids: Vec<String> = Vec::new();
        
        // Extract IDs from query parameters in preferred order
        for key in &["mid", "media_id", "sid"] {
            if let Some(val) = url.query_pairs().find(|(k, _)| k == *key).map(|(_, v)| v.into_owned()) {
                if !val.is_empty() && val != "unknown-id" {
                    let cleaned = val.trim().to_string();
                    if !candidate_ids.contains(&cleaned) {
                        candidate_ids.push(cleaned);
                    }
                }
            }
        }

        // Fallback/Supplement: look for UUID-like segment in path
        if let Some(mut segments) = url.path_segments() {
            // SNAPCHAT UUID segments are usually 32-38 chars and contain at least one hyphen
            if let Some(seg) = segments.find(|s| (s.len() >= 32 && s.len() <= 38) && s.contains('-')) {
                let cleaned = seg.trim().to_string();
                if !candidate_ids.contains(&cleaned) {
                    candidate_ids.push(cleaned);
                }
            }
        }

        if candidate_ids.is_empty() {
            tracing::debug!(url = %link, "Failed to extract any ID from URL");
            continue;
        }

        // The first ID in our prioritized list becomes the primary ID
        let id = candidate_ids[0].clone();

        let location = normalize_location(mem.location);
        let media_type = mem.media_type.clone();
        let date_str = mem.date.clone();

        let ts = crate::metadata::parse_date_flexible(&date_str)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        if media_type.eq_ignore_ascii_case("Video") {
            video_entries.push(VideoEntry {
                id,
                candidate_ids: candidate_ids.clone(),
                download_url: link,
                location,
                date_str,
                ts,
            });
        } else {
            image_entries.push(MemoryItem {
                id,
                segment_ids: None,
                candidate_ids: Some(candidate_ids),
                download_url: link,
                original_date: date_str,
                location,
                state: ProcessingState::Pending,
                error_message: None,
                extension: None,
                has_overlay: false,
                has_thumbnail: false,
                media_type: "Image".to_string(),
            });
        }
    }

    // Sort videos by timestamp for grouping
    video_entries.sort_by_key(|v| v.ts);

    let mut result = Vec::new();
    let mut current_group: Vec<VideoEntry> = Vec::new();

    for v in video_entries {
        if let Some(prev) = current_group.last() {
            if v.ts - prev.ts <= SEGMENT_GAP_SECONDS {
                current_group.push(v);
            } else {
                result.push(fold_video_group(current_group));
                current_group = vec![v];
            }
        } else {
            current_group.push(v);
        }
    }

    if !current_group.is_empty() {
        result.push(fold_video_group(current_group));
    }

    // Add images
    result.extend(image_entries);

    result
}

struct VideoEntry {
    id: String,
    candidate_ids: Vec<String>,
    download_url: String,
    location: Option<String>,
    date_str: String,
    ts: i64,
}

fn fold_video_group(group: Vec<VideoEntry>) -> MemoryItem {
    let first = &group[0];
    
    // PHYSICAL segments: one primary ID per physical segment entry
    let segment_ids = if group.len() > 1 {
        Some(group.iter().map(|v| v.id.clone()).collect())
    } else {
        None
    };

    // ALL candidate aliases from all segments for the ZIP indexer
    let mut candidate_ids = Vec::new();
    for v in &group {
        for cid in &v.candidate_ids {
            if !candidate_ids.contains(cid) {
                candidate_ids.push(cid.clone());
            }
        }
    }

    MemoryItem {
        id: first.id.clone(),
        segment_ids,
        candidate_ids: Some(candidate_ids),
        download_url: first.download_url.clone(),
        original_date: first.date_str.clone(),
        location: first.location.clone(),
        state: ProcessingState::Pending,
        error_message: None,
        extension: None,
        has_overlay: false,
        has_thumbnail: false,
        media_type: "Video".to_string(),
    }
}

fn normalize_location(val: Option<Value>) -> Option<String> {
    let val = val?;
    
    if let Some(s) = val.as_str() {
        let mut s = s.trim();
        let prefix = "Latitude, Longitude: ";
        if s.starts_with(prefix) {
            s = &s[prefix.len()..];
        }
        return if s.is_empty() { None } else { Some(s.to_string()) };
    }

    if let Some(arr) = val.as_array() {
        if arr.len() >= 2 {
            if let (Some(lat), Some(lon)) = (arr[0].as_f64(), arr[1].as_f64()) {
                return Some(format!("{}, {}", lat, lon));
            }
        }
    }

    if let Some(obj) = val.as_object() {
        let lat = obj.get("latitude").or(obj.get("Latitude")).and_then(|v| v.as_f64());
        let lon = obj.get("longitude").or(obj.get("Longitude")).and_then(|v| v.as_f64());
        if let (Some(lat), Some(lon)) = (lat, lon) {
            return Some(format!("{}, {}", lat, lon));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_video_group_aliases_not_segments() {
        // Test: Single segment with multiple candidate IDs
        let entry = VideoEntry {
            id: "MID1".to_string(),
            candidate_ids: vec!["MID1".to_string(), "SID1".to_string()],
            download_url: "url".to_string(),
            location: None,
            date_str: "2022-01-01 12:00:00".to_string(),
            ts: 123456789,
        };
        let item = fold_video_group(vec![entry]);
        
        // Should NOT have multiple segment IDs (as it's only one physical segment)
        assert!(item.segment_ids.is_none(), "Single physical segment should have None segment_ids");
        assert_eq!(item.id, "MID1");
        
        // Candidates should still be recorded for the indexer
        let candidates = item.candidate_ids.unwrap();
        assert!(candidates.contains(&"MID1".to_string()));
        assert!(candidates.contains(&"SID1".to_string()));
    }

    #[test]
    fn test_fold_video_group_multiple_physical_segments() {
        // Test: Two distinct physical segment entries
        let entry1 = VideoEntry {
            id: "MID1".to_string(),
            candidate_ids: vec!["MID1".to_string()],
            download_url: "url1".to_string(),
            location: None,
            date_str: "2022-01-01 12:00:00".to_string(),
            ts: 123456789,
        };
        let entry2 = VideoEntry {
            id: "MID2".to_string(),
            candidate_ids: vec!["MID2".to_string()],
            download_url: "url2".to_string(),
            location: None,
            date_str: "2022-01-01 12:00:10".to_string(),
            ts: 123456799,
        };
        let item = fold_video_group(vec![entry1, entry2]);
        
        // Should have 2 physical segments
        let segments = item.segment_ids.expect("Should have segment_ids");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], "MID1");
        assert_eq!(segments[1], "MID2");
        
        // Candidates should cover all
        let candidates = item.candidate_ids.unwrap();
        assert!(candidates.contains(&"MID1".to_string()));
        assert!(candidates.contains(&"MID2".to_string()));
    }
}
