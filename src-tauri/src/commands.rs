use crate::db::{DbManager, MemoryRepository};
use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use crate::pipeline::PipelineService;
use crate::fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use tauri::{AppHandle, State, Manager, Emitter};

pub struct SessionState {
    pub db: Option<Arc<DbManager>>,
    pub export_path: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub is_cancelled: Arc<AtomicBool>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            db: None,
            export_path: None,
            output_dir: None,
            is_cancelled: Arc::new(AtomicBool::new(false)),
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
pub async fn initialize_and_load(
    session_id: String,
    output_path: String,
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
    let db_manager = DbManager::new(db_path).await?;

    // Scan destination folder for previous progress tracking
    fs::hydrate_state_from_folder(&out_dir, &db_manager, &items).await?;

    let memories = db_manager.get_all_memories().await?;

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id).or_insert_with(SessionState::new);
    session.db = Some(Arc::new(db_manager));
    session.output_dir = Some(out_dir);

    Ok(memories)
}

#[tauri::command]
pub async fn get_memories_state(session_id: String, state: State<'_, AppState>) -> Result<Vec<MemoryItem>> {
    let db_clone = {
        let sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.get(&session_id) {
            session.db.clone()
        } else {
            None
        }
    };

    if let Some(db) = db_clone {
        return db.get_all_memories().await;
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
    overwrite_existing: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<()> {
    let (dest_dir, db, cancel_token) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
        
        let dest = session.output_dir.clone().ok_or_else(|| "No output directory selected".to_string())?;
        let db = session.db.clone().ok_or_else(|| "DB not initialized".to_string())?;
        
        session.is_cancelled.store(false, Ordering::SeqCst);
        let cancel_token = Arc::clone(&session.is_cancelled);
        
        (dest, db, cancel_token)
    };

    // If restarting after a pause, reset all Paused items back to Pending
    if !overwrite_existing {
        db.update_states(ProcessingState::Paused, ProcessingState::Pending).await?;
    }

    let items_to_process = {
        let all = db.get_all_memories().await?;
        all.into_iter()
            .filter(|i| i.state != ProcessingState::Completed || overwrite_existing)
            .collect::<Vec<_>>()
    };

    let pipeline_service = PipelineService::new(db.clone(), app.clone(), dest_dir, session_id.clone(), cancel_token);

    // Spawn a background task to process the pipeline asynchronously
    tokio::spawn(async move {
        let _ = app.emit(&format!("pipeline-started-{}", session_id), ());
        if let Err(e) = pipeline_service
            .process_all(items_to_process, overwrite_existing)
            .await
        {
            eprintln!("Pipeline background task error: {}", e);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn pause_pipeline(session_id: String, state: State<'_, AppState>) -> Result<()> {
    let db = {
        let sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.get(&session_id) {
            session.is_cancelled.store(true, Ordering::SeqCst);
            session.db.clone()
        } else {
            None
        }
    };

    if let Some(db) = db {
        // Bulk update any items still Pending
        db.update_states(ProcessingState::Pending, ProcessingState::Paused).await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn retry_item(session_id: String, item_id: String, state: State<'_, AppState>) -> Result<()> {
    let db = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
        session.db.clone().ok_or_else(|| "DB not initialized".to_string())?
    };

    db.reset_item_state(&item_id).await?;
    Ok(())
}
