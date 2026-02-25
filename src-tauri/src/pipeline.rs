use crate::db::MemoryRepository;
use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use crate::{combiner, downloader, extractor, metadata};
use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    lower == "mp4" || lower == "mov"
}

pub struct PipelineService<R: MemoryRepository> {
    pub db: Arc<R>,
    pub app: AppHandle,
    pub dest_dir: PathBuf,
}

impl<R: MemoryRepository> Clone for PipelineService<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            dest_dir: self.dest_dir.clone(),
        }
    }
}

impl<R: MemoryRepository + 'static> PipelineService<R> {
    pub fn new(db: Arc<R>, app: AppHandle, dest_dir: PathBuf) -> Self {
        Self { db, app, dest_dir }
    }

    pub async fn process_all(&self, items: Vec<MemoryItem>, concurrency_limit: usize, overwrite_existing: bool) -> Result<()> {
        stream::iter(items.into_iter().map(|item| {
            let service = self.clone();
            async move {
                let _ = service.process_item(item, overwrite_existing).await;
            }
        }))
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<()>>()
        .await;

        Ok(())
    }

    async fn process_item(&self, item: MemoryItem, overwrite_existing: bool) -> Result<MemoryItem> {
        let client = reqwest::Client::new();
        let mut current_item = item.clone();

        if overwrite_existing {
            current_item.state = ProcessingState::Pending;
        }

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
                );
                let _ = self.app.emit("memory-updated", current_item.clone());
            }};
        }

        // 1. Download
        let zip_path = if current_item.state == ProcessingState::Pending {
            let path = downloader::download_memory(&client, &current_item, &self.dest_dir).await;
            match path {
                Ok(p) => {
                    update_state!(ProcessingState::Downloaded, None);
                    p
                }
                Err(e) => {
                    update_state!(ProcessingState::Failed, Some(e.to_string()));
                    return Err(e.into());
                }
            }
        } else {
            // If already downloaded, find the zip
            self.dest_dir.join(format!("{}-raw.zip", current_item.id))
        };

        // 2. Extract
        let extracted_files = if current_item.state == ProcessingState::Downloaded {
            if zip_path.exists() {
                let res = extractor::extract_memory(&zip_path, &current_item.id, &self.dest_dir).await;
                match res {
                    Ok(files) => {
                        let ext = files.0.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
                        let has_overlay = files.1.is_some();
                        update_state!(ProcessingState::Extracted, None, ext, Some(has_overlay));
                        files
                    }
                    Err(e) => {
                        update_state!(ProcessingState::Failed, Some(e.to_string()));
                        return Err(e.into());
                    }
                }
            } else {
                update_state!(ProcessingState::Failed, Some("Downloaded zip missing".to_string()));
                return Err("Downloaded zip missing".into());
            }
        } else {
            let ext = current_item.extension.as_deref().unwrap_or_else(|| {
                let url = current_item.download_url.to_lowercase();
                if url.contains(".mp4") || url.contains(".mov") || url.contains("video") {
                    "mp4"
                } else {
                    "jpg"
                }
            });
            let main = self.dest_dir.join(format!("{}-main.{}", current_item.id, ext));
            let overlay = self.dest_dir.join(format!("{}-overlay.png", current_item.id));
            (main, if overlay.exists() { Some(overlay) } else { None })
        };

        // 3. Combine
        let final_file = if current_item.state == ProcessingState::Extracted {
            let (main_path, overlay_path) = extracted_files.clone();
            let ext = current_item.extension.as_deref().unwrap_or_else(|| {
                let url = current_item.download_url.to_lowercase();
                if url.contains(".mp4") || url.contains(".mov") || url.contains("video") {
                    "mp4"
                } else {
                    "jpg"
                }
            });
            let clean_name = metadata::generate_clean_filename(&current_item.original_date, &current_item.id, ext);
            let combined_dest = self.dest_dir.join(clean_name);

            if let Some(overlay) = overlay_path {
                if is_video_ext(ext) {
                    if let Err(e) = combiner::combine_video(&self.app, &main_path, &overlay, &combined_dest).await {
                        update_state!(ProcessingState::Failed, Some(e.to_string()));
                        return Err(e.into());
                    }
                } else {
                    if let Err(e) = combiner::combine_image(&main_path, &overlay, &combined_dest).await {
                        update_state!(ProcessingState::Failed, Some(e.to_string()));
                        return Err(e.into());
                    }
                }
            } else {
                let _ = tokio::fs::copy(main_path, &combined_dest).await;
            }

            update_state!(ProcessingState::Combined, None);
            combined_dest
        } else {
            let ext = current_item.extension.as_deref().unwrap_or_else(|| {
                let url = current_item.download_url.to_lowercase();
                if url.contains(".mp4") || url.contains(".mov") || url.contains("video") {
                    "mp4"
                } else {
                    "jpg"
                }
            });
            let clean_name = metadata::generate_clean_filename(&current_item.original_date, &current_item.id, ext);
            self.dest_dir.join(&clean_name)
        };

        // 4. Metadata
        if current_item.state == ProcessingState::Combined {
            if let Err(e) = metadata::set_file_times(&final_file, &current_item.original_date).await {
                update_state!(ProcessingState::Failed, Some(format!("Metadata Error: {}", e)));
                return Err(e.into());
            }

            // Cleanup intermediate files
            let _ = tokio::fs::remove_file(&extracted_files.0).await;
            if let Some(overlay_path) = &extracted_files.1 {
                let _ = tokio::fs::remove_file(overlay_path).await;
            }
            let _ = tokio::fs::remove_file(&zip_path).await;

            update_state!(ProcessingState::Completed, None);
        }

        Ok(current_item)
    }
}
