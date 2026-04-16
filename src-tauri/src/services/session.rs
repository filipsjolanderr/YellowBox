use crate::db::{DbManager, MemoryRepository};
use crate::error::Result;
use crate::fs;
use crate::models::MemoryItem;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::AtomicBool,
    Arc,
};
use tokio::sync::RwLock;
use tauri::{AppHandle, State, Manager};
use tracing::info;

pub struct SessionState {
    pub db: Option<Arc<DbManager>>,
    pub export_paths: Vec<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub is_cancelled: Arc<AtomicBool>,
    pub main_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
    pub overlay_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            db: None,
            export_paths: Vec::new(),
            output_dir: None,
            is_cancelled: Arc::new(AtomicBool::new(false)),
            main_index: None,
            overlay_index: None,
        }
    }
}

pub fn apply_export_paths(session: &mut SessionState, new_paths: Vec<PathBuf>) -> bool {
    if session.export_paths == new_paths {
        return false;
    }

    session.export_paths = new_paths;
    // ZIP list changed; cached indexes may be incomplete/outdated.
    session.main_index = None;
    session.overlay_index = None;
    true
}

pub struct AppState {
    pub sessions: RwLock<HashMap<String, SessionState>>,
}

pub struct SessionService;

impl SessionService {
    pub async fn check_zip_structure(
        session_id: String,
        path: String,
        state: State<'_, AppState>,
    ) -> Result<Vec<MemoryItem>> {
        info!(session_id = %session_id, path = %path, "check_zip_structure");
        let path_buf = PathBuf::from(&path);
        let path_clone = path_buf.clone();

        let content = tauri::async_runtime::spawn_blocking(move || {
            fs::extract_json_from_zip(Path::new(&path_clone))
        })
        .await
        .map_err(|e| e.to_string())??;

        let (json_content, _mem_dir) = content;

        let mut sessions = state.sessions.write().await;
        let session = sessions
            .entry(session_id.clone())
            .or_insert_with(SessionState::new);

        if !session.export_paths.contains(&path_buf) {
            let mut next = session.export_paths.clone();
            next.push(path_buf);
            apply_export_paths(session, next);
        }

        let items = if let Some(content) = json_content {
            crate::parser::parse_memories_json(&content)
        } else {
            Vec::new()
        };

        info!(session_id = %session_id, items_count = items.len(), "zip structure validated");
        Ok(items)
    }

    /// Replace the ZIP list for the session.
    /// This is the source of truth for what archives the pipeline may read.
    pub async fn set_export_paths(
        session_id: String,
        paths: Vec<String>,
        state: State<'_, AppState>,
    ) -> Result<()> {
        let new_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

        let mut sessions = state.sessions.write().await;
        let session = sessions
            .entry(session_id.clone())
            .or_insert_with(SessionState::new);

        if apply_export_paths(session, new_paths) {
            info!(session_id = %session_id, zips = session.export_paths.len(), "export paths updated");
        }

        Ok(())
    }

    pub async fn initialize_and_load(
        session_id: String,
        output_path: String,
        items: Vec<MemoryItem>,
        app: AppHandle,
        state: State<'_, AppState>,
    ) -> Result<Vec<MemoryItem>> {
        info!(
            session_id = %session_id,
            output_path = %output_path,
            items_count = items.len(),
            "initialize_and_load"
        );
        let out_dir = PathBuf::from(output_path);

        if !out_dir.exists() {
            std::fs::create_dir_all(&out_dir)?;
        }

        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app_data_dir: {}", e))?;
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

        let mut sessions = state.sessions.write().await;
        let session = sessions
            .entry(session_id.clone())
            .or_insert_with(SessionState::new);
        session.db = Some(db_arc);
        session.output_dir = Some(out_dir);
        info!(
            session_id = %session_id,
            memories_count = memories.len(),
            "initialize_and_load complete"
        );
        Ok(memories)
    }

    pub async fn get_memories_state(
        session_id: String,
        state: State<'_, AppState>,
    ) -> Result<Vec<MemoryItem>> {
        let db_clone = {
            let sessions = state.sessions.read().await;
            sessions
                .get(&session_id)
                .and_then(|session| session.db.clone())
        };

        if let Some(db) = db_clone {
            return db.get_all_memories().await;
        }

        Err("Database not initialized yet for this session".into())
    }

    pub async fn reset_application(session_id: String, state: State<'_, AppState>) -> Result<()> {
        let mut sessions = state.sessions.write().await;
        let _ = sessions.remove(&session_id);
        Ok(())
    }

    pub async fn cleanup_database(
        session_id: String,
        app: AppHandle,
        state: State<'_, AppState>,
    ) -> Result<()> {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.remove(&session_id) {
            session.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);

            if let Ok(app_data_dir) = app.path().app_data_dir() {
                let db_path = app_data_dir
                    .join("sessions")
                    .join(format!("{}.db", session_id));
                
                // wait for arc to drop and sqlite to release lock
                for _ in 0..20 {
                    if !db_path.exists() || std::fs::remove_file(&db_path).is_ok() {
                        // Also try to delete WAL and SHM files
                        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
                        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
        Ok(())
    }

    pub async fn clear_all_data(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
        let mut sessions = state.sessions.write().await;
        // Cancel all tasks so they release their Arc<DbManager>
        for session in sessions.values() {
            session.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        // Clear in-memory state
        sessions.clear();

        // Delete all database files
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            let sessions_dir = app_data_dir.join("sessions");
            if sessions_dir.exists() {
                // Retry deleting since pipeline tasks might take a few moments to exit and release file locks
                for _ in 0..20 {
                    if std::fs::remove_dir_all(&sessions_dir).is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
                let _ = std::fs::create_dir_all(&sessions_dir);
            }
        }
        Ok(())
    }

    pub async fn resolve_local_media_paths(
        session_id: String,
        memory_ids: Vec<String>,
        state: State<'_, AppState>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Session not found".to_string())?;
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

    pub fn check_overlay_exists(
        output_dir: String,
        memory_id: String,
        clean_date: String,
    ) -> Result<bool> {
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

    pub async fn retry_item(
        session_id: String,
        item_id: String,
        state: State<'_, AppState>,
    ) -> Result<()> {
        let db = {
            let sessions = state.sessions.read().await;
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| "Session not found".to_string())?;
            session
                .db
                .clone()
                .ok_or_else(|| "DB not initialized".to_string())?
        };

        db.reset_item_state(&item_id).await?;
        Ok(())
    }
}
