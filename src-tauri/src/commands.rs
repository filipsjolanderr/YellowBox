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
    pub preview_dir: Option<PathBuf>,
    pub is_cancelled: Arc<AtomicBool>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            db: None,
            export_path: None,
            output_dir: None,
            preview_dir: None,
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
    let db_arc = Arc::new(db_manager);

    // Proactively generate thumbnails from the ZIP in the background for setup previews
    if let Some(sessions) = state.sessions.lock().ok() {
        if let Some(session) = sessions.get(&session_id) {
            if let Some(ref zip_path) = session.export_path {
                let app_clone = app.clone();
                let zip_clone = zip_path.clone();
                let out_clone = out_dir.clone();
                let db_clone = db_arc.clone();
                let items_clone = memories.clone();
                let sid_clone = session_id.clone();
                
                tokio::spawn(async move {
                    let _ = fs::generate_thumbnails_from_zip(
                        &app_clone,
                        &zip_clone,
                        &out_clone,
                        &db_clone,
                        items_clone,
                        sid_clone
                    ).await;
                });
            }
        }
    }

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id).or_insert_with(SessionState::new);
    session.db = Some(db_arc);
    session.output_dir = Some(out_dir);
    // Keep preview_dir so temp-extracted previews keep working (no reload)

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
        session.db = None;

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
    let (dest_dir, db, cancel_token, export_path) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
        
        let dest = session.output_dir.clone().ok_or_else(|| "No output directory selected".to_string())?;
        let db = session.db.clone().ok_or_else(|| "DB not initialized".to_string())?;
        
        session.is_cancelled.store(false, Ordering::SeqCst);
        let cancel_token = Arc::clone(&session.is_cancelled);
        let export_path = session.export_path.clone();
        
        (dest, db, cancel_token, export_path)
    };

    // If restarting after a pause or failure, reset items back to Pending
    if !overwrite_existing {
        db.update_states(ProcessingState::Paused, ProcessingState::Pending).await?;
        db.update_states(ProcessingState::Failed, ProcessingState::Pending).await?;
    }

    let items_to_process = {
        let all = db.get_all_memories().await?;
        all.into_iter()
            .filter(|i| (i.state != ProcessingState::Completed || !i.has_thumbnail) || overwrite_existing)
            .collect::<Vec<_>>()
    };

    let pipeline_service = PipelineService::new(db.clone(), app.clone(), dest_dir, export_path, session_id.clone(), cancel_token.clone());

    // Spawn a background task to process the pipeline with auto-retry
    const MAX_RETRY_PASSES: u32 = 3;
    tokio::spawn(async move {
        let _ = app.emit(&format!("pipeline-started-{}", session_id), ());
        let mut current_batch = items_to_process;
        for pass in 0..MAX_RETRY_PASSES {
            if let Err(e) = pipeline_service
                .process_all(current_batch.clone(), overwrite_existing)
                .await
            {
                eprintln!("Pipeline pass {} error: {}", pass + 1, e);
            }
            if cancel_token.load(Ordering::SeqCst) {
                break;
            }
            // Check for failed items and retry
            let failed = match db.get_all_memories().await {
                Ok(all) => all.into_iter().filter(|i| i.state == ProcessingState::Failed).collect::<Vec<_>>(),
                Err(_) => break,
            };
            if failed.is_empty() || pass >= MAX_RETRY_PASSES - 1 {
                break;
            }
            let _ = db.update_states(ProcessingState::Failed, ProcessingState::Pending).await;
            current_batch = failed;
            tokio::time::sleep(std::time::Duration::from_secs(2 + pass as u64 * 2)).await;
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
pub fn clear_preview_temp(session_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = sessions.get_mut(&session_id) {
        if let Some(ref preview_dir) = session.preview_dir.take() {
            let _ = std::fs::remove_dir_all(preview_dir);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn extract_preview_media(
    session_id: String,
    zip_path: String,
    memory_ids: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String> {
    let app_temp = app.path().temp_dir().map_err(|e| e.to_string())?;
    let preview_dir = fs::extract_preview_to_temp(
        Path::new(&zip_path),
        &memory_ids,
        &app_temp,
    )?;

    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&session_id).ok_or_else(|| "Session not found".to_string())?;
    session.preview_dir = Some(preview_dir.clone());

    Ok(preview_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn resolve_local_media_paths(
    session_id: String,
    memory_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
    let output_dir = session.output_dir.as_ref();
    let preview_dir = session.preview_dir.as_ref();
    let scan_dirs: Vec<&PathBuf> = output_dir
        .into_iter()
        .chain(preview_dir.into_iter())
        .collect();
    if scan_dirs.is_empty() {
        return Err("No output or preview directory".into());
    }
    let mut result = std::collections::HashMap::new();
    for id in memory_ids {
        for dir in &scan_dirs {
            if let Some(path) = fs::resolve_local_media_path(dir, &id) {
                result.insert(id, path.to_string_lossy().to_string());
                break;
            }
        }
    }
    Ok(result)
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
