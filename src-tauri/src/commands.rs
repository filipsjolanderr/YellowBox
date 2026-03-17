use crate::db::{DbManager, MemoryRepository};
use tracing::{error, info};
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
    pub export_paths: Vec<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub is_cancelled: Arc<AtomicBool>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            db: None,
            export_paths: Vec::new(),
            output_dir: None,
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct AppState {
    pub sessions: Mutex<HashMap<String, SessionState>>,
}

#[tauri::command]
pub async fn check_zip_structure(session_id: String, path: String, state: State<'_, AppState>) -> Result<Option<String>> {
    info!(session_id = %session_id, path = %path, "check_zip_structure");
    let path_buf = PathBuf::from(&path);
    let path_clone = path_buf.clone();
    
    let (json_content, _default_memories_dir) = tokio::task::spawn_blocking(move || {
        fs::extract_json_from_zip(Path::new(&path_clone))
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id.clone()).or_insert_with(SessionState::new);
    if !session.export_paths.contains(&path_buf) {
        session.export_paths.push(path_buf);
    }
    info!(session_id = %session_id, "zip structure validated");
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
    info!(session_id = %session_id, output_path = %output_path, items_count = items.len(), "initialize_and_load");
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

    let mut sessions = state.sessions.lock().unwrap();
    let session = sessions.entry(session_id.clone()).or_insert_with(SessionState::new);
    session.db = Some(db_arc);
    session.output_dir = Some(out_dir);
    info!(session_id = %session_id, memories_count = memories.len(), "initialize_and_load complete");
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
    let _ = sessions.remove(&session_id);
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
    max_concurrency: Option<usize>,
    output_path: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<()> {
    let (dest_dir, db, cancel_token, export_paths) = {
        let mut sessions = state.sessions.lock().unwrap();
        let session = sessions.get_mut(&session_id).ok_or_else(|| "Session not found".to_string())?;
        
        let dest = session.output_dir.clone().or_else(|| {
            output_path.as_ref().map(|p| PathBuf::from(p))
        }).ok_or_else(|| "No output directory selected. Wait for setup to finish, or select the destination folder again.".to_string())?;
        if session.output_dir.is_none() && output_path.is_some() {
            session.output_dir = Some(dest.clone());
        }
        let db = session.db.clone().ok_or_else(|| "DB not initialized. Wait for setup to finish.".to_string())?;
        
        session.is_cancelled.store(false, Ordering::SeqCst);
        let cancel_token = Arc::clone(&session.is_cancelled);
        let export_paths = session.export_paths.clone();
        
        (dest, db, cancel_token, export_paths)
    };

    // If restarting after a pause or failure, reset items back to Pending
    if !overwrite_existing {
        db.update_states(ProcessingState::Paused, ProcessingState::Pending).await?;
        db.update_states(ProcessingState::Failed, ProcessingState::Pending).await?;
    }

    let items_to_process = {
        let all = db.get_all_memories().await?;
        all.into_iter()
            .filter(|i| i.state != ProcessingState::Completed || overwrite_existing)
            .collect::<Vec<_>>()
    };

    // Build main + overlay ZIP indexes for fast extraction (avoids O(n*m) scan per item)
    // Main index needs all segment IDs (split videos have multiple segments); overlay uses primary id
    let (export_zip_index, export_overlay_index) = if !export_paths.is_empty() {
        let main_ids: Vec<String> = items_to_process
            .iter()
            .flat_map(|i| {
                i.segment_ids
                    .as_ref()
                    .map(|s| s.clone())
                    .unwrap_or_else(|| vec![i.id.clone()])
            })
            .collect();
        let overlay_items: Vec<crate::pipeline::OverlayItemRef> = items_to_process
            .iter()
            .map(|i| crate::pipeline::OverlayItemRef {
                id: i.id.clone(),
                segment_ids: i.segment_ids.clone(),
            })
            .collect();
        let export_paths_clone = export_paths.clone();
        let (main_idx, overlay_idx) = tokio::task::spawn_blocking(move || {
            let main = crate::pipeline::build_main_media_zip_index(&export_paths_clone, &main_ids).ok();
            let overlay = crate::pipeline::build_overlay_zip_index(&export_paths_clone, &overlay_items).ok();
            (main, overlay)
        })
        .await
        .unwrap_or((None, None));
        (
            main_idx.and_then(|m| if m.is_empty() { None } else { Some(Arc::new(m)) }),
            overlay_idx.and_then(|i| if i.is_empty() { None } else { Some(Arc::new(i)) }),
        )
    } else {
        (None, None)
    };

    let http_client = reqwest::Client::new();
    let max_concurrency = max_concurrency
        .filter(|&n| n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));

    let pipeline_service = PipelineService::new(
        db.clone(),
        app.clone(),
        dest_dir,
        export_paths,
        export_zip_index,
        export_overlay_index,
        session_id.clone(),
        cancel_token.clone(),
        http_client,
        max_concurrency,
    );

    // Spawn a background task to process the pipeline with auto-retry
    const MAX_RETRY_PASSES: u32 = 3;
    tokio::spawn(async move {
        let _ = app.emit(&format!("pipeline-started-{}", session_id), ());
        let mut current_batch = items_to_process;
        for pass in 0..MAX_RETRY_PASSES {
            let batch = std::mem::take(&mut current_batch);
            if let Err(e) = pipeline_service.run_pipeline(batch, overwrite_existing).await {
                error!(pass = pass + 1, error = %e, "pipeline pass error");
            }
            if cancel_token.load(Ordering::SeqCst) {
                break;
            }
            let failed = match db.get_memories_by_state(ProcessingState::Failed).await {
                Ok(f) => f,
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

// DELETED extract_preview_media command

#[tauri::command]
pub fn resolve_local_media_paths(
    session_id: String,
    memory_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get(&session_id).ok_or_else(|| "Session not found".to_string())?;
    let output_dir = session.output_dir.as_ref();
    let scan_dirs: Vec<PathBuf> = output_dir.into_iter().cloned().collect();
    if scan_dirs.is_empty() {
        return Err("No output directory".into());
    }
    let id_set: std::collections::HashSet<String> = memory_ids.into_iter().collect();
    let result = fs::resolve_local_media_paths_batch(&scan_dirs, &id_set);
    info!(session_id = %session_id, paths_count = result.len(), "resolve_local_media_paths");
    Ok(result)
}

#[tauri::command]
pub fn check_overlay_exists(output_dir: String, memory_id: String, clean_date: String) -> Result<bool> {
    use std::path::Path;
    let base = Path::new(&output_dir);
    for ext in ["png", "jpg"] {
        let id_only = base.join(format!("{}-overlay.{}", memory_id, ext));
        let with_date = base.join(format!("{}_{}-overlay.{}", clean_date, memory_id, ext));
        if id_only.exists() || with_date.exists() {
            return Ok(true);
        }
    }
    Ok(false)
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
