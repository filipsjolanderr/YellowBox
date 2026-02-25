use crate::db::{DbManager, MemoryRepository};
use crate::error::Result;
use crate::pipeline::PipelineService;
use crate::{downloader, extractor, metadata, fs};
use crate::models::{MemoryItem, ProcessingState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

pub struct AppState {
    pub db: Mutex<Option<Arc<DbManager>>>,
    pub export_path: Mutex<Option<PathBuf>>,
    pub output_dir: Mutex<Option<PathBuf>>,
}

#[tauri::command]
pub fn check_zip_structure(path: &str, state: State<'_, AppState>) -> Result<String> {
    let base_path = Path::new(path);
    let (json_content, _default_memories_dir) = fs::extract_json_from_zip(base_path)?;

    *state.export_path.lock().unwrap() = Some(PathBuf::from(path));
    Ok(json_content)
}

#[tauri::command]
pub fn initialize_and_load(
    output_path: &str,
    items: Vec<MemoryItem>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryItem>> {
    let out_dir = PathBuf::from(output_path);

    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir)?;
    }

    let db_path = out_dir.join("memories.db");
    let db_manager = DbManager::new(db_path)?;

    // Scan destination folder for previous progress tracking
    fs::hydrate_state_from_folder(&out_dir, &db_manager, &items)?;

    let memories = db_manager.get_all_memories()?;

    *state.db.lock().unwrap() = Some(Arc::new(db_manager));
    *state.output_dir.lock().unwrap() = Some(out_dir);

    Ok(memories)
}

#[tauri::command]
pub fn get_memories_state(state: State<'_, AppState>) -> Result<Vec<MemoryItem>> {
    if let Some(db) = state.db.lock().unwrap().as_ref() {
        db.get_all_memories()
    } else {
        Err("Database not initialized yet".into())
    }
}

#[tauri::command]
pub fn reset_application(state: State<'_, AppState>) -> Result<()> {
    *state.db.lock().unwrap() = None;
    *state.export_path.lock().unwrap() = None;
    *state.output_dir.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn cleanup_database(state: State<'_, AppState>) -> Result<()> {
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

#[tauri::command]
pub async fn start_pipeline(
    concurrency_limit: usize,
    overwrite_existing: bool,
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<()> {
    let dest_dir = {
        let lock = state.output_dir.lock().unwrap();
        lock.clone().ok_or_else(|| "No output directory selected".to_string())?
    };

    let items_to_process = {
        let db_lock = state.db.lock().unwrap();
        let db = db_lock.as_ref().ok_or_else(|| "DB not initialized".to_string())?;
        let all = db.get_all_memories()?;
        all.into_iter()
            .filter(|i| i.state != ProcessingState::Completed || overwrite_existing)
            .collect::<Vec<_>>()
    };

    let db = {
        let db_lock = state.db.lock().unwrap();
        db_lock.as_ref().cloned().ok_or_else(|| "DB not initialized".to_string())?
    };

    let pipeline_service = PipelineService::new(db, app, dest_dir);

    // Spawn a background task to process the pipeline asynchronously
    tokio::spawn(async move {
        if let Err(e) = pipeline_service.process_all(items_to_process, concurrency_limit, overwrite_existing).await {
            eprintln!("Pipeline background task error: {}", e);
        }
    });

    Ok(())
}
