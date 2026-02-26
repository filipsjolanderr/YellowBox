use crate::db::{DbManager, MemoryRepository};
use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use crate::pipeline::PipelineService;
use crate::fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State, Manager};

pub struct SessionState {
    pub db: Option<Arc<DbManager>>,
    pub export_path: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            db: None,
            export_path: None,
            output_dir: None,
        }
    }
}

pub struct AppState {
    pub sessions: Mutex<HashMap<String, SessionState>>,
}

#[tauri::command]
pub fn check_zip_structure(session_id: String, path: &str, state: State<'_, AppState>) -> Result<String> {
    let base_path = Path::new(path);
    let (json_content, _default_memories_dir) = fs::extract_json_from_zip(base_path)?;

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id).or_insert_with(SessionState::new);
    session.export_path = Some(PathBuf::from(path));
    
    Ok(json_content)
}

#[tauri::command]
pub fn initialize_and_load(
    session_id: String,
    output_path: &str,
    items: Vec<MemoryItem>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryItem>> {
    let out_dir = PathBuf::from(output_path);

    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir)?;
    }

    let app_data_dir = app.path().app_data_dir().map_err(|e| format!("Failed to get app_data_dir: {}", e))?;
    let db_dir = app_data_dir.join("sessions");
    if !db_dir.exists() {
        std::fs::create_dir_all(&db_dir)?;
    }
    
    let db_path = db_dir.join(format!("{}.db", session_id));
    let db_manager = DbManager::new(db_path)?;

    // Scan destination folder for previous progress tracking
    fs::hydrate_state_from_folder(&out_dir, &db_manager, &items)?;

    let memories = db_manager.get_all_memories()?;

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id).or_insert_with(SessionState::new);
    session.db = Some(Arc::new(db_manager));
    session.output_dir = Some(out_dir);

    Ok(memories)
}

#[tauri::command]
pub fn get_memories_state(session_id: String, state: State<'_, AppState>) -> Result<Vec<MemoryItem>> {
    let sessions = state.sessions.lock().unwrap();
    if let Some(session) = sessions.get(&session_id) {
        if let Some(db) = session.db.as_ref() {
            return db.get_all_memories();
        }
    }
    Err("Database not initialized yet for this session".into())
}

#[tauri::command]
pub fn reset_application(session_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.remove(&session_id);
    Ok(())
}

#[tauri::command]
pub fn cleanup_database(session_id: String, app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(mut session) = sessions.remove(&session_id) {
        
        // Explicitly unhook sqlite file lock for this session
        session.db = None; 
        
        // Remove the database file from the internal app_data_dir
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            let db_path = app_data_dir.join("sessions").join(format!("{}.db", session_id));
            if db_path.exists() {
                let _ = std::fs::remove_file(db_path);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_pipeline(
    session_id: String,
    concurrency_limit: usize,
    overwrite_existing: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<()> {
    let (dest_dir, db) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
        
        let dest = session.output_dir.clone().ok_or_else(|| "No output directory selected".to_string())?;
        let db = session.db.clone().ok_or_else(|| "DB not initialized".to_string())?;
        
        (dest, db)
    };

    let items_to_process = {
        let all = db.get_all_memories()?;
        all.into_iter()
            .filter(|i| i.state != ProcessingState::Completed || overwrite_existing)
            .collect::<Vec<_>>()
    };

    let pipeline_service = PipelineService::new(db, app, dest_dir, session_id);

    // Spawn a background task to process the pipeline asynchronously
    tokio::spawn(async move {
        if let Err(e) = pipeline_service
            .process_all(items_to_process, concurrency_limit, overwrite_existing)
            .await
        {
            eprintln!("Pipeline background task error: {}", e);
        }
    });

    Ok(())
}
