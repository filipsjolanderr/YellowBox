//! Pipeline orchestration: processes one item through all stages.

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::models::ProcessingState;
use crate::pipeline::acquire::do_acquire_step;
use crate::pipeline::combine::do_combine_step;
use crate::pipeline::extract::do_extract_step;
use crate::pipeline::metadata_stage::do_metadata_step;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use std::sync::atomic::Ordering;
use tracing::info;

/// Processes one item through all pipeline stages. Used by parallel workers.
pub(crate) async fn process_item_full<R: MemoryRepository + 'static>(
    ctx: PipelineContext<R>,
    mut msg: PipelineMessage,
) -> Result<()> {
    const MAX_ATTEMPTS: u32 = 6;
    let mut last_error = None;

    let item_id = msg.item.id.clone();
    let mut item_for_fail = msg.item.clone();

    let item_start = std::time::Instant::now();
    info!(id = %item_id, "pipeline: processing item");

    for attempt in 1..=MAX_ATTEMPTS {
        item_for_fail = msg.item.clone();
        if ctx.is_cancelled.load(Ordering::SeqCst) {
            let _ = ctx
                .updates
                .update_state_and_emit(
                    msg.item,
                    ProcessingState::Paused,
                    None,
                    None,
                    None,
                    None,
                )
                .await;
            return Ok(());
        }

        let result = async {
            let stage_start = std::time::Instant::now();
            info!(id = %msg.item.id, attempt, "pipeline: acquire step");
            do_acquire_step(&ctx, &mut msg).await?;
            info!(id = %msg.item.id, attempt, elapsed_ms = stage_start.elapsed().as_millis(), "pipeline: acquire step done");
            if msg.item.state == ProcessingState::Pending {
                msg.item.state = ProcessingState::Acquired;
                // Persist + notify so UI stays live during long runs/resume.
                let _ = ctx
                    .updates
                    .update_state_and_emit(msg.item.clone(), ProcessingState::Acquired, None, None, None, None)
                    .await;
            }

            let stage_start = std::time::Instant::now();
            info!(id = %msg.item.id, attempt, "pipeline: extract step");
            do_extract_step(&ctx, &mut msg).await?;
            info!(id = %msg.item.id, attempt, elapsed_ms = stage_start.elapsed().as_millis(), "pipeline: extract step done");
            if msg.item.state == ProcessingState::Acquired {
                let path_ext = msg.extracted_files.as_ref().and_then(|(p, _)| {
                    p.extension().and_then(|s| s.to_str()).map(|s| s.to_string())
                });
                let has_overlay = msg
                    .extracted_files
                    .as_ref()
                    .map(|(_, o)| o.is_some())
                    .unwrap_or(false);
                msg.item.state = ProcessingState::Unpacked;
                // Videos always use mp4 (path may have wrong ext e.g. .jpg)
                let is_video = msg.item.media_type.eq_ignore_ascii_case("Video");
                msg.item.extension = Some(if is_video {
                    "mp4".to_string()
                } else {
                    path_ext.unwrap_or_else(|| "jpg".to_string())
                });
                msg.item.has_overlay = has_overlay;
                let _ = ctx
                    .updates
                    .update_state_and_emit(
                        msg.item.clone(),
                        ProcessingState::Unpacked,
                        None,
                        msg.item.extension.clone(),
                        Some(msg.item.has_overlay),
                        None,
                    )
                    .await;
            }

            let stage_start = std::time::Instant::now();
            info!(id = %msg.item.id, attempt, "pipeline: combine step");
            do_combine_step(&ctx, &mut msg).await?;
            info!(id = %msg.item.id, attempt, elapsed_ms = stage_start.elapsed().as_millis(), "pipeline: combine step done");
            if msg.item.state == ProcessingState::Unpacked {
                msg.item.state = ProcessingState::Composited;
                let _ = ctx
                    .updates
                    .update_state_and_emit(
                        msg.item.clone(),
                        ProcessingState::Composited,
                        None,
                        msg.item.extension.clone(),
                        Some(msg.item.has_overlay),
                        None,
                    )
                    .await;
            }

            let stage_start = std::time::Instant::now();
            info!(id = %msg.item.id, attempt, "pipeline: metadata step");
            do_metadata_step(&ctx, &msg).await?;
            info!(id = %msg.item.id, attempt, elapsed_ms = stage_start.elapsed().as_millis(), "pipeline: metadata step done");
            if msg.item.state == ProcessingState::Composited {
                msg.item.state = ProcessingState::Completed;
                let _ = ctx
                    .updates
                    .update_state_and_emit(
                        msg.item.clone(),
                        ProcessingState::Completed,
                        None,
                        msg.item.extension.clone(),
                        Some(msg.item.has_overlay),
                        None,
                    )
                    .await;
            }
            Ok::<(), crate::error::AppError>(())
        }
        .await;

        match result {
            Ok(()) => {
                info!(id = %item_id, elapsed_ms = item_start.elapsed().as_millis(), "pipeline: item completed");
                return Ok(());
            }
            Err(e) => {
                info!(id = %item_id, attempt, error = %e, "pipeline: step failed, will retry");
                last_error = Some(e);
                if attempt < MAX_ATTEMPTS {
                    let delay_ms = 1000 * 2u64.pow(attempt - 1);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    if let Some(e) = last_error {
        info!(id = %item_id, error = %e, "pipeline: item failed after all retries");
        let err_msg = e.to_string();
        let mut failed_item = item_for_fail;
        failed_item.state = ProcessingState::Failed;
        failed_item.error_message = Some(err_msg.clone());
        let _ = ctx
            .updates
            .update_state_and_emit(
                failed_item,
                ProcessingState::Failed,
                Some(err_msg.clone()),
                None,
                None,
                None,
            )
            .await;
        Err(crate::error::AppError::Message(err_msg))
    } else {
        Ok(())
    }
}
