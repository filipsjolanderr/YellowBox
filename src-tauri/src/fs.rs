use crate::db::DbManager;
use crate::models::{MemoryItem, ProcessingState};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Extracts the JSON metadata straight from the provided Snapchat `.zip` archive.
/// Returns the parsed string content and the deduced destination `memories` folder.
pub fn extract_json_from_zip(zip_path: &Path) -> Result<(String, PathBuf), String> {
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

    if !found {
        return Err("Could not find json/memories_history.json inside the provided zip file. Are you sure you selected your Snapchat Data Export zip?".to_string());
    }

    let parent_dir = zip_path.parent().unwrap_or(Path::new(""));
    let zip_name_without_ext = zip_path.file_stem().unwrap_or_default().to_string_lossy();
    let memories_dir = parent_dir.join(format!("{}_extracted_memories", zip_name_without_ext));

    Ok((json_content, memories_dir))
}

/// Recursively scans the memories folder and auto-populates the database state if
/// matching files are already present downloaded from Snapchat's new export system.
pub fn hydrate_state_from_folder(
    memories_dir: &Path,
    db: &DbManager,
    items: &[MemoryItem],
) -> Result<(), String> {
    if !memories_dir.exists() {
        return Ok(());
    }

    // A small local cache of filenames in the directory to avoid constant fs hits
    let mut existing_files: Vec<String> = Vec::new();
    for entry in WalkDir::new(memories_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                existing_files.push(name.to_lowercase());
            }
        }
    }

    for item in items {
        let _ = db.insert_or_ignore_memory(item);

        let id_lower = item.id.to_lowercase();

        let found_file = existing_files.iter().find(|f| {
            f.contains(&id_lower)
                && !f.contains("-main")
                && !f.contains("-overlay")
                && !f.ends_with(".zip")
        });

        let main_file = existing_files
            .iter()
            .find(|f| f.contains(&id_lower) && f.contains("-main"));

        let zip_exists = existing_files
            .iter()
            .any(|f| f.contains(&id_lower) && f.ends_with(".zip"));

        if let Some(f) = found_file {
            let ext = Path::new(f)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            let _ = db.update_state(&item.id, ProcessingState::Completed, None, ext);
        } else if let Some(m) = main_file {
            let ext = Path::new(m)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            let _ = db.update_state(&item.id, ProcessingState::Extracted, None, ext);
        } else if zip_exists {
            let _ = db.update_state(&item.id, ProcessingState::Downloaded, None, None);
        }
    }

    Ok(())
}
