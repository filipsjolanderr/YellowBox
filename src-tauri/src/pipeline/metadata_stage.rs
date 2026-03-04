//! Metadata stage: GPS, timestamps, cleanup.

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::metadata;
use crate::models::ProcessingState;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::zip::is_video_ext;
use std::path::Path;
use tracing::info;

/// Removes all intermediate files (raw, main, overlay, seg) for this memory when done.
async fn remove_intermediate_files(dest_dir: &Path, id: &str) {
    let prefixes = [
        format!("{}-raw.", id),
        format!("{}-main.", id),
        format!("{}-overlay.", id),
        format!("{}-seg", id),
    ];
    if let Ok(mut entries) = tokio::fs::read_dir(dest_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(name) = entry.file_name().to_str() {
                let name_lower = name.to_lowercase();
                if prefixes.iter().any(|p| name_lower.starts_with(&p.to_lowercase())) {
                    let path = entry.path();
                    if path.is_file() {
                        let _ = tokio::fs::remove_file(&path).await;
                        info!(id = %id, file = %name, "metadata: removed intermediate file");
                    }
                }
            }
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

        remove_intermediate_files(&ctx.dest_dir, &msg.item.id).await;
        info!(id = %msg.item.id, "metadata: done (GPS, timestamps, cleanup)");
    } else {
        info!(id = %msg.item.id, "metadata: skipped (state not Composited)");
    }
    Ok(())
}
