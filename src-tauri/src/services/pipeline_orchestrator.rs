use crate::db::MemoryRepository;
use crate::error::Result;
use crate::infra::event_emitter::EventEmitter;
use crate::infra::zip_access::ZipAccess;
use crate::models::ProcessingState;
use crate::pipeline::{OverlayItemRef, PipelineService};
use crate::pipeline::PipelineStatusPayload;
use crate::services::session::AppState;
use std::path::PathBuf;
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info};

pub struct PipelineOrchestrator;

impl PipelineOrchestrator {
    pub async fn start_pipeline(
        session_id: String,
        overwrite_existing: bool,
        max_concurrency: Option<usize>,
        output_path: Option<String>,
        app: AppHandle,
        state: State<'_, AppState>,
    ) -> Result<()> {
        let (dest_dir, db, cancel_token, export_paths, cached_main_idx, cached_overlay_idx) = {
            let mut sessions = state.sessions.write().await;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| "Session not found".to_string())?;

            let dest = session
                .output_dir
                .clone()
                .or_else(|| output_path.as_ref().map(PathBuf::from))
                .ok_or_else(|| "No output directory selected.".to_string())?;
            if session.output_dir.is_none() && output_path.is_some() {
                session.output_dir = Some(dest.clone());
            }

            let db = session
                .db
                .clone()
                .ok_or_else(|| "DB not initialized.".to_string())?;

            session.is_cancelled.store(false, Ordering::SeqCst);
            let cancel_token = Arc::clone(&session.is_cancelled);
            let export_paths = session.export_paths.clone();
            // Defensive: only process ZIPs that still exist on disk.
            // If the UI removed a ZIP (and the backend was updated), it won't be here.
            // If a file was deleted/moved, ignore it instead of keeping stale paths.
            let export_paths = export_paths
                .into_iter()
                .filter(|p| p.exists())
                .collect::<Vec<_>>();

            (
                dest,
                db,
                cancel_token,
                export_paths,
                session.main_index.clone(),
                session.overlay_index.clone(),
            )
        };

        // If restarting after a pause or failure, reset items back to Pending
        if !overwrite_existing {
            db.update_states(ProcessingState::Paused, ProcessingState::Pending)
                .await?;
            db.update_states(ProcessingState::Failed, ProcessingState::Pending)
                .await?;
        }

        let items_to_process = {
            let all = db.get_all_memories().await?;
            all.into_iter()
                .filter(|i| i.state != ProcessingState::Completed || overwrite_existing)
                .collect::<Vec<_>>()
        };

        let http_client = reqwest::Client::new();
        let max_concurrency = max_concurrency
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            });

        // Resource-specific concurrency limits.
        // Keep them bounded to avoid IO thrash and too many ffmpeg processes.
        let download_limit = (max_concurrency / 2).max(2);
        let io_limit = max_concurrency.max(1);
        let ffmpeg_limit = (max_concurrency / 2).max(1);
        let download_sem = Arc::new(tokio::sync::Semaphore::new(download_limit));
        let io_sem = Arc::new(tokio::sync::Semaphore::new(io_limit));
        let ffmpeg_sem = Arc::new(tokio::sync::Semaphore::new(ffmpeg_limit));

        // Spawn a background task to process the pipeline with auto-retry
        const MAX_RETRY_PASSES: u32 = 3;
        tokio::spawn(async move {
            let _ = app.emit(&format!("pipeline-started-{}", session_id), ());

            // Build (or reuse) ZIP indexes AFTER backup starts, so `start_pipeline` returns quickly.
            // Privacy: Only indexes the explicitly provided `export_paths` for this session.
            let mut export_zip_index = cached_main_idx;
            let mut export_overlay_index = cached_overlay_idx;
            if export_zip_index.is_none() && export_overlay_index.is_none() && !export_paths.is_empty() {
                let _ = app.emit(
                    &format!("pipeline-status-{}", session_id),
                    PipelineStatusPayload { message: "Indexing source archives...".into() },
                );

                let main_ids: Vec<String> = items_to_process
                    .iter()
                    .flat_map(|i| {
                        i.segment_ids
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|| vec![i.id.clone()])
                    })
                    .collect();
                let overlay_refs: Vec<OverlayItemRef> = items_to_process
                    .iter()
                    .map(|i| OverlayItemRef {
                        id: i.id.clone(),
                        segment_ids: i.segment_ids.clone(),
                        candidate_ids: i.candidate_ids.clone(),
                    })
                    .collect();
                let export_paths_clone = export_paths.clone();
                let app_clone = app.clone();
                let session_id_clone = session_id.clone();

                match tauri::async_runtime::spawn_blocking(move || {
                    crate::pipeline::build_pipeline_zip_indexes(
                        Some(&app_clone),
                        &session_id_clone,
                        &export_paths_clone,
                        &main_ids,
                        &overlay_refs,
                    )
                })
                .await
                {
                    Ok(Ok((main_idx_raw, overlay_idx_raw))) => {
                        export_zip_index = if main_idx_raw.is_empty() {
                            None
                        } else {
                            Some(Arc::new(main_idx_raw))
                        };
                        export_overlay_index = if overlay_idx_raw.is_empty() {
                            None
                        } else {
                            Some(Arc::new(overlay_idx_raw))
                        };

                        // Cache for future resumes within this session.
                        let app_state = app.state::<AppState>();
                        let mut sessions = app_state.sessions.write().await;
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.main_index = export_zip_index.clone();
                            session.overlay_index = export_overlay_index.clone();
                        }

                        let _ = app.emit(
                            &format!("pipeline-status-{}", session_id),
                            PipelineStatusPayload { message: "Indexing complete.".into() },
                        );
                    }
                    Ok(Err(e)) => {
                        let _ = app.emit(
                            &format!("pipeline-status-{}", session_id),
                            PipelineStatusPayload { message: format!("Indexing failed: {}", e) },
                        );
                        // Proceed without indexes; pipeline can still attempt CDN for missing items.
                    }
                    Err(e) => {
                        let _ = app.emit(
                            &format!("pipeline-status-{}", session_id),
                            PipelineStatusPayload { message: format!("Indexing task failed: {}", e) },
                        );
                    }
                }
            }

            let _ = app.emit(
                &format!("pipeline-status-{}", session_id),
                PipelineStatusPayload { message: "Processing your memories...".into() },
            );

            let zip_access = ZipAccess::new();
            let emitter = EventEmitter::new(app.clone(), session_id.clone());
            let pipeline_service = PipelineService::new(
                db.clone(),
                app.clone(),
                emitter,
                dest_dir,
                export_paths,
                export_zip_index,
                export_overlay_index,
                session_id.clone(),
                cancel_token.clone(),
                http_client,
                max_concurrency,
                download_sem,
                io_sem,
                ffmpeg_sem,
                zip_access,
            );

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
            info!("pipeline background task finished");
        });

        Ok(())
    }

    pub async fn pause_pipeline(session_id: String, state: State<'_, AppState>) -> Result<()> {
        let db = {
            let sessions = state.sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                session.is_cancelled.store(true, Ordering::SeqCst);
                session.db.clone()
            } else {
                None
            }
        };

        if let Some(db) = db {
            // Bulk update any items still Pending
            db.update_states(ProcessingState::Pending, ProcessingState::Paused)
                .await?;
        }

        Ok(())
    }
}

