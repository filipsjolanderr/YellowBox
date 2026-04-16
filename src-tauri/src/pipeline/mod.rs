//! Pipeline: Acquire → Extract → Combine → Metadata.
//!
//! Processes Snapchat Memories through staged transformation with parallel workers.

mod acquire;
mod combine;
mod extract;
mod metadata_stage;
mod process;
mod types;
mod zip;
mod update_sink;

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::infra::event_emitter::EventEmitter;
use crate::infra::zip_access::ZipAccess;
use crate::models::{MemoryItem, ProcessingState};
use crate::pipeline::process::process_item_full;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::update_sink::UpdateSink;
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tracing::info;

// Re-export public API
pub use zip::{
    build_export_zip_index,
    build_main_media_zip_index,
    build_overlay_zip_index,
    build_pipeline_zip_indexes,
    extract_from_export_zip,
    is_video_ext,
    OverlayItemRef,
};
pub use types::PipelineStatusPayload;

pub struct PipelineService<R: MemoryRepository> {
    pub db: Arc<R>,
    pub app: AppHandle,
    pub emitter: EventEmitter,
    pub dest_dir: PathBuf,
    pub export_paths: Vec<PathBuf>,
    pub export_zip_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
    pub export_overlay_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
    pub session_id: String,
    pub is_cancelled: Arc<AtomicBool>,
    pub http_client: reqwest::Client,
    pub max_concurrency: usize,
    pub download_sem: Arc<tokio::sync::Semaphore>,
    pub io_sem: Arc<tokio::sync::Semaphore>,
    pub ffmpeg_sem: Arc<tokio::sync::Semaphore>,
    pub zip_access: ZipAccess,
}

impl<R: MemoryRepository> Clone for PipelineService<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            emitter: self.emitter.clone(),
            dest_dir: self.dest_dir.clone(),
            export_paths: self.export_paths.clone(),
            export_zip_index: self.export_zip_index.clone(),
            export_overlay_index: self.export_overlay_index.clone(),
            session_id: self.session_id.clone(),
            is_cancelled: Arc::clone(&self.is_cancelled),
            http_client: self.http_client.clone(),
            max_concurrency: self.max_concurrency,
            download_sem: Arc::clone(&self.download_sem),
            io_sem: Arc::clone(&self.io_sem),
            ffmpeg_sem: Arc::clone(&self.ffmpeg_sem),
            zip_access: self.zip_access.clone(),
        }
    }
}

impl<R: MemoryRepository + 'static> PipelineService<R> {
    pub fn new(
        db: Arc<R>,
        app: AppHandle,
        emitter: EventEmitter,
        dest_dir: PathBuf,
        export_paths: Vec<PathBuf>,
        export_zip_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
        export_overlay_index: Option<Arc<HashMap<String, (usize, usize, String)>>>,
        session_id: String,
        is_cancelled: Arc<AtomicBool>,
        http_client: reqwest::Client,
        max_concurrency: usize,
        download_sem: Arc<tokio::sync::Semaphore>,
        io_sem: Arc<tokio::sync::Semaphore>,
        ffmpeg_sem: Arc<tokio::sync::Semaphore>,
        zip_access: ZipAccess,
    ) -> Self {
        Self {
            db,
            app,
            emitter,
            dest_dir,
            export_paths,
            export_zip_index,
            export_overlay_index,
            session_id,
            is_cancelled,
            http_client,
            max_concurrency,
            download_sem,
            io_sem,
            ffmpeg_sem,
            zip_access,
        }
    }

    /// Runs the pipeline: Acquire → Extract → Thumbnail → Combine → Metadata.
    /// Uses N parallel workers for throughput.
    pub async fn run_pipeline(
        &self,
        items: Vec<MemoryItem>,
        overwrite_existing: bool,
    ) -> Result<()> {
        let ctx = PipelineContext {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            emitter: self.emitter.clone(),
            updates: UpdateSink::new(Arc::clone(&self.db), self.emitter.clone()),
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
        };

        let mut items_to_process = Vec::new();
        for item in items {
            if ctx.is_cancelled.load(Ordering::SeqCst) {
                break;
            }
            let mut msg = PipelineMessage {
                item: item.clone(),
                raw_path: None,
                extracted_files: None,
                final_file: None,
            };
            if !overwrite_existing {
                if item.state == ProcessingState::Paused {
                    msg.item.state = ProcessingState::Paused;
                    let _ = ctx
                        .updates
                        .update_state_and_emit(
                            msg.item.clone(),
                            ProcessingState::Paused,
                            None,
                            None,
                            None,
                            None,
                        )
                        .await;
                    continue;
                }
                if item.state == ProcessingState::Completed
                    && item.error_message.is_none()
                {
                    let (clean_name, _) = item.generated_filename_and_ext();
                    let final_path = ctx.dest_dir.join(clean_name);
                    if final_path.exists() {
                        info!(id = %item.id, "item already completed, skipping");
                        continue;
                    }
                }
                if item.state == ProcessingState::Completed {
                    let (clean_name, _) = item.generated_filename_and_ext();
                    let final_path = ctx.dest_dir.join(clean_name);
                    if final_path.exists() {
                        let ext = item.extension.clone();
                        let has_overlay = item.has_overlay;
                        let _ = ctx
                            .updates
                            .update_state_and_emit(
                                item,
                                ProcessingState::Completed,
                                None,
                                ext,
                                Some(has_overlay),
                                None,
                            )
                            .await;
                        continue;
                    }
                }
            }
            if overwrite_existing {
                msg.item.state = ProcessingState::Pending;
            } else if msg.item.state == ProcessingState::Failed {
                msg.item.state = ProcessingState::Pending;
            }
            items_to_process.push(msg);
        }

        let concurrency = self.max_concurrency.max(1);
        let count = items_to_process.len();
        info!(count, concurrency, "pipeline: starting");
        stream::iter(items_to_process.into_iter().map(|msg| {
            let ctx = ctx.clone();
            async move {
                let _ = process_item_full(ctx, msg).await;
            }
        }))
        .buffer_unordered(concurrency)
        .collect::<Vec<()>>()
        .await;

        info!(count, "pipeline: finished");
        Ok(())
    }
}
