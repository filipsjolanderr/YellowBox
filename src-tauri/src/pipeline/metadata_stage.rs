//! Metadata stage: GPS, timestamps, cleanup.

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::metadata;
use crate::models::ProcessingState;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::zip::is_video_ext;
use tracing::info;

/// Removes the intermediate files for this pipeline item using the concrete paths
/// already known to the pipeline. Avoids scanning the entire output directory.
async fn cleanup_intermediate_files(msg: &PipelineMessage) {
    // raw file (CDN download or source-ZIP extracted segment)
    if let Some(p) = &msg.raw_path {
        if !p.as_os_str().is_empty() {
            let _ = tokio::fs::remove_file(p).await;
            info!(file = %p.display(), "metadata: removed intermediate file");
        }
    }
    // extracted main and optional overlay
    if let Some((main, overlay)) = &msg.extracted_files {
        // main may equal raw_path (non-zip source) — remove_file is idempotent on ENOENT
        let _ = tokio::fs::remove_file(main).await;
        if let Some(ov) = overlay {
            let _ = tokio::fs::remove_file(ov).await;
        }
    }
}

pub(crate) async fn do_metadata_step<R: MemoryRepository>(
    ctx: &PipelineContext<R>,
    msg: &PipelineMessage,
) -> Result<()> {
    let final_file = msg
        .final_file
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Message("No final file".to_string()))?;
    let _ = msg
        .extracted_files
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Message("No extracted files".to_string()))?;

    if msg.item.state == ProcessingState::Composited {
        info!(id = %msg.item.id, "metadata: applying GPS and timestamps");
        if let Some(ref loc) = msg.item.location {
            let (_, ext) = msg.item.generated_filename_and_ext();
            let is_video = is_video_ext(&ext);
            metadata::apply_location_metadata(&ctx.app, final_file, loc, is_video)
                .await
                .map_err(|e| {
                    crate::error::AppError::Metadata(format!("Failed to apply GPS metadata: {}", e))
                })?;
        }
        metadata::set_file_times(final_file, &msg.item.original_date)
            .await
            .map_err(|e| crate::error::AppError::Metadata(e.to_string()))?;

        cleanup_intermediate_files(msg).await;
        info!(id = %msg.item.id, "metadata: done (GPS, timestamps, cleanup)");
    } else {
        info!(id = %msg.item.id, "metadata: skipped (state not Composited)");
    }
    Ok(())
}
