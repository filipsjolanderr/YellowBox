use crate::db::MemoryRepository;
use crate::infra::event_emitter::EventEmitter;
use crate::infra::zip_access::ZipAccess;
use crate::models::MemoryItem;
use crate::pipeline::update_sink::UpdateSink;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
    pub emitter: EventEmitter,
    pub updates: UpdateSink<R>,
    pub dest_dir: PathBuf,
    pub export_paths: Vec<PathBuf>,
    /// Index: "date|id" -> (zip_file_index, zip_entry_index, extension)
    pub export_zip_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
    /// Index: "date|primary_id" -> (zip_file_index, zip_entry_index, extension)
    pub export_overlay_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
    pub session_id: String,
    pub is_cancelled: Arc<AtomicBool>,
    pub http_client: reqwest::Client,
    pub download_sem: Arc<tokio::sync::Semaphore>,
    pub io_sem: Arc<tokio::sync::Semaphore>,
    pub ffmpeg_sem: Arc<tokio::sync::Semaphore>,
    pub zip_access: ZipAccess,
}

impl<R: MemoryRepository> Clone for PipelineContext<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            emitter: self.emitter.clone(),
            updates: self.updates.clone(),
            dest_dir: self.dest_dir.clone(),
            export_paths: self.export_paths.clone(),
            export_zip_index: self.export_zip_index.clone(),
            export_overlay_index: self.export_overlay_index.clone(),
            session_id: self.session_id.clone(),
            is_cancelled: Arc::clone(&self.is_cancelled),
            http_client: self.http_client.clone(),
            download_sem: Arc::clone(&self.download_sem),
            io_sem: Arc::clone(&self.io_sem),
            ffmpeg_sem: Arc::clone(&self.ffmpeg_sem),
            zip_access: self.zip_access.clone(),
        }
    }
}
