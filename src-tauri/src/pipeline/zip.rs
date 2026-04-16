//! ZIP indexing and extraction utilities for the pipeline.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::Emitter;
use serde::Serialize;
use chrono::{NaiveDate, NaiveDateTime};

#[derive(Serialize, Clone)]
struct ZipProgressPayload {
    path: String,
    progress: f32,
}

/// Known extensions for raw memory files (from downloader.rs).
pub(crate) const RAW_FILE_EXTENSIONS: &[&str] = &["zip", "mp4", "jpg", "mov"];

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
    main_index: &HashMap<String, (usize, usize, String)>,
    overlay_index: Option<&HashMap<String, (usize, usize, String)>>,
    date_str: Option<&str>,
) -> std::result::Result<PathBuf, String> {
    use std::fs::File;
    use std::io::BufReader;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let reader = BufReader::with_capacity(256 * 1024, file);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

    let clean_date = date_str.map(|s| crate::metadata::get_clean_date_prefix(s));
    let key_exact = clean_date.as_ref().map(|d| format!("{}|{}", d, id));
    let key_date_only = clean_date
        .as_ref()
        .filter(|d| d.len() >= 10)
        .map(|d| format!("{}|{}", &d[..10], id));
    // Never use "|id" fallback when we have a date - prevents wrong-month cascade overwrite
    let key_fallback = date_str.is_none().then(|| format!("|{}", id));
    let &(_zip_file_idx, zip_entry_index, ref ext) = key_exact
        .as_ref()
        .and_then(|k| main_index.get(k))
        .or_else(|| key_date_only.as_ref().and_then(|k| main_index.get(k)))
        .or_else(|| key_fallback.as_ref().and_then(|k| main_index.get(k)))
        .ok_or_else(|| format!("id {} not in main index", id))?;
    {
        let mut zip_file = archive.by_index(zip_entry_index).map_err(|e| e.to_string())?;
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
        if let Some(&(_ov_zip_file_idx, ov_zip_entry_index, ref ov_ext)) = ov_entry {
            if let Ok(mut ov_file) = archive.by_index(ov_zip_entry_index) {
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

fn normalize_date_prefix(s: &str) -> String {
    if s.len() >= 19 {
        let mut normalized = s[..19].to_string();
        // Force YYYY-MM-DD_HH-MM-SS format
        normalized.replace_range(10..11, "_");
        normalized.replace_range(13..14, "-");
        normalized.replace_range(16..17, "-");
        normalized
    } else if s.len() >= 10 {
        s[..10].to_string()
    } else {
        s.to_string()
    }
}

fn extract_date_prefix_from_name(name: &str) -> Option<String> {
    let base = Path::new(name).file_name()?.to_str()?;
    
    // Attempt 19-char timestamp (YYYY-MM-DD_HH-MM-SS)
    if base.len() >= 19 {
        let s = &base[..19];
        if s.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == '_' || c == ' ' || c == '-',
            13 | 16 => c == '-' || c == '_',
            _ => c.is_ascii_digit(),
        }) {
            return Some(s.to_string());
        }
    }
    
    // Attempt 10-char date (YYYY-MM-DD)
    if base.len() >= 10 {
        let s = &base[..10];
        if s.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        }) {
            return Some(s.to_string());
        }
    }
    
    None
}

fn date_prefix_to_epoch_seconds(date_prefix: &str) -> Option<i64> {
    let normalized = normalize_date_prefix(date_prefix);
    if normalized.len() >= 19 {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d_%H-%M-%S") {
            return Some(dt.and_utc().timestamp());
        }
    }
    if date_prefix.len() >= 10 {
        // YYYY-MM-DD
        if let Ok(d) = NaiveDate::parse_from_str(&date_prefix[..10], "%Y-%m-%d") {
            return Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
        }
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
            tracing::trace!(name = %name_lower, variant = %variant, before_ok, after_ok, "zip: token match failed on boundaries");
            search_start = abs_pos + 1;
        }
    }
    false
}

/// Scan fallback when index lookup fails. Finds main media by date + id.
/// Returns (zip_index, ext) if exactly one match, or None.
pub(crate) fn scan_main_by_date_and_id(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    date_prefix: &str,
    date_only: Option<&str>,
    seg_id: &str,
) -> Option<(usize, String)> {
    let mut candidates: Vec<(usize, String)> = Vec::new();
    // Optimization: avoid by_index(i) which reads local headers.
    // zip-rs file_names() is O(1) per entry once central directory is read.
    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    for (i, name) in names.into_iter().enumerate() {
        let name_lower = name.to_lowercase();
        if name_lower.contains("-overlay") {
            continue;
        }
        let file_date = match extract_date_prefix_from_name(&name) {
            Some(d) => d,
            None => continue,
        };
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
        let ext = Path::new(&name)
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

/// Fallback scan when date-based lookup fails (JSON date differs from ZIP filename timestamp).
/// Finds candidates by ID across all dates and picks the closest timestamp match.
///
/// Prefers `-main` files when tie-breaking.
pub(crate) fn scan_main_by_best_date_match(
    archive: &mut zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    target_epoch_seconds: Option<i64>,
    seg_id: &str,
) -> Option<(usize, String)> {
    let mut best: Option<(usize, String, i64, bool)> = None; // (idx, ext, abs_diff, is_main)
    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();

    for (i, name) in names.into_iter().enumerate() {
        let name_lower = name.to_lowercase();
        if name_lower.contains("-overlay") {
            continue;
        }
        if !id_appears_as_token(&name_lower, seg_id) {
            continue;
        }

        let ext = Path::new(&name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mp4")
            .to_string();
        let is_main = name_lower.contains("-main");

        let abs_diff = match (target_epoch_seconds, extract_date_prefix_from_name(&name).as_deref().and_then(date_prefix_to_epoch_seconds)) {
            (Some(t), Some(f)) => (t - f).abs(),
            _ => i64::MAX / 2,
        };

        match best {
            None => best = Some((i, ext, abs_diff, is_main)),
            Some((_, _, best_diff, best_is_main)) => {
                if abs_diff < best_diff || (abs_diff == best_diff && is_main && !best_is_main) {
                    best = Some((i, ext, abs_diff, is_main));
                }
            }
        }
    }

    best.map(|(i, ext, _, _)| (i, ext))
}

/// Composite key for date-disambiguated lookup. Same segment ID can appear in Jan and Mar.
fn composite_key(date: Option<&str>, id: &str) -> String {
    match date {
        Some(d) => format!("{}|{}", d, id),
        None => format!("|{}", id),
    }
}

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
        .trim_start_matches('-') // Added hyphen support
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
        .trim_start_matches('-') // Added hyphen support
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

/// Builds an index of `"date|id"` → `(zip_file_index, zip_entry_index, extension)` for main media only
/// (excludes overlay). Indexes ALL media files so we can match by date when JSON IDs differ from ZIP.
/// Uses date from the ZIP filename to disambiguate when the same ID appears in multiple months.
/// Prefers `-main` files. Call from `spawn_blocking` since it performs synchronous I/O.
pub fn build_main_media_zip_index(
    zip_paths: &[PathBuf],
    item_ids: &[String],
) -> std::result::Result<HashMap<String, (usize, usize, String)>, String> {
    let (main_idx, _) = build_pipeline_zip_indexes(None, "", zip_paths, item_ids, &[])?;
    Ok(main_idx)
}

/// Item info needed to assign overlays to the correct primary id for split videos.
#[derive(Clone)]
pub struct OverlayItemRef {
    pub id: String,
    pub segment_ids: Option<Vec<String>>,
    pub candidate_ids: Option<Vec<String>>,
}

/// Builds an index of "date|primary_id" -> (zip_file_index, zip_entry_index, extension) for overlay files only.
/// Uses date from ZIP filename to disambiguate when same ID appears in multiple months.
/// Matches: (1) filenames containing "overlay", or (2) PNGs containing id that aren't -main
/// (Snapchat export may use DATE_ID1-ID2.png for overlays without "overlay" in name).
///
/// For split videos: when an overlay file matches multiple segment IDs (e.g. DATE_ID1_ID2.png),
/// assigns it to the primary id of the memory whose segment_ids contains all matching IDs.
/// Merged indexing: Builds both main and overlay indexes in a single pass over the ZIP archives.
pub fn build_pipeline_zip_indexes(
    app: Option<&tauri::AppHandle>,
    session_id: &str,
    zip_paths: &[PathBuf],
    main_ids: &[String],
    overlay_refs: &[OverlayItemRef],
) -> std::result::Result<
    (
        HashMap<String, (usize, usize, String)>,
        HashMap<String, (usize, usize, String)>,
    ),
    String,
> {
    let overall_start = std::time::Instant::now();
    let mut main_index = HashMap::new();
    let mut overlay_temp: HashMap<String, (usize, usize, String, usize)> = HashMap::new();

    // 1. Pre-calculate lookup maps for fast ID resolving
    let mut main_id_map: HashMap<String, String> = HashMap::new();
    for id in main_ids {
        let id_lower = id.to_lowercase();
        main_id_map.insert(id_lower.clone(), id.clone());
        let without_hyphens: String = id_lower.chars().filter(|c| *c != '-').collect();
        if without_hyphens != id_lower {
            main_id_map.insert(without_hyphens, id.clone());
        }
    }

    let mut segment_to_primary: HashMap<String, (&OverlayItemRef, HashSet<String>)> = HashMap::new();
    for item in overlay_refs {
        let mut item_ids: HashSet<String> = HashSet::new();
        // Use candidate_ids if available, otherwise fallback to id+segments
        let ids: Vec<String> = item.candidate_ids.as_ref().cloned().unwrap_or_else(|| {
            item.segment_ids
                .as_ref()
                .cloned()
                .unwrap_or_else(|| vec![item.id.clone()])
        });
        
        for id in ids {
            let id_lower = id.to_lowercase();
            item_ids.insert(id_lower.clone());
            let without_hyphens: String = id_lower.chars().filter(|c| *c != '-').collect();
            if without_hyphens != id_lower {
                item_ids.insert(without_hyphens);
            }
        }
        
        for id in &item_ids {
            segment_to_primary.insert(id.clone(), (item, item_ids.clone()));
        }
    }

    // Parallel processing across ZIP archives using scoped threads (Rust 1.63+)
    // to avoid 'static lifetime requirements on shared maps.
    std::thread::scope(|s| {
        let mut handlers = Vec::new();
        for (zip_file_idx, zip_path) in zip_paths.iter().enumerate() {
            let zip_path = zip_path.clone();
            let session_id = session_id.to_string();
            let app = app.cloned();
            let main_id_map = &main_id_map;
            let segment_to_primary = &segment_to_primary;

            handlers.push(s.spawn(move || {
                index_single_zip(
                    zip_file_idx,
                    &zip_path,
                    &session_id,
                    app.as_ref(),
                    main_id_map,
                    segment_to_primary,
                )
            }));
        }

        for (zip_file_idx, handler) in handlers.into_iter().enumerate() {
            match handler.join() {
                Ok(Ok((local_main, local_ov))) => {
                    main_index.extend(local_main);
                    overlay_temp.extend(local_ov);
                }
                Ok(Err(e)) => {
                    tracing::error!(zip_file_idx, error = %e, "zip: indexing failed for archive");
                }
                Err(_) => {
                    tracing::error!(zip_file_idx, "zip: indexer thread panicked");
                }
            }
        }
    });

    let overlay_index: HashMap<String, (usize, usize, String)> = overlay_temp
        .into_iter()
        .map(|(k, (z, e, ext, _))| (k, (z, e, ext)))
        .collect();

    tracing::info!(
        zip_count = zip_paths.len(),
        main_index_len = main_index.len(),
        overlay_index_len = overlay_index.len(),
        elapsed_ms = overall_start.elapsed().as_millis(),
        "zip: indexing finished"
    );
    Ok((main_index, overlay_index))
}


pub fn build_overlay_zip_index(
    zip_paths: &[PathBuf],
    items: &[OverlayItemRef],
) -> std::result::Result<HashMap<String, (usize, usize, String)>, String> {
    let (_, overlay_idx) = build_pipeline_zip_indexes(None, "", zip_paths, &[], items)?;
    Ok(overlay_idx)
}

/// Indexes a single ZIP archive. Called from a scoped thread by `build_pipeline_zip_indexes`.
///
/// Returns `(local_main, local_overlay)` maps keyed by `"date|id"`.
/// Progress events are emitted at most once every 200 ms to avoid flooding the frontend.
#[allow(clippy::type_complexity)]
fn index_single_zip(
    zip_file_idx: usize,
    zip_path: &Path,
    session_id: &str,
    app: Option<&tauri::AppHandle>,
    main_id_map: &HashMap<String, String>,
    segment_to_primary: &HashMap<String, (&OverlayItemRef, HashSet<String>)>,
) -> std::result::Result<
    (
        HashMap<String, (usize, usize, String)>,
        HashMap<String, (usize, usize, String, usize)>,
    ),
    String,
> {
    let zip_start = Instant::now();
    let mut local_main: HashMap<String, (usize, usize, String)> = HashMap::new();
    let mut local_overlay: HashMap<String, (usize, usize, String, usize)> = HashMap::new();
    let zip_path_str = zip_path.to_string_lossy().to_string();

    // Throttle: emit at most one progress event per 200 ms.
    const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
    let mut last_emit = Instant::now()
        .checked_sub(PROGRESS_INTERVAL)
        .unwrap_or(Instant::now());

    let emit_progress = |progress: f32, last_emit: &mut Instant| {
        if let Some(ref app) = app {
            let now = Instant::now();
            if now.duration_since(*last_emit) >= PROGRESS_INTERVAL {
                let _ = app.emit(
                    &format!("zip-indexing-progress-{}", session_id),
                    ZipProgressPayload {
                        path: zip_path_str.clone(),
                        progress,
                    },
                );
                *last_emit = now;
            }
        }
    };

    emit_progress(0.0, &mut last_emit);

    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open {}: {}", zip_path.display(), e))?;
    let reader = std::io::BufReader::with_capacity(256 * 1024, file);
    let archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Invalid zip {}: {}", zip_path.display(), e))?;
    let len = archive.len();

    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    for (i, name) in names.into_iter().enumerate() {
        emit_progress((i as f32 / len as f32) * 100.0, &mut last_emit);

        let name_lower = name.to_lowercase();
        let ext = Path::new(&name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("zip")
            .to_string();

        let is_main = name_lower.contains("-main");
        let is_explicit_overlay = name_lower.contains("-overlay") || name_lower.contains("overlay");
        let is_png = ext.eq_ignore_ascii_case("png");        let date = extract_date_prefix_from_name(&name);
        let normalized_date_prefix = date.as_ref().map(|d| normalize_date_prefix(d));

        let ids_from_file = if is_explicit_overlay || is_png {
            extract_ids_from_overlay_filename(&name).unwrap_or_default()
        } else {
            extract_id_from_filename(&name).map(|id| vec![id]).unwrap_or_default()
        };

        if ids_from_file.is_empty() && (is_main || is_explicit_overlay) {
             tracing::debug!(name = %name, "zip: failed to extract ID from filename");
        }

        if !name_lower.contains("-overlay") {
            for id in &ids_from_file {
                let id_lower = id.to_lowercase();
                
                // 1. Direct index by ID in filename
                let key_from_file = composite_key(normalized_date_prefix.as_deref(), &id_lower);
                if is_main || !local_main.contains_key(&key_from_file) {
                    local_main.insert(key_from_file, (zip_file_idx, i, ext.clone()));
                }

                // 2. Index by all known aliases (siblings) for this segment.
                // This ensures lookup works regardless of whether filename used SID or MID.
                if let Some((_item, item_ids)) = segment_to_primary.get(&id_lower) {
                    for alias_id in item_ids {
                        let alias_key = composite_key(normalized_date_prefix.as_deref(), alias_id);
                        if is_main || !local_main.contains_key(&alias_key) {
                            local_main.insert(alias_key, (zip_file_idx, i, ext.clone()));
                        }
                    }
                }

                // 3. Fallback check from main_id_map
                if let Some(original_id) = main_id_map.get(&id_lower) {
                    let key_from_json = composite_key(normalized_date_prefix.as_deref(), original_id);
                    if is_main || !local_main.contains_key(&key_from_json) {
                        local_main.insert(key_from_json, (zip_file_idx, i, ext.clone()));
                    }
                }
            }
        }


        if is_explicit_overlay || (is_png && !is_main) {
            let mut matching_ids: HashSet<String> = HashSet::new();
            for id in &ids_from_file {
                let id_lower = id.to_lowercase();
                if segment_to_primary.contains_key(&id_lower) {
                    matching_ids.insert(id_lower);
                }
            }

            if !matching_ids.is_empty() {
                let specificity = matching_ids.len();
                let best_ref = matching_ids
                    .iter()
                    .filter_map(|m| segment_to_primary.get(m))
                    .find(|&(_, ref item_ids)| matching_ids.iter().all(|m| item_ids.contains(m)));

                if let Some((item, _)) = best_ref {
                    let key = composite_key(date.as_deref(), &item.id);
                    let should_insert =
                        local_overlay.get(&key).map_or(true, |(_, _, _, s)| specificity > *s);
                    if should_insert {
                        local_overlay.insert(key, (zip_file_idx, i, ext.clone(), specificity));
                    }
                } else if let Some(first_match_id) = matching_ids.iter().next() {
                    if let Some((item, _)) = segment_to_primary.get(first_match_id) {
                        let key = composite_key(date.as_deref(), &item.id);
                        let should_insert = local_overlay
                            .get(&key)
                            .map_or(true, |(_, _, _, s)| *s == 0 && specificity > 0);
                        if should_insert {
                            local_overlay.insert(key, (zip_file_idx, i, ext.clone(), 0));
                        }
                    }
                }
            }
        }
    }

    // Final 100% progress (always emit, bypassing the throttle).
    if let Some(ref app) = app {
        let _ = app.emit(
            &format!("zip-indexing-progress-{}", session_id),
            ZipProgressPayload {
                path: zip_path_str,
                progress: 100.0,
            },
        );
    }
    tracing::info!(
        zip_path = %zip_path.display(),
        entries = len,
        elapsed_ms = zip_start.elapsed().as_millis(),
        "zip: indexed archive"
    );
    Ok((local_main, local_overlay))
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
        let index = build_main_media_zip_index(&[zip_path], &ids).unwrap();
        // Index includes all files (123 and 12345) plus item_ids matches
        assert!(index.len() >= 1);
        let composite_key = "2018-02-07|123";
        assert!(index.contains_key(composite_key));
        let (zip_file_idx, entry_idx, ext) = index.get(composite_key).unwrap();
        assert_eq!(*zip_file_idx, 0);
        assert_eq!(*entry_idx, 1); // 123-main, not 12345-main
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
                candidate_ids: None,
            },
            OverlayItemRef {
                id: id3.to_string(),
                segment_ids: None,
                candidate_ids: None,
            },
        ];
        let overlay_index = build_overlay_zip_index(&[zip_path], &items).unwrap();
        assert!(
            overlay_index.len() >= 2,
            "index has JSON + filename-based entries"
        );
        let key1 = format!("2018-02-07|{}", id1);
        let key3 = format!("2018-02-07|{}", id3);
        assert!(overlay_index.contains_key(&key1));
        assert!(overlay_index.contains_key(&key3));
        let (ov_idx1_file, ov_idx1_ent, _) = overlay_index.get(&key1).unwrap();
        let (ov_idx3_file, ov_idx3_ent, _) = overlay_index.get(&key3).unwrap();
        assert_eq!(*ov_idx1_file, 0);
        assert_eq!(*ov_idx1_ent, 2, "overlay with ID1_ID2 for split video");
        assert_eq!(*ov_idx3_file, 0);
        assert_eq!(*ov_idx3_ent, 3, "overlay with ID3 only");
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
        let index = build_main_media_zip_index(&[zip_path], &ids).unwrap();
        let (jan_idx_file, jan_idx_ent, _) = index
            .get("2021-01-19_21-16-16|c888a42f-2b82-53a0-c17c-e4d0b67cc1fb")
            .unwrap();
        let (mar_idx_file, mar_idx_ent, _) = index
            .get("2021-03-03_19-41-46|f560c834-a033-13f2-cf18-0531ed083e27")
            .unwrap();
        let (jun_idx_file, jun_idx_ent, _) = index
            .get("2021-06-03_12-47-35|013c9515-02f6-d22c-a44e-ab166d8e8731")
            .unwrap();
        assert_eq!(*jan_idx_file, 0);
        assert_eq!(*jan_idx_ent, 0); // Jan at entry 0
        assert_eq!(*mar_idx_file, 0);
        assert_eq!(*mar_idx_ent, 1); // Mar at entry 1
        assert_eq!(*jun_idx_file, 0);
        assert_eq!(*jun_idx_ent, 2); // Jun at entry 2
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
        let index = build_main_media_zip_index(&[zip_path], &ids).unwrap();
        assert!(
            index.len() >= 2,
            "same ID in two months = at least two index entries"
        );
        let (jan_idx_file, jan_idx_ent, _) = index
            .get("2021-01-15_12-30-00|SHARED-ID-1111-2222-3333-444455556666")
            .unwrap();
        let (mar_idx_file, mar_idx_ent, _) = index
            .get("2021-03-20_10-00-00|SHARED-ID-1111-2222-3333-444455556666")
            .unwrap();
        assert_eq!(*jan_idx_file, 0);
        assert_eq!(*jan_idx_ent, 0); // Jan entry at index 0
        assert_eq!(*mar_idx_file, 0);
        assert_eq!(*mar_idx_ent, 1); // Mar entry at index 1
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
            candidate_ids: None,
        }];
        let overlay_index = build_overlay_zip_index(&[zip_path], &items).unwrap();
        assert!(overlay_index.len() >= 1);
        let key1 = format!("2018-02-07|{}", id1);
        let (ov_idx_file, ov_idx_ent, _) = overlay_index.get(&key1).unwrap();
        assert_eq!(*ov_idx_file, 0);
        assert_eq!(
            *ov_idx_ent, 2,
            "split overlay (ID1_ID2.png) should win over single (ID1-overlay.png)"
        );
    }

    #[test]
    fn test_extract_id_from_filename_35char_hyphen_date() {
        // Test case based on findings: 10-char date with hyphen separator, and a non-standard length ID.
        let name = "memories/2021-11-06-82a52b8f-099b-507-1cd2-c30936076a8a-main.mp4";
        let date = extract_date_prefix_from_name(name).expect("Should extract 10-char date");
        assert_eq!(date, "2021-11-06");
        let id = extract_id_from_filename(name).expect("Should extract ID");
        assert_eq!(id, "82a52b8f-099b-507-1cd2-c30936076a8a");
    }

    #[test]
    fn test_normalize_date_prefix_hyphens() {
        // Test normalization of dashes to underscores in timestamps
        let raw = "2021-01-15-12-30-00";
        let normalized = normalize_date_prefix(raw);
        assert_eq!(normalized, "2021-01-15_12-30-00");
    }
}

/// Builds an index of id -> (zip_file_index, zip_entry_index, extension) for fast extraction from export ZIP.
/// Includes both main and overlay; last match wins. Prefer build_main_media_zip_index for pipeline.
pub fn build_export_zip_index(
    zip_paths: &[PathBuf],
    item_ids: &[String],
) -> std::result::Result<HashMap<String, (usize, usize, String)>, String> {
    use std::io::BufReader;
    let id_set: HashSet<&str> = item_ids.iter().map(|s| s.as_str()).collect();
    let mut index = HashMap::new();

    for (zip_file_idx, zip_path) in zip_paths.iter().enumerate() {
        let file = std::fs::File::open(zip_path).map_err(|e| format!("Failed to open {}: {}", zip_path.display(), e))?;
        let reader = BufReader::new(file);
        let archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid zip {}: {}", zip_path.display(), e))?;

        let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
        for (i, name) in names.into_iter().enumerate() {
            let name_lower = name.to_lowercase();
            let ext = Path::new(&name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("zip");
            for id in &id_set {
                if id_appears_as_token(&name_lower, id) {
                    index.insert((*id).to_string(), (zip_file_idx, i, ext.to_string()));
                    break;
                }
            }
        }
    }
    Ok(index)
}
