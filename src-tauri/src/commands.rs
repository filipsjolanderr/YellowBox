use tauri::{AppHandle, Emitter, State};
use std::path::{Path, PathBuf};
use crate::models::{MemoryItem, ProcessingState};
use crate::db::DbManager;
use crate::fs;
use crate::downloader;
use crate::extractor;
use crate::combiner;
use crate::metadata;
use std::sync::Mutex;
use std::sync::Arc;

fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    lower == "mp4" || lower == "mov"
}

pub struct AppState {
    pub db: Mutex<Option<DbManager>>,
    pub export_path: Mutex<Option<PathBuf>>,
    pub output_dir: Mutex<Option<PathBuf>>,
}

#[tauri::command]
pub fn check_zip_structure(path: &str, state: State<'_, AppState>) -> Result<String, String> {
    let base_path = Path::new(path);
    let (json_content, _default_memories_dir) = fs::extract_json_from_zip(base_path)?;

    *state.export_path.lock().unwrap() = Some(PathBuf::from(path));
    Ok(json_content)
}

#[tauri::command]
pub fn initialize_and_load(output_path: &str, items: Vec<MemoryItem>, state: State<'_, AppState>) -> Result<Vec<MemoryItem>, String> {
    let out_dir = PathBuf::from(output_path);
    
    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    }
    
    let db_path = out_dir.join("memories.db");
    let db_manager = DbManager::new(db_path.to_str().unwrap()).map_err(|e| e.to_string())?;
    
    // Scan destination folder for previous progress tracking
    fs::hydrate_state_from_folder(&out_dir, &db_manager, &items)?;
    
    let memories = db_manager.get_all_memories().map_err(|e| e.to_string())?;
    
    *state.db.lock().unwrap() = Some(db_manager);
    *state.output_dir.lock().unwrap() = Some(out_dir);
    
    Ok(memories)
}

#[tauri::command]
pub fn get_memories_state(state: State<'_, AppState>) -> Result<Vec<MemoryItem>, String> {
    if let Some(db) = state.db.lock().unwrap().as_ref() {
        db.get_all_memories().map_err(|e| e.to_string())
    } else {
        Err("Database not initialized yet".to_string())
    }
}

#[tauri::command]
pub fn reset_application(state: State<'_, AppState>) -> Result<(), String> {
    *state.db.lock().unwrap() = None;
    *state.export_path.lock().unwrap() = None;
    *state.output_dir.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn cleanup_database(state: State<'_, AppState>) -> Result<(), String> {
    let out_dir = state.output_dir.lock().unwrap().clone();
    
    // Explicitly unhook sqlite file lock
    *state.db.lock().unwrap() = None;
    
    if let Some(dir) = out_dir {
        let db_path = dir.join("memories.db");
        if db_path.exists() {
            let _ = std::fs::remove_file(db_path);
        }
    }
    Ok(())
}

async fn process_item(item: MemoryItem, db: Arc<Mutex<DbManager>>, app: AppHandle, dest_dir: PathBuf) -> Result<MemoryItem, String> {
    let client = reqwest::Client::new();
    let mut current_item = item.clone();

    macro_rules! update_state {
        ($state:expr, $err:expr) => {
            update_state!($state, $err, None)
        };
        ($state:expr, $err:expr, $ext:expr) => {{
            current_item.state = $state;
            current_item.error_message = $err;
            if let Some(ext) = $ext {
                current_item.extension = Some(ext);
            }
            let db_lock = db.lock().unwrap();
            let _ = db_lock.update_state(&current_item.id, current_item.state.clone(), current_item.error_message.as_deref(), current_item.extension.clone());
            let _ = app.emit("memory-updated", current_item.clone());
        }};
    }

    // 1. Download
    let zip_path = if current_item.state == ProcessingState::Pending {
        let path = downloader::download_memory(&client, &current_item, &dest_dir).await;
        match path {
            Ok(p) => {
                update_state!(ProcessingState::Downloaded, None);
                p
            },
            Err(e) => {
                update_state!(ProcessingState::Failed, Some(e.clone()));
                return Err(e);
            }
        }
    } else {
        // If already downloaded, find the zip
        dest_dir.join(format!("{}-raw.zip", current_item.id))
    };

    // 2. Extract
    let extracted_files = if current_item.state == ProcessingState::Downloaded {
        if zip_path.exists() {
            let res = extractor::extract_memory(&zip_path, &current_item.id, &dest_dir).await;
            match res {
                Ok(files) => {
                    let ext = files.0.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
                    update_state!(ProcessingState::Extracted, None, ext);
                    files
                },
                Err(e) => {
                    update_state!(ProcessingState::Failed, Some(e.clone()));
                    return Err(e);
                }
            }
        } else {
             update_state!(ProcessingState::Failed, Some("Downloaded zip missing".to_string()));
             return Err("Downloaded zip missing".to_string());
        }
    } else {
         let ext = current_item.extension.as_deref().unwrap_or_else(|| {
             let url = current_item.download_url.to_lowercase();
             if url.contains(".mp4") || url.contains(".mov") || url.contains("video") { "mp4" } else { "jpg" }
         });
         let main = dest_dir.join(format!("{}-main.{}", current_item.id, ext));
         let overlay = dest_dir.join(format!("{}-overlay.png", current_item.id));
         (main, if overlay.exists() { Some(overlay) } else { None })
    };

    // 3. Combine
    let final_file = if current_item.state == ProcessingState::Extracted {
        let (main_path, overlay_path) = extracted_files.clone();
        let ext = current_item.extension.as_deref().unwrap_or_else(|| {
             let url = current_item.download_url.to_lowercase();
             if url.contains(".mp4") || url.contains(".mov") || url.contains("video") { "mp4" } else { "jpg" }
        });
        let clean_name = metadata::generate_clean_filename(&current_item.original_date, &current_item.id, ext);
        let combined_dest = dest_dir.join(clean_name);

        if let Some(overlay) = overlay_path {
            if is_video_ext(ext) {
                if let Err(e) = combiner::combine_video(&app, &main_path, &overlay, &combined_dest).await {
                    update_state!(ProcessingState::Failed, Some(e.clone()));
                    return Err(e);
                }
            } else {
                if let Err(e) = combiner::combine_image(&main_path, &overlay, &combined_dest).await {
                    update_state!(ProcessingState::Failed, Some(e.clone()));
                    return Err(e);
                }
            }
        } else {
            let _ = tokio::fs::copy(main_path, &combined_dest).await;
        }

        update_state!(ProcessingState::Combined, None);
        combined_dest
    } else {
         let ext = current_item.extension.as_deref().unwrap_or_else(|| {
             let url = current_item.download_url.to_lowercase();
             if url.contains(".mp4") || url.contains(".mov") || url.contains("video") { "mp4" } else { "jpg" }
         });
         let clean_name = metadata::generate_clean_filename(&current_item.original_date, &current_item.id, ext);
         dest_dir.join(&clean_name)
    };

    // 4. Metadata
    if current_item.state == ProcessingState::Combined {
        if let Err(e) = metadata::set_file_times(&final_file, &current_item.original_date).await {
            update_state!(ProcessingState::Failed, Some(format!("Metadata Error: {}", e)));
            return Err(e);
        }

        // Cleanup intermediate files
        let _ = tokio::fs::remove_file(&extracted_files.0).await;
        if let Some(overlay_path) = &extracted_files.1 {
            let _ = tokio::fs::remove_file(overlay_path).await;
        }
        let _ = tokio::fs::remove_file(&zip_path).await;

        update_state!(ProcessingState::Completed, None);
    }

    Ok(current_item)
}

#[tauri::command]
pub async fn start_pipeline(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dest_dir = {
        let lock = state.output_dir.lock().unwrap();
        lock.clone().ok_or("No output directory selected")?
    };

    let items_to_process = {
        let db_lock = state.db.lock().unwrap();
        let db = db_lock.as_ref().ok_or("DB not initialized")?;
        let all = db.get_all_memories().map_err(|e| e.to_string())?;
        all.into_iter().filter(|i| i.state != ProcessingState::Completed).collect::<Vec<_>>()
    };

    let db_path = dest_dir.join("memories.db");
    let arc_db = Arc::new(Mutex::new(DbManager::new(db_path.to_str().unwrap()).map_err(|e| e.to_string())?));

    for item in items_to_process {
        let db_clone = arc_db.clone();
        let app_clone = app.clone();
        let dest_clone = dest_dir.clone();
        
        let _ = tokio::spawn(async move {
            let _ = process_item(item, db_clone, app_clone, dest_clone).await;
        });
    }

    Ok(())
}
