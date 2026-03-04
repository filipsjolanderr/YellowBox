use crate::db::MemoryRepository;
use crate::models::MemoryItem;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

/// Message passed between pipeline stages. Each stage populates the relevant fields.
pub struct PipelineMessage {
    pub item: MemoryItem,
    pub raw_path: Option<PathBuf>,
    pub extracted_files: Option<(PathBuf, Option<PathBuf>)>,
    pub final_file: Option<PathBuf>,
}

/// Shared context for all pipeline stages.
pub struct PipelineContext<R: MemoryRepository> {
    pub db: Arc<R>,
    pub app: AppHandle,
    pub dest_dir: PathBuf,
    pub export_path: Option<PathBuf>,
    pub export_zip_index: Option<Arc<HashMap<String, (usize, String)>>>,
    pub export_overlay_index: Option<Arc<HashMap<String, (usize, String)>>>,
    pub session_id: String,
    pub is_cancelled: Arc<AtomicBool>,
    pub http_client: reqwest::Client,
}

impl<R: MemoryRepository> Clone for PipelineContext<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            dest_dir: self.dest_dir.clone(),
            export_path: self.export_path.clone(),
            export_zip_index: self.export_zip_index.clone(),
            export_overlay_index: self.export_overlay_index.clone(),
            session_id: self.session_id.clone(),
            is_cancelled: Arc::clone(&self.is_cancelled),
            http_client: self.http_client.clone(),
        }
    }
}
