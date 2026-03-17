use crate::db::MemoryRepository;
use tracing::info;
use crate::metadata;
use crate::models::{MemoryItem, ProcessingState};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Extracts the JSON metadata straight from the provided Snapchat `.zip` archive.
/// Returns the parsed string content and the deduced destination `memories` folder.
pub fn extract_json_from_zip(zip_path: &Path) -> Result<(Option<String>, PathBuf), String> {
    info!(path = %zip_path.display(), "extract_json_from_zip");
    let file = File::open(zip_path).map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {}", e))?;

    let mut json_content = String::new();
    let mut found = false;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        if file.name() == "json/memories_history.json" {
            file.read_to_string(&mut json_content)
                .map_err(|e| format!("Failed reading JSON bytes: {}", e))?;
            found = true;
            break;
        }
    }

    let result_json = if found { Some(json_content) } else { None };

    let parent_dir = zip_path.parent().unwrap_or(Path::new(""));
    let zip_name_without_ext = zip_path.file_stem().unwrap_or_default().to_string_lossy();
    let memories_dir = parent_dir.join(format!("{}_extracted_memories", zip_name_without_ext));

    Ok((result_json, memories_dir))
}

/// Recursively scans the memories folder and auto-populates the database state if
/// matching files are already present downloaded from Snapchat's new export system.
pub async fn hydrate_state_from_folder(
    memories_dir: &Path,
    db: &impl MemoryRepository,
    items: &[MemoryItem],
) -> Result<(), String> {
    // 1. Bulk insert items first so they exist in DB before we update them
    let _ = db.bulk_insert_memories(items.to_vec()).await;

    if !memories_dir.exists() {
        return Ok(());
    }

    // 2. Scan root-level files and build a lookup map for O(1) checks
    let mut files_by_name: HashMap<String, PathBuf> = HashMap::new();
    for entry in WalkDir::new(memories_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                files_by_name.insert(name.to_lowercase(), entry.path().to_path_buf());
            }
        }
    }

    for item in items {
        let id_lower = item.id.to_lowercase();

        let id_marker = format!("_{}.", id_lower);
        let id_marker_alt = format!("{}.", id_lower); // In case it's just the ID
        let main_marker = format!("{}-main", id_lower);
        let zip_marker = format!("{}-raw.zip", id_lower);

        // Prefer date-prefixed combined file (pipeline format) when it exists
        let clean_date = metadata::get_clean_date_prefix(&item.original_date);
        let ext = item.extension.clone().or_else(|| {
            let url = item.download_url.to_lowercase();
            if url.contains(".mp4") || url.contains(".mov") || url.contains("video") {
                Some("mp4".to_string())
            } else {
                Some("jpg".to_string())
            }
        });
        
        let expected_combined = ext.as_ref().map(|e| format!("{}_{}.{}", clean_date, item.id, e).to_lowercase());
        
        let found_date_prefixed = expected_combined.as_ref().and_then(|expected| {
            files_by_name.get(expected).map(|_| expected.as_str())
        });

        let found_file = found_date_prefixed.map(|s| s.to_string()).or_else(|| {
            // Fallback: search for files containing the ID marker. 
            // This is still slightly slower but we only do it if the direct date-match fails.
            files_by_name.keys().find(|f| {
                (f.contains(&id_marker) || f.starts_with(&id_marker_alt))
                    && !f.contains("-main")
                    && !f.contains("-overlay")
                    && !f.ends_with(".zip")
            }).cloned()
        });

        let main_file = files_by_name.keys().find(|f| f.contains(&main_marker));

        let overlay_marker = format!("{}-overlay", id_lower);
        let overlay_exists = files_by_name.keys().any(|f| f.contains(&overlay_marker));

        let zip_exists = files_by_name.keys().any(|f| f.contains(&zip_marker));

        if let Some(ref f) = found_file {
            let ext = Path::new(f)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());

            let _ = db.update_state(
                &item.id,
                ProcessingState::Completed,
                None,
                ext,
                Some(overlay_exists),
                None,
            ).await;
        } else if let Some(m) = main_file {
            let ext = Path::new(m)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());

            let _ = db.update_state(
                &item.id,
                ProcessingState::Unpacked,
                None,
                ext,
                Some(overlay_exists),
                None,
            ).await;
        } else if zip_exists {
            let _ = db.update_state(
                &item.id,
                ProcessingState::Acquired,
                None,
                None,
                Some(overlay_exists),
                None,
            ).await;
        }
    }

    Ok(())
}

// DELETED extract_preview_to_temp

/// Scans each directory once and builds id -> path map for all requested memory IDs.
/// Avoids N+1 WalkDir scans when resolving many paths.
pub fn resolve_local_media_paths_batch(
    scan_dirs: &[PathBuf],
    memory_ids: &HashSet<String>,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    if memory_ids.is_empty() {
        return result;
    }
    for dir in scan_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let path_str = entry.path().to_string_lossy().to_string();
            for id in memory_ids {
                if result.contains_key(id) {
                    continue;
                }
                let id_lower = id.to_lowercase();
                let id_marker = format!("_{}.", id_lower);
                let id_marker_alt = format!("{}.", id_lower);
                let main_marker = format!("{}-main", id_lower);
                // Preview format: {id}.{ext} (e.g. extracted from ZIP to temp)
                let is_preview = Path::new(&name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase() == id_lower)
                    .unwrap_or(false);
                let is_combined = (name.contains(&id_marker) || name.starts_with(&id_marker_alt))
                    && !name.contains("-main")
                    && !name.contains("-overlay")
                    && !name.ends_with(".zip");
                let is_main = name.contains(&main_marker);
                if is_preview || is_combined || is_main {
                    result.insert(id.clone(), path_str.clone());
                    break;
                }
            }
        }
    }
    result
}

/// Finds the actual local media file path for a memory by scanning the output directory.
/// Returns the path if a matching file exists, None otherwise.
pub fn resolve_local_media_path(memories_dir: &Path, memory_id: &str) -> Option<PathBuf> {
    if !memories_dir.exists() {
        return None;
    }
    let id_lower = memory_id.to_lowercase();
    let id_marker = format!("_{}.", id_lower);
    let id_marker_alt = format!("{}.", id_lower);
    let main_marker = format!("{}-main", id_lower);

    for entry in WalkDir::new(memories_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        // Combined file: contains _id. or starts with id. (exclude -main, -overlay, .zip)
        let is_combined = (name.contains(&id_marker) || name.starts_with(&id_marker_alt))
            && !name.contains("-main")
            && !name.contains("-overlay")
            && !name.ends_with(".zip");
        // Extracted main file
        let is_main = name.contains(&main_marker);
        if is_combined || is_main {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

