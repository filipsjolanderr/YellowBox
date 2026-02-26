use crate::db::MemoryRepository;
use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use crate::{combiner, downloader, extractor, metadata};
use futures::stream::{self, StreamExt};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use tauri::{AppHandle, Emitter};

fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    lower == "mp4" || lower == "mov"
}

pub struct PipelineService<R: MemoryRepository> {
    pub db: Arc<R>,
    pub app: AppHandle,
    pub dest_dir: PathBuf,
    pub session_id: String,
    pub is_cancelled: Arc<AtomicBool>,
}

impl<R: MemoryRepository> Clone for PipelineService<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            dest_dir: self.dest_dir.clone(),
            session_id: self.session_id.clone(),
            is_cancelled: Arc::clone(&self.is_cancelled),
        }
    }
}

impl<R: MemoryRepository + 'static> PipelineService<R> {
    pub fn new(db: Arc<R>, app: AppHandle, dest_dir: PathBuf, session_id: String, is_cancelled: Arc<AtomicBool>) -> Self {
        Self { db, app, dest_dir, session_id, is_cancelled }
    }

    pub async fn process_all(
        &self,
        items: Vec<MemoryItem>,
        overwrite_existing: bool,
    ) -> Result<()> {
        stream::iter(items.into_iter().map(|item| {
            let service = self.clone();
            async move {
                let _ = service.process_item(item, overwrite_existing).await;
            }
        }))
        .buffer_unordered(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1))
        .collect::<Vec<()>>()
        .await;

        Ok(())
    }

    async fn process_item(&self, item: MemoryItem, overwrite_existing: bool) -> Result<MemoryItem> {
        let mut current_item = item.clone();

        if self.is_cancelled.load(Ordering::SeqCst) || current_item.state == ProcessingState::Paused {
            // Already cancelled before we even started doing work
            current_item.state = ProcessingState::Paused;
            let _ = self.db.update_state(&current_item.id, ProcessingState::Paused, None, None, None).await;
            let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
            return Ok(current_item);
        }

        if overwrite_existing {
            current_item.state = ProcessingState::Pending;
        } else {
            // Early exit if the final file already exists
            let (clean_name, _) = current_item.generated_filename_and_ext();
            let final_path = self.dest_dir.join(clean_name);
            if final_path.exists() {
                if current_item.state != ProcessingState::Completed {
                    // Update DB to reflect completed status
                    let _ = self.db.update_state(
                        &current_item.id,
                        ProcessingState::Completed,
                        None,
                        current_item.extension.clone(),
                        Some(current_item.has_overlay),
                    ).await;
                    current_item.state = ProcessingState::Completed;
                    let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
                }
                return Ok(current_item);
            }
        }

        // We can't move the macro easily since it borrows self and current_item, 
        // but we can pass current_item and self to helper methods and let them return a new state or item.
        // Actually, we can keep the macro and just call the phases. 
        macro_rules! update_state {
            ($state:expr, $err:expr) => {
                update_state!($state, $err, None, None)
            };
            ($state:expr, $err:expr, $ext:expr) => {
                update_state!($state, $err, $ext, None)
            };
            ($state:expr, $err:expr, $ext:expr, $overlay:expr) => {{
                current_item.state = $state;
                current_item.error_message = $err;
                if let Some(ext) = $ext {
                    current_item.extension = Some(ext);
                }
                if let Some(overlay) = $overlay {
                    current_item.has_overlay = overlay;
                }
                let _ = self.db.update_state(
                    &current_item.id,
                    current_item.state.clone(),
                    current_item.error_message.as_deref(),
                    current_item.extension.clone(),
                    Some(current_item.has_overlay),
                ).await;
                let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
            }};
        }

        let zip_path = match self.execute_download_step(&mut current_item).await {
            Ok(p) => {
                if current_item.state == ProcessingState::Pending {
                    update_state!(ProcessingState::Downloaded, None);
                }
                p
            }
            Err(e) => {
                update_state!(ProcessingState::Failed, Some(e.to_string()));
                return Err(e);
            }
        };

        if self.is_cancelled.load(Ordering::SeqCst) {
            update_state!(ProcessingState::Paused, None);
            return Ok(current_item);
        }

        let extracted_files = match self.execute_extract_step(&mut current_item, &zip_path).await {
            Ok(files) => {
                if current_item.state == ProcessingState::Downloaded {
                    let ext = files.0.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
                    let has_overlay = files.1.is_some();
                    update_state!(ProcessingState::Extracted, None, ext, Some(has_overlay));
                }
                files
            }
            Err(e) => {
                update_state!(ProcessingState::Failed, Some(e.to_string()));
                return Err(e);
            }
        };

        if self.is_cancelled.load(Ordering::SeqCst) {
            update_state!(ProcessingState::Paused, None);
            return Ok(current_item);
        }

        let final_file = match self.execute_combine_step(&mut current_item, &extracted_files).await {
            Ok(p) => {
                if current_item.state == ProcessingState::Extracted {
                    update_state!(ProcessingState::Combined, None);
                }
                p
            }
            Err(e) => {
                update_state!(ProcessingState::Failed, Some(e.to_string()));
                return Err(e);
            }
        };

        match self.execute_metadata_step(&mut current_item, &final_file, &zip_path, &extracted_files).await {
            Ok(_) => {
                if current_item.state == ProcessingState::Combined {
                    update_state!(ProcessingState::Completed, None);
                }
            }
            Err(e) => {
                update_state!(ProcessingState::Failed, Some(format!("Metadata Error: {}", e)));
                return Err(e);
            }
        }

        Ok(current_item)
    }

    async fn execute_download_step(&self, item: &mut MemoryItem) -> Result<PathBuf> {
        if item.state == ProcessingState::Pending {
            let client = reqwest::Client::new();
            Ok(downloader::download_memory(&client, item, &self.dest_dir).await?)
        } else {
            Ok(self.dest_dir.join(format!("{}-raw.zip", item.id)))
        }
    }

    async fn execute_extract_step(&self, item: &mut MemoryItem, zip_path: &PathBuf) -> Result<(PathBuf, Option<PathBuf>)> {
        if item.state == ProcessingState::Downloaded {
            if zip_path.exists() {
                Ok(extractor::extract_memory(zip_path, &item.id, &self.dest_dir).await?)
            } else {
                Err(crate::error::AppError::MissingFile("Downloaded zip missing".to_string()))
            }
        } else {
            let (_, ext) = item.generated_filename_and_ext();
            let main = self.dest_dir.join(format!("{}-main.{}", item.id, ext));
            let overlay = self.dest_dir.join(format!("{}-overlay.png", item.id));
            Ok((main, if overlay.exists() { Some(overlay) } else { None }))
        }
    }

    async fn execute_combine_step(&self, item: &mut MemoryItem, extracted_files: &(PathBuf, Option<PathBuf>)) -> Result<PathBuf> {
        let (clean_name, ext) = item.generated_filename_and_ext();
        let combined_dest = self.dest_dir.join(clean_name);

        if item.state == ProcessingState::Extracted {
            let (main_path, overlay_path) = extracted_files.clone();
            if let Some(overlay) = overlay_path {
                if is_video_ext(&ext) {
                    combiner::combine_video(&self.app, &main_path, &overlay, &combined_dest).await?;
                } else {
                    combiner::combine_image(&main_path, &overlay, &combined_dest).await?;
                }
            } else {
                tokio::fs::copy(main_path, &combined_dest).await?;
            }
        }
        Ok(combined_dest)
    }

    async fn execute_metadata_step(&self, item: &mut MemoryItem, final_file: &PathBuf, zip_path: &PathBuf, extracted_files: &(PathBuf, Option<PathBuf>)) -> Result<()> {
        if item.state == ProcessingState::Combined {
            if let Some(ref loc) = item.location {
                let (_, ext) = item.generated_filename_and_ext();
                let is_video = is_video_ext(&ext);
                if let Err(e) = metadata::apply_location_metadata(&self.app, final_file, loc, is_video).await {
                    eprintln!("Warning: Failed to apply location metadata: {}", e);
                }
            }

            metadata::set_file_times(final_file, &item.original_date).await.map_err(|e| crate::error::AppError::Metadata(e.to_string()))?;

            let _ = tokio::fs::remove_file(&extracted_files.0).await;
            if let Some(overlay_path) = &extracted_files.1 {
                let _ = tokio::fs::remove_file(overlay_path).await;
            }
            let _ = tokio::fs::remove_file(zip_path).await;
        }
        Ok(())
    }
}
