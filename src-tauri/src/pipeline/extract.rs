//! Extract stage: unpack ZIP or resolve extracted paths.

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::extractor;
use crate::metadata;
use crate::models::ProcessingState;
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use std::path::{Path, PathBuf};
use tracing::info;

/// Tries to find overlay file (.png or .jpg) - acquire may write either extension.
fn try_find_overlay(dest_dir: &Path, id: &str, clean_date: &str) -> Option<PathBuf> {
    for ext in ["png", "jpg"] {
        let with_date = dest_dir.join(format!("{}_{}-overlay.{}", clean_date, id, ext));
        let only_id = dest_dir.join(format!("{}-overlay.{}", id, ext));
        if with_date.exists() {
            return Some(with_date);
        }
        if only_id.exists() {
            return Some(only_id);
        }
    }
    None
}

/// Tries to find main file, optionally with alternate extension (e.g. .jpg for videos with wrong ext).
fn try_find_main(
    dest_dir: &Path,
    id: &str,
    clean_date: &str,
    ext: &str,
    alt_ext: Option<&str>,
) -> Option<PathBuf> {
    let try_ext = |e: &str| {
        let with_date = dest_dir.join(format!("{}_{}-main.{}", clean_date, id, e));
        let only_id = dest_dir.join(format!("{}-main.{}", id, e));
        if with_date.exists() {
            Some(with_date)
        } else if only_id.exists() {
            Some(only_id)
        } else {
            None
        }
    };
    try_ext(ext).or_else(|| alt_ext.and_then(|a| try_ext(a)))
}

pub(crate) async fn do_extract_step<R: MemoryRepository>(
    ctx: &PipelineContext<R>,
    msg: &mut PipelineMessage,
) -> Result<()> {
    let raw_path = msg
        .raw_path
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Message("No raw path".to_string()))?;
    for _ in 0..2 {
        if msg.item.state == ProcessingState::Acquired {
            if raw_path.exists() {
                info!(id = %msg.item.id, raw = %raw_path.display(), "extract: unpacking ZIP");
                let extracted = extractor::extract_memory(raw_path, &msg.item.id, &ctx.dest_dir)
                    .await
                    .map_err(|e| crate::error::AppError::Extraction(e))?;
                info!(id = %msg.item.id, main = %extracted.0.display(), "extract: unpack complete");
                msg.extracted_files = Some(extracted);
                return Ok(());
            }
            return Err(crate::error::AppError::MissingFile(
                "Downloaded zip missing".to_string(),
            ));
        }
        let (_, ext) = msg.item.generated_filename_and_ext();
        let clean_date = metadata::get_clean_date_prefix(&msg.item.original_date);
        let is_video = msg.item.media_type.eq_ignore_ascii_case("Video");
        let alt_ext = if is_video && ext == "mp4" {
            Some("jpg") // video may have wrong ext in source
        } else {
            None
        };
        let main = match try_find_main(
            &ctx.dest_dir,
            &msg.item.id,
            &clean_date,
            &ext,
            alt_ext.as_deref(),
        ) {
            Some(m) => m,
            None => {
                info!(id = %msg.item.id, "extract: main not found, retrying as Acquired");
                msg.item.state = ProcessingState::Acquired;
                continue;
            }
        };
        let overlay = try_find_overlay(&ctx.dest_dir, &msg.item.id, &clean_date);
        info!(id = %msg.item.id, main = %main.display(), has_overlay = overlay.is_some(), "extract: found existing extracted files");
        msg.extracted_files = Some((main, overlay));
        return Ok(());
    }
    Err(crate::error::AppError::Extraction(
        "Failed to extract or find extracted files after reset".to_string(),
    ))
}
