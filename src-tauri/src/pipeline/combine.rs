//! Combine stage: overlay images/videos.

use crate::combiner;
use crate::db::MemoryRepository;
use crate::error::Result;
use crate::models::ProcessingState;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::zip::is_video_ext;
use tracing::info;

pub(crate) async fn do_combine_step<R: MemoryRepository>(
    ctx: &PipelineContext<R>,
    msg: &mut PipelineMessage,
) -> Result<()> {
    let (main_path, overlay_path) = msg
        .extracted_files
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Message("No extracted files".to_string()))?;
    let (clean_name, ext) = msg.item.generated_filename_and_ext();
    let combined_dest = ctx.dest_dir.join(clean_name);
    if msg.item.state == ProcessingState::Unpacked {
        if let Some(overlay) = overlay_path {
            if is_video_ext(&ext) {
                info!(id = %msg.item.id, "combine: overlaying video");
                let _permit = ctx
                    .ffmpeg_sem
                    .acquire()
                    .await
                    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
                if let Err(e) =
                    combiner::combine_video(&ctx.app, main_path, overlay, &combined_dest).await
                {
                    info!(id = %msg.item.id, err = %e, "combine: video overlay failed, copying main");
                    tokio::fs::copy(main_path, &combined_dest).await?;
                }
            } else {
                info!(id = %msg.item.id, "combine: overlaying image");
                if let Err(e) =
                    combiner::combine_image(main_path, overlay, &combined_dest).await
                {
                    // Main may be a video with wrong extension (e.g. .jpg); try video overlay
                    let video_dest = combined_dest.with_extension("mp4");
                    let _permit = ctx
                        .ffmpeg_sem
                        .acquire()
                        .await
                        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
                    if let Err(ve) =
                        combiner::combine_video(&ctx.app, main_path, overlay, &video_dest).await
                    {
                        info!(id = %msg.item.id, err = %e, video_err = %ve, "combine: image and video overlay failed, copying main");
                        tokio::fs::copy(main_path, &combined_dest).await?;
                    } else {
                        info!(id = %msg.item.id, "combine: image failed but video overlay succeeded (main was video with wrong ext)");
                        msg.item.extension = Some("mp4".to_string());
                        msg.final_file = Some(video_dest);
                        return Ok(());
                    }
                }
            }
        } else {
            info!(id = %msg.item.id, "combine: copying main (no overlay)");
            tokio::fs::copy(main_path, &combined_dest).await?;
        }
    }
    info!(id = %msg.item.id, dest = %combined_dest.display(), "combine: done");
    msg.final_file = Some(combined_dest);
    Ok(())
}
