//! ZIP indexing and extraction utilities for the pipeline.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Known extensions for raw memory files (from downloader.rs).
pub(crate) const RAW_FILE_EXTENSIONS: &[&str] = &[
    "zip", "mp4", "jpg", "jpeg", "mov", "png", "gif", "webm", "heic",
];

pub fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    lower == "mp4" || lower == "mov"
}

/// Tries to find raw file by id using known extensions first (O(1) per item).
pub(crate) fn find_raw_file_fast(dest_dir: &Path, id: &str) -> Option<PathBuf> {
    for ext in RAW_FILE_EXTENSIONS {
        let path = dest_dir.join(format!("{}-raw.{}", id, ext));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Extracts main and optionally overlay from export ZIP. Used by pipeline and E2E tests.
/// When date_str is provided and index uses composite keys (date|id), uses it for lookup.
pub fn extract_from_export_zip(
    zip_path: &Path,
    id: &str,
    dest_dir: &Path,
    main_index: &HashMap<String, (usize, String)>,
    overlay_index: Option<&HashMap<String, (usize, String)>>,
    date_str: Option<&str>,
) -> std::result::Result<PathBuf, String> {
    use std::fs::File;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let clean_date = date_str.map(|s| crate::metadata::get_clean_date_prefix(s));
    let key_exact = clean_date.as_ref().map(|d| format!("{}|{}", d, id));
    let key_date_only = clean_date
        .as_ref()
        .filter(|d| d.len() >= 10)
        .map(|d| format!("{}|{}", &d[..10], id));
    // Never use "|id" fallback when we have a date - prevents wrong-month cascade overwrite
    let key_fallback = date_str.is_none().then(|| format!("|{}", id));
    let &(zip_index, ref ext) = key_exact
        .as_ref()
        .and_then(|k| main_index.get(k))
        .or_else(|| key_date_only.as_ref().and_then(|k| main_index.get(k)))
        .or_else(|| key_fallback.as_ref().and_then(|k| main_index.get(k)))
        .ok_or_else(|| format!("id {} not in main index", id))?;
    {
        let mut zip_file = archive.by_index(zip_index).map_err(|e| e.to_string())?;
        let out_path = dest_dir.join(format!("{}-raw.{}", id, ext));
        let mut outfile = File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut zip_file, &mut outfile).map_err(|e| e.to_string())?;
    }
    if let Some(ov_idx) = overlay_index {
        let ov_key_exact = clean_date.as_ref().map(|d| format!("{}|{}", d, id));
        let ov_key_date_only = clean_date
            .as_ref()
            .filter(|d| d.len() >= 10)
            .map(|d| format!("{}|{}", &d[..10], id));
        let ov_key_fallback = date_str.is_none().then(|| format!("|{}", id));
        let ov_entry = ov_key_exact
            .as_ref()
            .and_then(|k| ov_idx.get(k))
            .or_else(|| ov_key_date_only.as_ref().and_then(|k| ov_idx.get(k)))
            .or_else(|| ov_key_fallback.as_ref().and_then(|k| ov_idx.get(k)));
        if let Some((ov_zip_index, ov_ext)) = ov_entry {
            if let Ok(mut ov_file) = archive.by_index(*ov_zip_index) {
                let ov_path = dest_dir.join(format!("{}-overlay.{}", id, ov_ext));
                if let Ok(mut ov_out) = File::create(&ov_path) {
                    let _ = std::io::copy(&mut ov_file, &mut ov_out);
                }
            }
        }
    }
    Ok(dest_dir.join(format!("{}-raw.{}", id, ext)))
}

/// Normalizes ID for matching: with hyphens (UUID) and without (some exports use no hyphens).
fn id_match_variants(id: &str) -> Vec<String> {
    let id_lower = id.to_lowercase();
    let without_hyphens: String = id_lower.chars().filter(|c| *c != '-').collect();
    if without_hyphens == id_lower {
        vec![id_lower]
    } else {
        vec![id_lower.clone(), without_hyphens]
    }
}

/// Extracts date prefix (YYYY-MM-DD_HH-MM-SS or YYYY-MM-DD) from ZIP entry name for disambiguation.
/// Snapchat exports use "2021-01-15_12-30-00_ID-main.mp4" - same ID can appear in Jan and Mar.
fn extract_date_prefix_from_name(name: &str) -> Option<String> {
    let base = Path::new(name).file_name().and_then(|n| n.to_str())?;
    if base.len() >= 19 {
        let s = &base[..19];
        if s.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == '_' || c == ' ',
            13 | 16 => c == '-',
            _ => c.is_ascii_digit(),
        }) {
            return Some(s.replace(' ', "_"));
        }
    }
    if base.len() >= 10 && base[..10].chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Some(base[..10].to_string());
    }
    None
}

/// Returns true if `id` appears in `name` as a complete token (not a substring of another ID).
/// Prevents "123" from matching "12345" or "abc" from matching "abc-def-123".
pub(crate) fn id_appears_as_token(name: &str, id: &str) -> bool {
    let name_lower = name.to_lowercase();
    for variant in id_match_variants(id) {
        if variant.is_empty() {
            continue;
        }
        let mut search_start = 0;
        while let Some(pos) = name_lower[search_start..].find(&variant) {
            let abs_pos = search_start + pos;
            let before_ok = abs_pos == 0
                || name_lower[..abs_pos]
                    .chars()
                    .last()
                    .map_or(true, |c| !c.is_ascii_alphanumeric());
            let end = abs_pos + variant.len();
            let after_ok = end >= name_lower.len()
                || name_lower[end..]
                    .chars()
                    .next()
                    .map_or(true, |c| !c.is_ascii_alphanumeric());
            if before_ok && after_ok {
                return true;
            }
            search_start = end;
        }
    }
    false
}

/// Scan fallback when index lookup fails. Finds main media by date + id.
/// Returns (zip_index, ext) if exactly one match, or None.
pub(crate) fn scan_main_by_date_and_id(
    archive: &mut zip::ZipArchive<std::fs::File>,
    date_prefix: &str,
    date_only: Option<&str>,
    seg_id: &str,
) -> Option<(usize, String)> {
    let mut candidates: Vec<(usize, String)> = Vec::new();
    for i in 0..archive.len() {
        let zip_file = archive.by_index(i).ok()?;
        let name = zip_file.name();
        let name_lower = name.to_lowercase();
        if name_lower.contains("-overlay") {
            continue;
        }
        let file_date = extract_date_prefix_from_name(name)?;
        let file_date_only = file_date.get(..10);
        let date_matches = file_date == date_prefix
            || date_only
                .and_then(|d| file_date_only.map(|fd| fd == d))
                .unwrap_or(false);
        if !date_matches {
            continue;
        }
        if !id_appears_as_token(&name_lower, seg_id) {
            continue;
        }
        let ext = Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mp4")
            .to_string();
        let is_main = name_lower.contains("-main");
        if is_main {
            candidates.insert(0, (i, ext));
        } else {
            candidates.push((i, ext));
        }
    }
    candidates.into_iter().next()
}

/// Composite key for date-disambiguated lookup. Same segment ID can appear in Jan and Mar.
fn composite_key(date: Option<&str>, id: &str) -> String {
    match date {
        Some(d) => format!("{}|{}", d, id),
        None => format!("|{}", id),
    }
}

/// Extracts the media ID from a ZIP filename. Format: "DATE_ID-main.mp4" or "DATE_ID-overlay.png".
pub(crate) fn extract_id_from_filename(name: &str) -> Option<String> {
    let base = Path::new(name).file_name().and_then(|n| n.to_str())?;
    let before_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let id_part = before_ext
        .strip_suffix("-main")
        .or_else(|| before_ext.strip_suffix("-overlay"))?;
    let date = extract_date_prefix_from_name(name)?;
    let after_date = id_part
        .strip_prefix(&date)?
        .trim_start_matches('_')
        .trim_start_matches(' ');
    if after_date.is_empty() {
        return None;
    }
    Some(after_date.to_string())
}

/// Extracts all IDs from overlay filename. Handles DATE_ID-overlay.png and DATE_ID1_ID2.png.
fn extract_ids_from_overlay_filename(name: &str) -> Option<Vec<String>> {
    let base = Path::new(name).file_name().and_then(|n| n.to_str())?;
    let before_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let date = extract_date_prefix_from_name(name)?;
    let after_date = before_ext
        .strip_prefix(&date)?
        .trim_start_matches('_')
        .trim_start_matches(' ');
    if after_date.is_empty() {
        return None;
    }
    if let Some(id_part) = after_date.strip_suffix("-overlay") {
        return Some(vec![id_part.trim().to_string()]);
    }
    // DATE_ID1_ID2.png - split by underscore; each part may be UUID with hyphens
    let ids: Vec<String> = after_date
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if ids.is_empty() {
        return None;
    }
    Some(ids)
}

/// Builds an index of "date|id" -> (zip_index, extension) for main media only (excludes overlay).
/// Indexes ALL media files (not just item_ids) so we can match by date when JSON IDs differ from ZIP.
/// Uses date from ZIP filename to disambiguate when same ID appears in multiple months.
/// Prefers -main files. Call from spawn_blocking since it does sync I/O.
pub fn build_main_media_zip_index(
    zip_path: &Path,
    item_ids: &[String],
) -> std::result::Result<HashMap<String, (usize, String)>, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut index = HashMap::new();

    for i in 0..archive.len() {
        let zip_file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = zip_file.name();
        let name_lower = name.to_lowercase();
        if name_lower.contains("-overlay") {
            continue;
        }
        let date = extract_date_prefix_from_name(name);
        let ext = Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("zip");
        let is_main = name_lower.contains("-main");

        // Index by date|id from filename (covers all files, handles JSON/ZIP ID mismatch)
        if let (Some(ref d), Some(ref id_from_file)) = (&date, extract_id_from_filename(name)) {
            let key = format!("{}|{}", d, id_from_file);
            if is_main || !index.contains_key(&key) {
                index.insert(key, (i, ext.to_string()));
            }
        }

        // Also index by item_ids for direct match (original behavior)
        for id in item_ids {
            if id_appears_as_token(&name_lower, id) {
                let key = composite_key(date.as_deref(), id);
                if is_main || !index.contains_key(&key) {
                    index.insert(key, (i, ext.to_string()));
                }
                break;
            }
        }
    }

    // Do NOT add "date!" fallback - when JSON has same date for multiple memories (e.g. all June 3),
    // date! would return the single June file for all, causing wrong video on Jan/Mar outputs.
    Ok(index)
}

/// Item info needed to assign overlays to the correct primary id for split videos.
#[derive(Clone)]
pub struct OverlayItemRef {
    pub id: String,
    pub segment_ids: Option<Vec<String>>,
}

/// Builds an index of "date|primary_id" -> (zip_index, extension) for overlay files only.
/// Uses date from ZIP filename to disambiguate when same ID appears in multiple months.
/// Matches: (1) filenames containing "overlay", or (2) PNGs containing id that aren't -main
/// (Snapchat export may use DATE_ID1-ID2.png for overlays without "overlay" in name).
///
/// For split videos: when an overlay file matches multiple segment IDs (e.g. DATE_ID1_ID2.png),
/// assigns it to the primary id of the memory whose segment_ids contains all matching IDs.
pub fn build_overlay_zip_index(
    zip_path: &Path,
    items: &[OverlayItemRef],
) -> std::result::Result<HashMap<String, (usize, String)>, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    // Store (zip_index, ext, specificity) so we prefer more specific overlays
    let mut index: HashMap<String, (usize, String, usize)> = HashMap::new();
    for i in 0..archive.len() {
        let zip_file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = zip_file.name();
        let name_lower = name.to_lowercase();
        let ext = Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("png");
        let is_png = ext.eq_ignore_ascii_case("png");
        let is_explicit_overlay = name_lower.contains("-overlay") || name_lower.contains("overlay");
        let is_main = name_lower.contains("-main");
        if !is_explicit_overlay && !(is_png && !is_main) {
            continue;
        }
        let date = extract_date_prefix_from_name(name);
        // Collect all ids (from any item) that appear in this overlay filename as tokens
        let mut matching_ids: HashSet<String> = HashSet::new();
        for item in items {
            let ids: Vec<&str> = item
                .segment_ids
                .as_ref()
                .map(|s| s.iter().map(|x| x.as_str()).collect())
                .unwrap_or_else(|| vec![item.id.as_str()]);
            for id in ids {
                if id_appears_as_token(&name_lower, id) {
                    matching_ids.insert(id.to_lowercase());
                }
            }
        }
        let specificity = matching_ids.len();
        let primary_id = items.iter().find(|item| {
            let item_ids: HashSet<String> = item
                .segment_ids
                .as_ref()
                .map(|s| s.iter().map(|x| x.to_lowercase()).collect())
                .unwrap_or_else(|| [item.id.to_lowercase()].into_iter().collect());
            matching_ids.iter().all(|m| item_ids.contains(m))
        });
        if let Some(item) = primary_id {
            let key = composite_key(date.as_deref(), &item.id);
            let should_insert = index.get(&key).map_or(true, |(_, _, s)| specificity > *s);
            if should_insert {
                index.insert(key, (i, ext.to_string(), specificity));
            }
        } else if let Some(first_match) = items.iter().find(|item| {
            let ids: Vec<&str> = item
                .segment_ids
                .as_ref()
                .map(|s| s.iter().map(|x| x.as_str()).collect())
                .unwrap_or_else(|| vec![item.id.as_str()]);
            ids.iter().any(|id| id_appears_as_token(&name_lower, id))
        }) {
            let key = composite_key(date.as_deref(), &first_match.id);
            let should_insert = index
                .get(&key)
                .map_or(true, |(_, _, s)| *s == 0 && specificity > 0);
            if should_insert {
                index.insert(key, (i, ext.to_string(), 0));
            }
        }

        // Index by date|id_from_filename so overlay lookup by main file's ID works
        // (JSON IDs can differ from ZIP filenames; main lookup uses filename ID)
        if let (Some(ref d), Some(ref ids_from_file)) =
            (&date, &extract_ids_from_overlay_filename(name))
        {
            let date_only = if d.len() >= 10 {
                d[..10].to_string()
            } else {
                d.clone()
            };
            for id_from_file in ids_from_file {
                let key_full = format!("{}|{}", d, id_from_file);
                let should_insert_full = index
                    .get(&key_full)
                    .map_or(true, |(_, _, s)| specificity > *s);
                if should_insert_full {
                    index.insert(key_full, (i, ext.to_string(), specificity));
                }
                let key_date = format!("{}|{}", date_only, id_from_file);
                let should_insert_date = index
                    .get(&key_date)
                    .map_or(true, |(_, _, s)| specificity > *s);
                if should_insert_date {
                    index.insert(key_date, (i, ext.to_string(), specificity));
                }
            }
        }
    }
    Ok(index
        .into_iter()
        .map(|(k, (idx, ext, _))| (k, (idx, ext)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    #[test]
    fn test_id_appears_as_token_no_substring_match() {
        // "123" must not match "12345" (substring)
        assert!(!id_appears_as_token("12345-main.mp4", "123"));
        assert!(!id_appears_as_token("2018-02-07_12345-main.mp4", "123"));
        assert!(!id_appears_as_token("abc12345-overlay.png", "abc123"));
    }

    #[test]
    fn test_id_appears_as_token_valid_matches() {
        assert!(id_appears_as_token(
            "2018-02-07_3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC-main.mp4",
            "3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC"
        ));
        assert!(id_appears_as_token(
            "2018-02-07_3c6dc8b5-6d3b-4f64-8e8a-2f90d45b63fc-main.mp4",
            "3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC"
        ));
        assert!(id_appears_as_token(
            "2018-02-07_3c6dc8b56d3b4f648e8a2f90d45b63fc-overlay.png",
            "3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC"
        )); // no-hyphen variant
    }

    #[test]
    fn test_id_appears_as_token_split_overlay() {
        let id1 = "3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC";
        let id2 = "2D98014C-444F-4D50-99BE-940E4E883393";
        let name = "2018-02-07_3C6DC8B5-6D3B-4F64-8E8A-2F90D45B63FC_2D98014C-444F-4D50-99BE-940E4E883393.png";
        assert!(id_appears_as_token(name, id1));
        assert!(id_appears_as_token(name, id2));
    }

    fn create_test_zip(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        (dir, zip_path)
    }

    #[test]
    fn test_build_main_index_no_substring_false_positive() {
        let (_dir, zip_path) = create_test_zip(&[
            ("2018-02-07_12345-main.mp4", b"fake"),
            ("2018-02-07_123-main.mp4", b"fake2"),
        ]);
        let ids = vec!["123".to_string()];
        let index = build_main_media_zip_index(&zip_path, &ids).unwrap();
        // Index includes all files (123 and 12345) plus item_ids matches
        assert!(index.len() >= 1);
        let composite_key = "2018-02-07|123";
        assert!(index.contains_key(composite_key));
        let (idx, ext) = index.get(composite_key).unwrap();
        assert_eq!(*idx, 1); // 123-main, not 12345-main
        assert_eq!(ext, "mp4");
    }

    #[test]
    fn test_build_overlay_index_split_video_assignment() {
        let id1 = "AAA11111-2222-3333-4444-555555555555";
        let id2 = "BBB22222-3333-4444-5555-666666666666";
        let id3 = "CCC33333-4444-5555-6666-777777777777";
        let (_dir, zip_path) = create_test_zip(&[
            ("2018-02-07_AAA11111-2222-3333-4444-555555555555-main.mp4", b"main1"),
            ("2018-02-07_BBB22222-3333-4444-5555-666666666666-main.mp4", b"main2"),
            ("2018-02-07_AAA11111-2222-3333-4444-555555555555_BBB22222-3333-4444-5555-666666666666.png", b"overlay"),
            ("2018-02-07_CCC33333-4444-5555-6666-777777777777-overlay.png", b"overlay2"),
        ]);
        let items = vec![
            OverlayItemRef {
                id: id1.to_string(),
                segment_ids: Some(vec![id1.to_string(), id2.to_string()]),
            },
            OverlayItemRef {
                id: id3.to_string(),
                segment_ids: None,
            },
        ];
        let overlay_index = build_overlay_zip_index(&zip_path, &items).unwrap();
        assert!(
            overlay_index.len() >= 2,
            "index has JSON + filename-based entries"
        );
        let key1 = format!("2018-02-07|{}", id1);
        let key3 = format!("2018-02-07|{}", id3);
        assert!(overlay_index.contains_key(&key1));
        assert!(overlay_index.contains_key(&key3));
        let (ov_idx1, _) = overlay_index.get(&key1).unwrap();
        let (ov_idx3, _) = overlay_index.get(&key3).unwrap();
        assert_eq!(*ov_idx1, 2, "overlay with ID1_ID2 for split video");
        assert_eq!(*ov_idx3, 3, "overlay with ID3 only");
    }

    #[test]
    fn test_index_same_video_different_dates() {
        // Same content in Jan, Mar, Jun - each gets its own index entry (no dedup)
        let same_content = b"same_video_content_here";
        let (_dir, zip_path) = create_test_zip(&[
            (
                "memories/2021-01-19_21-16-16_c888a42f-2b82-53a0-c17c-e4d0b67cc1fb-main.mp4",
                same_content,
            ),
            (
                "memories/2021-03-03_19-41-46_f560c834-a033-13f2-cf18-0531ed083e27-main.mp4",
                same_content,
            ),
            (
                "memories/2021-06-03_12-47-35_013c9515-02f6-d22c-a44e-ab166d8e8731-main.mp4",
                same_content,
            ),
        ]);
        let ids = vec![
            "c888a42f-2b82-53a0-c17c-e4d0b67cc1fb".to_string(),
            "f560c834-a033-13f2-cf18-0531ed083e27".to_string(),
            "013c9515-02f6-d22c-a44e-ab166d8e8731".to_string(),
        ];
        let index = build_main_media_zip_index(&zip_path, &ids).unwrap();
        let (jan_idx, _) = index
            .get("2021-01-19_21-16-16|c888a42f-2b82-53a0-c17c-e4d0b67cc1fb")
            .unwrap();
        let (mar_idx, _) = index
            .get("2021-03-03_19-41-46|f560c834-a033-13f2-cf18-0531ed083e27")
            .unwrap();
        let (jun_idx, _) = index
            .get("2021-06-03_12-47-35|013c9515-02f6-d22c-a44e-ab166d8e8731")
            .unwrap();
        assert_eq!(*jan_idx, 0, "Jan at index 0");
        assert_eq!(*mar_idx, 1, "Mar at index 1");
        assert_eq!(*jun_idx, 2, "Jun at index 2");
    }

    #[test]
    fn test_build_main_index_date_disambiguates_same_id_in_different_months() {
        // Same segment ID in Jan and Mar - index must use date to disambiguate
        let seg_id = "SHARED-ID-1111-2222-3333-444455556666";
        let (_dir, zip_path) = create_test_zip(&[
            (
                "2021-01-15_12-30-00_SHARED-ID-1111-2222-3333-444455556666-main.mp4",
                b"jan_video",
            ),
            (
                "2021-03-20_10-00-00_SHARED-ID-1111-2222-3333-444455556666-main.mp4",
                b"mar_video",
            ),
        ]);
        let ids = vec![seg_id.to_string()];
        let index = build_main_media_zip_index(&zip_path, &ids).unwrap();
        assert!(
            index.len() >= 2,
            "same ID in two months = at least two index entries"
        );
        let (jan_idx, _) = index
            .get("2021-01-15_12-30-00|SHARED-ID-1111-2222-3333-444455556666")
            .unwrap();
        let (mar_idx, _) = index
            .get("2021-03-20_10-00-00|SHARED-ID-1111-2222-3333-444455556666")
            .unwrap();
        assert_eq!(*jan_idx, 0, "Jan entry at index 0");
        assert_eq!(*mar_idx, 1, "Mar entry at index 1");
    }

    #[test]
    fn test_overlay_specificity_prefer_split_over_single() {
        // When ZIP has both DATE_ID1_ID2.png (split) and DATE_ID1-overlay.png (single),
        // the split overlay must win for the split video (prevents wrong overlay on combined video)
        let id1 = "AAA11111-2222-3333-4444-555555555555";
        let id2 = "BBB22222-3333-4444-5555-666666666666";
        let (_dir, zip_path) = create_test_zip(&[
            ("2018-02-07_AAA11111-2222-3333-4444-555555555555-main.mp4", b"main1"),
            ("2018-02-07_AAA11111-2222-3333-4444-555555555555-overlay.png", b"wrong_overlay"),
            ("2018-02-07_AAA11111-2222-3333-4444-555555555555_BBB22222-3333-4444-5555-666666666666.png", b"correct_overlay"),
        ]);
        let items = vec![OverlayItemRef {
            id: id1.to_string(),
            segment_ids: Some(vec![id1.to_string(), id2.to_string()]),
        }];
        let overlay_index = build_overlay_zip_index(&zip_path, &items).unwrap();
        assert!(overlay_index.len() >= 1);
        let key1 = format!("2018-02-07|{}", id1);
        let (ov_idx, _) = overlay_index.get(&key1).unwrap();
        assert_eq!(
            *ov_idx, 2,
            "split overlay (ID1_ID2.png) should win over single (ID1-overlay.png)"
        );
    }
}

/// Builds an index of id -> (zip_index, extension) for fast extraction from export ZIP.
/// Includes both main and overlay; last match wins. Prefer build_main_media_zip_index for pipeline.
pub fn build_export_zip_index(
    zip_path: &Path,
    item_ids: &[String],
) -> std::result::Result<HashMap<String, (usize, String)>, String> {
    let id_set: HashSet<&str> = item_ids.iter().map(|s| s.as_str()).collect();
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut index = HashMap::new();
    for i in 0..archive.len() {
        let zip_file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = zip_file.name();
        let name_lower = name.to_lowercase();
        let ext = Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("zip");
        for id in &id_set {
            if id_appears_as_token(&name_lower, id) {
                index.insert((*id).to_string(), (i, ext.to_string()));
                break;
            }
        }
    }
    Ok(index)
}
