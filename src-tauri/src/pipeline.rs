use crate::db::MemoryRepository;
use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use crate::{combiner, downloader, extractor, metadata, thumbnailer};
use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use tauri::{AppHandle, Emitter};

pub fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    lower == "mp4" || lower == "mov"
}

pub struct PipelineService<R: MemoryRepository> {
    pub db: Arc<R>,
    pub app: AppHandle,
    pub dest_dir: PathBuf,
    pub export_path: Option<PathBuf>,
    pub session_id: String,
    pub is_cancelled: Arc<AtomicBool>,
}

impl<R: MemoryRepository> Clone for PipelineService<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            app: self.app.clone(),
            dest_dir: self.dest_dir.clone(),
            export_path: self.export_path.clone(),
            session_id: self.session_id.clone(),
            is_cancelled: Arc::clone(&self.is_cancelled),
        }
    }
}

    impl<R: MemoryRepository + 'static> PipelineService<R> {
    pub fn new(db: Arc<R>, app: AppHandle, dest_dir: PathBuf, export_path: Option<PathBuf>, session_id: String, is_cancelled: Arc<AtomicBool>) -> Self {
        Self { db, app, dest_dir, export_path, session_id, is_cancelled }
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
            current_item.state = ProcessingState::Paused;
            let _ = self.db.update_state(&current_item.id, ProcessingState::Paused, None, None, None, None).await;
            let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
            return Ok(current_item);
        }

        if overwrite_existing {
            current_item.state = ProcessingState::Pending;
        } else {
            if current_item.state == ProcessingState::Failed {
                current_item.state = ProcessingState::Pending;
            }

            let (clean_name, _) = current_item.generated_filename_and_ext();
            let final_path = self.dest_dir.join(clean_name);
            if final_path.exists() {
                if current_item.state == ProcessingState::Completed && current_item.error_message.is_none() && current_item.has_thumbnail {
                    return Ok(current_item);
                } else if current_item.state == ProcessingState::Completed {
                    let _ = self.db.update_state(
                        &current_item.id,
                        ProcessingState::Completed,
                        None,
                        current_item.extension.clone(),
                        Some(current_item.has_overlay),
                        Some(current_item.has_thumbnail),
                    ).await;
                    current_item.error_message = None;
                    let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
                    return Ok(current_item);
                }
            }
        }

        macro_rules! update_state {
            ($state:expr, $err:expr) => {
                update_state!($state, $err, None, None, None)
            };
            ($state:expr, $err:expr, $ext:expr) => {
                update_state!($state, $err, $ext, None, None)
            };
            ($state:expr, $err:expr, $ext:expr, $overlay:expr) => {
                update_state!($state, $err, $ext, $overlay, None)
            };
            ($state:expr, $err:expr, $ext:expr, $overlay:expr, $thumb:expr) => {{
                current_item.state = $state;
                current_item.error_message = $err;
                if let Some(ext) = $ext {
                    current_item.extension = Some(ext);
                }
                if let Some(overlay) = $overlay {
                    current_item.has_overlay = overlay;
                }
                if let Some(thumb) = $thumb {
                    current_item.has_thumbnail = thumb;
                }
                let _ = self.db.update_state(
                    &current_item.id,
                    current_item.state.clone(),
                    current_item.error_message.as_deref(),
                    current_item.extension.clone(),
                    Some(current_item.has_overlay),
                    Some(current_item.has_thumbnail),
                ).await;
                let _ = self.app.emit(&format!("memory-updated-{}", self.session_id), current_item.clone());
            }};
        }

        const MAX_ATTEMPTS: u32 = 6;
        let mut last_error = None;
        for attempt in 1..=MAX_ATTEMPTS {
            if self.is_cancelled.load(Ordering::SeqCst) {
                update_state!(ProcessingState::Paused, None);
                return Ok(current_item);
            }

            let result = async {
                let zip_path = self.execute_download_step(&mut current_item).await?;
                if current_item.state == ProcessingState::Pending {
                    update_state!(ProcessingState::Downloaded, None);
                }

                let extracted_files = self.execute_extract_step(&mut current_item, &zip_path).await?;
                if current_item.state == ProcessingState::Downloaded {
                    let ext = extracted_files.0.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
                    let has_overlay = extracted_files.1.is_some();
                    update_state!(ProcessingState::Extracted, None, ext, Some(has_overlay));
                }

                let final_file = self.execute_combine_step(&mut current_item, &extracted_files).await?;
                if current_item.state == ProcessingState::Extracted {
                    update_state!(ProcessingState::Combined, None);
                }

                // Thumbnail step (new)
                if !current_item.has_thumbnail {
                    if let Ok(_) = self.execute_thumbnail_step(&mut current_item, &final_file).await {
                        update_state!(current_item.state.clone(), None, None, None, Some(true));
                    }
                }

                self.execute_metadata_step(&mut current_item, &final_file, &zip_path, &extracted_files).await?;
                if current_item.state == ProcessingState::Combined {
                    update_state!(ProcessingState::Completed, None);
                }

                Ok::<MemoryItem, crate::error::AppError>(current_item.clone())
            }.await;

            match result {
                Ok(item) => return Ok(item),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < MAX_ATTEMPTS {
                        // Exponential backoff: 1s, 2s, 4s, 8s, 16s
                        let delay_ms = 1000 * 2u64.pow(attempt - 1);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        let err_msg = last_error.map(|e| e.to_string()).unwrap_or_else(|| format!("Unknown error after {} attempts", MAX_ATTEMPTS));
        update_state!(ProcessingState::Failed, Some(err_msg.clone()));
        Err(crate::error::AppError::Message(err_msg))
    }

    async fn execute_download_step(&self, item: &mut MemoryItem) -> Result<PathBuf> {
        if item.state == ProcessingState::Pending {
            // 1. Check if it already exists in dest_dir (Resilience for interrupted sessions)
            // Note: downloader.rs saves as {id}-raw.{ext}. We check for any extension.
            let base_name = format!("{}-raw.", item.id);
            if let Ok(mut entries) = tokio::fs::read_dir(&self.dest_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(&base_name) && !name.ends_with(".tmp") {
                            return Ok(entry.path());
                        }
                    }
                }
            }

            // 2. Check source ZIP if provided (Optimization: Export May Already contain media)
            if let Some(ref zip_path) = self.export_path {
                if let Ok(found_path) = self.try_extract_from_source_zip(item, zip_path).await {
                    return Ok(found_path);
                }
            }

            // 3. Fallback: Download from CDN
            let client = reqwest::Client::new();
            Ok(downloader::download_memory(&client, item, &self.dest_dir).await?)
        } else {
            // Already downloaded or beyond. Find the raw file.
            let base_name = format!("{}-raw.", item.id);
            let mut entries = tokio::fs::read_dir(&self.dest_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&base_name) && !name.ends_with(".tmp") {
                        return Ok(entry.path());
                    }
                }
            }

            // If we are already beyond Downloaded state, it's fine if the raw file is gone
            if item.state != ProcessingState::Pending && item.state != ProcessingState::Downloaded {
                return Ok(PathBuf::new());
            }

            Err(crate::error::AppError::MissingFile(format!("Raw file for {} not found in state {:?}", item.id, item.state)))
        }
    }

    async fn try_extract_from_source_zip(&self, item: &MemoryItem, zip_path: &PathBuf) -> Result<PathBuf> {
        let zip_path = zip_path.clone();
        let id = item.id.clone();
        let dest_dir = self.dest_dir.clone();

        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

            // Snapchat usually puts them in memories/ folder within the export zip
            for i in 0..archive.len() {
                let mut zip_file = archive.by_index(i).map_err(|e| e.to_string())?;
                let name = zip_file.name();
                
                // Check if the filename contains the ID
                if name.contains(&id) {
                    let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("zip");
                    let out_path = dest_dir.join(format!("{}-raw.{}", id, ext));
                    
                    let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                    std::io::copy(&mut zip_file, &mut outfile).map_err(|e| e.to_string())?;
                    return Ok(out_path);
                }
            }
            Err("Not found in ZIP".to_string())
        }).await
          .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
          .map_err(|e| crate::error::AppError::Message(e))
    }

    async fn execute_extract_step(&self, item: &mut MemoryItem, zip_path: &PathBuf) -> Result<(PathBuf, Option<PathBuf>)> {
        for _ in 0..2 {
            if item.state == ProcessingState::Downloaded {
                if zip_path.exists() {
                    return Ok(extractor::extract_memory(zip_path, &item.id, &self.dest_dir).await?);
                } else {
                    return Err(crate::error::AppError::MissingFile("Downloaded zip missing".to_string()));
                }
            } else {
                let (_, ext) = item.generated_filename_and_ext();
                let clean_date = crate::metadata::get_clean_date_prefix(&item.original_date);
                
                // Try both the date-prefixed (legacy/consistent) and just the ID (clean) version
                let main_with_date = self.dest_dir.join(format!("{}_{}-main.{}", clean_date, item.id, ext));
                let main_only_id = self.dest_dir.join(format!("{}-main.{}", item.id, ext));
                
                let main = if main_with_date.exists() {
                    main_with_date
                } else if main_only_id.exists() {
                    main_only_id
                } else {
                    item.state = ProcessingState::Downloaded;
                    continue; // Loop back and extract
                };
                
                let overlay_with_date = self.dest_dir.join(format!("{}_{}-overlay.png", clean_date, item.id));
                let overlay_only_id = self.dest_dir.join(format!("{}-overlay.png", item.id));
                let overlay = if overlay_with_date.exists() {
                    Some(overlay_with_date)
                } else if overlay_only_id.exists() {
                    Some(overlay_only_id)
                } else {
                    None
                };
                
                return Ok((main, overlay));
            }
        }
        
        Err(crate::error::AppError::Extraction("Failed to extract or find extracted files after reset".to_string()))
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
                metadata::apply_location_metadata(&self.app, final_file, loc, is_video).await
                    .map_err(|e| crate::error::AppError::Metadata(format!("Failed to apply GPS metadata: {}", e)))?;
            }

            metadata::set_file_times(final_file, &item.original_date).await.map_err(|e| crate::error::AppError::Metadata(e.to_string()))?;

            let _ = tokio::fs::remove_file(&extracted_files.0).await;
            if let Some(overlay_path) = &extracted_files.1 {
                let _ = tokio::fs::remove_file(overlay_path).await;
            }
            
            if zip_path.exists() && zip_path.is_file() {
                let _ = tokio::fs::remove_file(zip_path).await;
            }
        }
        Ok(())
    }

    async fn execute_thumbnail_step(&self, item: &mut MemoryItem, final_file: &PathBuf) -> Result<()> {
        let thumbs_dir = self.dest_dir.join(".thumbs");
        if !thumbs_dir.exists() {
            tokio::fs::create_dir_all(&thumbs_dir).await?;
        }

        let thumb_path = thumbs_dir.join(format!("{}.jpg", item.id));
        
        // If we have an overlay, we MUST refresh the thumbnail from the combined file
        let force_refresh = item.state == ProcessingState::Combined && item.has_overlay;
        
        if !thumb_path.exists() || force_refresh {
            let (_, ext) = item.generated_filename_and_ext();
            let is_video = is_video_ext(&ext);
            thumbnailer::generate_thumbnail(&self.app, final_file, &thumb_path, is_video).await?;
        }

        item.has_thumbnail = true;
        Ok(())
    }
}
