#![cfg(feature = "e2e_mydata")]

//! End-to-end test using the real mydata~1771881423240.zip Snapchat export.
//! Verifies: JSON extraction, index building, extraction, combine, and output.

use std::path::Path;
use yellowbox_lib::db::{DbManager, MemoryRepository};
use yellowbox_lib::fs;
use yellowbox_lib::metadata;
use yellowbox_lib::models::{MemoryItem, ProcessingState};
use yellowbox_lib::pipeline::{self, build_main_media_zip_index, build_overlay_zip_index};
use yellowbox_lib::{combiner, extractor};

fn test_zip_path() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let root = Path::new(&manifest_dir).parent().unwrap_or(Path::new(&manifest_dir));
    root.join("test_resources").join("mydata~1771881423240.zip")
}

fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .try_init();
}

/// Parse Snapchat date string to milliseconds since epoch. Tries common formats.
fn parse_date_to_ms(s: &str) -> i64 {
    if s.is_empty() {
        return 0;
    }
    // RFC3339: 2020-04-28T06:32:52Z
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return dt.timestamp_millis();
    }
    // 2020-04-28 06:32:52 UTC
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S UTC") {
        return dt.timestamp_millis();
    }
    // Naive: 2020-04-28 06:32:52 (assume UTC)
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return naive.and_utc().timestamp_millis();
    }
    // Apr 28, 2020, 06:32
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%b %d, %Y, %H:%M") {
        return dt.timestamp_millis();
    }
    0
}

/// Parse Saved Media from memories_history.json to get memory items (matching frontend parser logic).
/// Groups videos within 10 seconds of each other (Snapchat segments); includes all images.
fn parse_memories_from_json(json: &str) -> Vec<MemoryItem> {
    let payload: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    let saved = payload
        .get("Saved Media")
        .and_then(|v| v.as_array())
        .expect("Saved Media array");

    #[derive(Clone)]
    struct VideoSegment {
        id: String,
        download_url: String,
        location: Option<String>,
        date_str: String,
        ts_ms: i64,
    }

    const SEGMENT_GAP_MS: i64 = 10_000; // solo <10s, combine >10s (Snapchat 10-sec segments)

    let mut video_entries: Vec<VideoSegment> = Vec::new();
    let mut images: Vec<(String, String, String, Option<String>)> = Vec::new();

    for entry in saved {
        let download_link = entry
            .get("Download Link")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("Media Download Url").and_then(|v| v.as_str()))
            .unwrap_or("");
        if download_link.is_empty() {
            continue;
        }
        let url = match url::Url::parse(download_link) {
            Ok(u) => u,
            Err(_) => continue,
        };
        let id = url
            .query_pairs()
            .find(|(k, _)| k == "mid" || k == "sid")
            .map(|(_, v)| v.to_string());
        let Some(id) = id else {
            continue;
        };
        if id == "unknown" || id.is_empty() {
            continue;
        }
        let date_str = entry.get("Date").and_then(|v| v.as_str()).unwrap_or("");
        let ts_ms = parse_date_to_ms(date_str);
        let media_url = entry
            .get("Media Download Url")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("Download Link").and_then(|v| v.as_str()))
            .unwrap_or(download_link);
        let location = entry
            .get("Location")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let media_type = entry
            .get("Media Type")
            .and_then(|v| v.as_str())
            .unwrap_or("Image");

        if media_type == "Video" {
            video_entries.push(VideoSegment {
                id: id.clone(),
                download_url: media_url.to_string(),
                location: location.clone(),
                date_str: date_str.to_string(),
                ts_ms,
            });
        } else {
            images.push((id, media_url.to_string(), date_str.to_string(), location));
        }
    }

    // Sort by timestamp, group consecutive videos within SEGMENT_GAP_MS
    video_entries.sort_by_key(|v| v.ts_ms);
    let mut video_groups: Vec<Vec<VideoSegment>> = Vec::new();
    let mut current: Vec<VideoSegment> = Vec::new();
    for v in video_entries {
        let prev_ts = current.last().map(|p| p.ts_ms).unwrap_or(i64::MIN);
        if current.is_empty() || v.ts_ms.saturating_sub(prev_ts) <= SEGMENT_GAP_MS {
            current.push(v);
        } else {
            video_groups.push(std::mem::take(&mut current));
            current.push(v);
        }
    }
    if !current.is_empty() {
        video_groups.push(current);
    }

    let mut items = Vec::new();

    for segments in video_groups {
        let segment_ids: Vec<String> = segments.iter().map(|s| s.id.clone()).collect();
        let primary = &segments[0];
        items.push(MemoryItem {
            id: primary.id.clone(),
            segment_ids: if segment_ids.len() > 1 {
                Some(segment_ids)
            } else {
                None
            },
            download_url: primary.download_url.clone(),
            original_date: primary.date_str.clone(),
            location: primary.location.clone(),
            state: ProcessingState::Pending,
            error_message: None,
            extension: None,
            has_overlay: false,
            has_thumbnail: false,
            media_type: "Video".to_string(),
        });
    }

    // Include all images (no overlay filtering - was causing missing pictures)
    for (id, download_url, original_date, location) in images {
        items.push(MemoryItem {
            id,
            segment_ids: None,
            download_url,
            original_date,
            location,
            state: ProcessingState::Pending,
            error_message: None,
            extension: None,
            has_overlay: false,
            has_thumbnail: false,
            media_type: "Image".to_string(),
        });
    }

    items
}

#[test]
fn test_mydata_zip_exists() {
    let zip_path = test_zip_path();
    assert!(
        zip_path.exists(),
        "Test ZIP not found at {}. Run from project root.",
        zip_path.display()
    );
}

#[test]
fn test_extract_json_from_mydata_zip() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _memories_dir) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let content_str = json_content.as_deref().unwrap_or("");
    assert!(!content_str.is_empty());
    assert!(content_str.contains("Saved Media"));
}

#[test]
fn test_parse_memories_from_mydata_json() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let items = parse_memories_from_json(json_content.as_deref().unwrap_or(""));
    assert!(!items.is_empty(), "Should have parsed memories");
}

/// Collect all IDs for index building (includes segment IDs for split videos, matching pipeline).
fn collect_ids_for_index(items: &[MemoryItem]) -> Vec<String> {
    items
        .iter()
        .flat_map(|i| {
            i.segment_ids
                .as_ref()
                .cloned()
                .unwrap_or_else(|| vec![i.id.clone()])
        })
        .collect()
}

#[test]
fn test_build_indexes_from_mydata_zip() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let items = parse_memories_from_json(json_content.as_deref().unwrap_or(""));
    let ids = collect_ids_for_index(&items);

    let main_index = build_main_media_zip_index(&[zip_path.clone()], &ids).expect("main index");
    let overlay_items: Vec<pipeline::OverlayItemRef> = items
        .iter()
        .map(|i| pipeline::OverlayItemRef {
            id: i.id.clone(),
            segment_ids: i.segment_ids.clone(),
        })
        .collect();
    let overlay_index = build_overlay_zip_index(&[zip_path.clone()], &overlay_items).expect("overlay index");

    assert!(!main_index.is_empty(), "main index should have entries");
    // At least one item with overlay (e.g. 24801068-6038-4BC2-830C-051CFEEF4F6D)
    let has_overlay = overlay_index.len() > 0;
    assert!(has_overlay, "overlay index should have at least one entry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_extract_and_combine_single_image_from_mydata() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let items = parse_memories_from_json(json_content.as_deref().unwrap_or(""));
    let ids = collect_ids_for_index(&items);

    let main_index = build_main_media_zip_index(&[zip_path.clone()], &ids).expect("main index");
    let overlay_items: Vec<pipeline::OverlayItemRef> = items
        .iter()
        .map(|i| pipeline::OverlayItemRef {
            id: i.id.clone(),
            segment_ids: i.segment_ids.clone(),
        })
        .collect();
    let overlay_index = build_overlay_zip_index(&[zip_path.clone()], &overlay_items).ok();

    // Pick an image with overlay: 24801068-6038-4BC2-830C-051CFEEF4F6D
    let target_id = items
        .iter()
        .find(|i| {
            let key = format!("{}|{}", metadata::get_clean_date_prefix(&i.original_date), i.id);
            i.id.contains("24801068") && overlay_index.as_ref().map_or(false, |o| o.contains_key(&key))
        })
        .or_else(|| items.iter().find(|i| i.media_type == "Image"))
        .expect("need at least one image");

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let dest_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&dest_dir).expect("create output");

    // Extract from export ZIP
    let raw_path = pipeline::extract_from_export_zip(
        &zip_path,
        &target_id.id,
        &dest_dir,
        &main_index,
        overlay_index.as_ref(),
        Some(&target_id.original_date),
    )
    .expect("extract from export zip");

    assert!(raw_path.exists(), "raw file should exist");

    // Run extractor (copies -raw to -main, finds -overlay)
    let (main_path, overlay_path) =
        extractor::extract_memory(&raw_path, &target_id.id, &dest_dir).await.expect("extract_memory");

    assert!(main_path.exists(), "main file should exist");

    // Combine if we have overlay
    let clean_name = metadata::generate_clean_filename(
        &target_id.original_date,
        &target_id.id,
        main_path.extension().and_then(|e| e.to_str()).unwrap_or("jpg"),
    );
    let combined_dest = dest_dir.join(&clean_name);

    if let Some(ref overlay) = overlay_path {
        if overlay.exists() {
            combiner::combine_image(&main_path, overlay, &combined_dest)
                .await
                .expect("combine image");
        } else {
            std::fs::copy(&main_path, &combined_dest).expect("copy main");
        }
    } else {
        std::fs::copy(&main_path, &combined_dest).expect("copy main");
    }

    assert!(combined_dest.exists(), "combined file should exist");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_extract_and_combine_video_with_overlay_from_mydata() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let items = parse_memories_from_json(json_content.as_deref().unwrap_or(""));
    let ids = collect_ids_for_index(&items);

    let main_index = build_main_media_zip_index(&[zip_path.clone()], &ids).expect("main index");
    let overlay_items: Vec<pipeline::OverlayItemRef> = items
        .iter()
        .map(|i| pipeline::OverlayItemRef {
            id: i.id.clone(),
            segment_ids: i.segment_ids.clone(),
        })
        .collect();
    let overlay_index = build_overlay_zip_index(&[zip_path.clone()], &overlay_items).ok();

    // Pick a video with overlay: 2D98014C-444F-4D50-99BE-940E4E883393 has main.mp4 + overlay.png
    let target_id = items
        .iter()
        .find(|i| {
            let key = format!("{}|{}", metadata::get_clean_date_prefix(&i.original_date), i.id);
            i.media_type == "Video"
                && overlay_index.as_ref().map_or(false, |o| o.contains_key(&key))
        })
        .or_else(|| items.iter().find(|i| i.media_type == "Video"))
        .expect("need at least one video");

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let dest_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&dest_dir).expect("create output");

    // Extract from export ZIP
    let raw_path = pipeline::extract_from_export_zip(
        &zip_path,
        &target_id.id,
        &dest_dir,
        &main_index,
        overlay_index.as_ref(),
        Some(&target_id.original_date),
    )
    .expect("extract from export zip");

    assert!(raw_path.exists(), "raw video file should exist");

    // Run extractor (copies -raw to -main, finds -overlay)
    let (main_path, overlay_path) =
        extractor::extract_memory(&raw_path, &target_id.id, &dest_dir).await.expect("extract_memory");

    assert!(main_path.exists(), "main video file should exist");
    assert!(
        overlay_path.as_ref().map_or(false, |p| p.exists()),
        "overlay should exist for video with overlay"
    );

    // Combine video with overlay (requires Tauri app + ffmpeg sidecar)
    let clean_name = metadata::generate_clean_filename(
        &target_id.original_date,
        &target_id.id,
        main_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4"),
    );
    let combined_dest = dest_dir.join(&clean_name);

    if let Some(ref overlay) = overlay_path {
        if overlay.exists() {
            // Build minimal Tauri app to get AppHandle for ffmpeg sidecar
            let context = tauri::test::mock_context(tauri::test::noop_assets());
            let app = tauri::Builder::default()
                .plugin(tauri_plugin_shell::init())
                .build(context)
                .expect("build app");
            let handle = app.handle().clone();

            match combiner::combine_video(&handle, &main_path, overlay, &combined_dest).await {
                Ok(()) => assert!(combined_dest.exists(), "combined video should exist"),
                Err(e) => {
                    // FFmpeg sidecar may not be available in test env; extraction still verified
                    eprintln!("combine_video skipped (ffmpeg may be unavailable): {}", e);
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_hydrate_state_from_folder() {
    init_test_logging();
    let zip_path = test_zip_path();
    let (json_content, _) = fs::extract_json_from_zip(&zip_path).expect("extract JSON");
    let items = parse_memories_from_json(json_content.as_deref().unwrap_or(""));
    let ids = collect_ids_for_index(&items);
    let overlay_items: Vec<pipeline::OverlayItemRef> = items
        .iter()
        .map(|i| pipeline::OverlayItemRef {
            id: i.id.clone(),
            segment_ids: i.segment_ids.clone(),
        })
        .collect();

    let main_index = build_main_media_zip_index(&[zip_path.clone()], &ids).expect("main index");
    let overlay_index = build_overlay_zip_index(&[zip_path.clone()], &overlay_items).ok();

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let dest_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&dest_dir).expect("create output");

    // Extract a few items (use primary id; for split videos extract_from_export_zip gets first segment)
    for item in items.iter().take(3) {
        let _ = pipeline::extract_from_export_zip(
            &zip_path,
            &item.id,
            &dest_dir,
            &main_index,
            overlay_index.as_ref(),
            Some(&item.original_date),
        );
    }

    let db_path = temp_dir.path().join("test.db");
    let db = DbManager::new(&db_path).await.expect("create db");

    fs::hydrate_state_from_folder(&dest_dir, &db, &items)
        .await
        .expect("hydrate");

    let memories = db.get_all_memories().await.expect("get memories");
    assert!(!memories.is_empty());
}
