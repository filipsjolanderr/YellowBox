//! Acquire stage: 1. Check memories folder first, 2. Source ZIP, 3. CDN download.
//! For split videos (multiple segments), extracts and concatenates with ffmpeg.
//! Uses direct lookup by date|segment_id - no canonical substitution.

use crate::db::MemoryRepository;
use crate::error::Result;
use crate::metadata;
use crate::models::{ProcessingState};
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::zip::{find_raw_file_fast, extract_id_from_filename, id_appears_as_token, scan_main_by_date_and_id};
use crate::downloader;
use std::path::{Path, PathBuf};
use tauri_plugin_shell::ShellExt;
use tracing::info;

pub(crate) async fn do_acquire_step<R: MemoryRepository>(
    ctx: &PipelineContext<R>,
    msg: &mut PipelineMessage,
) -> Result<()> {
    if msg.item.state == ProcessingState::Pending {
        if let Some(path) = find_raw_file_fast(&ctx.dest_dir, &msg.item.id) {
            info!(id = %msg.item.id, path = %path.display(), "acquire: found in memories folder");
            msg.raw_path = Some(path);
            return Ok(());
        }
        if let Some(ref zip_path) = ctx.export_path {
            match try_extract_from_source_zip(ctx, &msg.item, zip_path).await {
                Ok(found_path) => {
                    info!(id = %msg.item.id, path = %found_path.display(), "acquire: extracted from source ZIP");
                    msg.raw_path = Some(found_path);
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        info!(id = %msg.item.id, "acquire: downloading from CDN");
        let path = downloader::download_memory(&ctx.http_client, &msg.item, &ctx.dest_dir).await
            .map_err(|e| crate::error::AppError::Message(e))?;
        info!(id = %msg.item.id, path = %path.display(), "acquire: download complete");
        msg.raw_path = Some(path);
        return Ok(());
    }
    if let Some(path) = find_raw_file_fast(&ctx.dest_dir, &msg.item.id) {
        info!(id = %msg.item.id, path = %path.display(), "acquire: found existing raw (non-pending)");
        msg.raw_path = Some(path);
        return Ok(());
    }
    if msg.item.state != ProcessingState::Pending && msg.item.state != ProcessingState::Acquired {
        msg.raw_path = Some(PathBuf::new());
        return Ok(());
    }
    Err(crate::error::AppError::MissingFile(format!(
        "Raw file for {} not found in state {:?}",
        msg.item.id, msg.item.state
    )))
}

/// Extracts main media and overlay from source ZIP. Uses item's own segment_ids and date.
/// Direct lookup only - no canonical substitution.
pub(crate) async fn try_extract_from_source_zip<R: MemoryRepository>(
    ctx: &PipelineContext<R>,
    item: &crate::models::MemoryItem,
    zip_path: &PathBuf,
) -> Result<PathBuf> {
    let segment_ids: Vec<String> = item
        .segment_ids
        .as_ref()
        .filter(|s| s.len() > 1)
        .cloned()
        .unwrap_or_else(|| vec![item.id.clone()]);
    let clean_date = metadata::get_clean_date_prefix(&item.original_date);
    let overlay_clean_date = clean_date.clone();

    let zip_path = zip_path.clone();
    let dest_dir = ctx.dest_dir.clone();
    let dest_dir_clone = dest_dir.clone();
    let main_index = ctx.export_zip_index.clone();
    let overlay_index = ctx.export_overlay_index.clone();
    let id = item.id.clone();
    let id_for_final = id.clone();

    let segment_paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut paths = Vec::new();
        let mut id_from_main_filename: Option<String> = None;

        if let Some(ref idx) = main_index {
            for (i, seg_id) in segment_ids.iter().enumerate() {
                let key_exact = format!("{}|{}", clean_date, seg_id);
                let key_date_only = if clean_date.len() >= 10 {
                    Some(format!("{}|{}", &clean_date[..10], seg_id))
                } else {
                    None
                };
                let key_fallback = if clean_date.is_empty() {
                    Some(format!("|{}", seg_id))
                } else {
                    None
                };
                let (zip_index, ext) = match idx
                    .get(&key_exact)
                    .or_else(|| key_date_only.as_ref().and_then(|k| idx.get(k)))
                    .or_else(|| key_fallback.as_ref().and_then(|k| idx.get(k)))
                {
                    Some(r) => (r.0, r.1.clone()),
                    None => scan_main_by_date_and_id(
                        &mut archive,
                        &clean_date,
                        if clean_date.len() >= 10 {
                            Some(&clean_date[..10])
                        } else {
                            None
                        },
                        seg_id,
                    )
                    .ok_or_else(|| {
                        format!(
                            "Segment {} not in ZIP index (tried {}) and scan found no match",
                            seg_id, key_exact
                        )
                    })?,
                };
                let mut zip_file = archive.by_index(zip_index).map_err(|e| e.to_string())?;
                let entry_name = zip_file.name().to_string();
                if !id_appears_as_token(&entry_name, seg_id) {
                    return Err(format!(
                        "ZIP entry {} does not contain segment id {} - wrong file would overwrite",
                        entry_name, seg_id
                    ));
                }
                if i == 0 {
                    id_from_main_filename = extract_id_from_filename(&entry_name);
                }
                let out_path = dest_dir.join(format!("{}-seg{}.{}", id, i, ext));
                let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                std::io::copy(&mut zip_file, &mut outfile).map_err(|e| e.to_string())?;
                paths.push(out_path);
            }
            if let Some(ref ov_idx) = overlay_index {
                let overlay_lookup_id = id_from_main_filename.as_deref().unwrap_or(&id);
                let ov_key_exact = format!("{}|{}", overlay_clean_date, overlay_lookup_id);
                let ov_key_date_only = if overlay_clean_date.len() >= 10 {
                    Some(format!("{}|{}", &overlay_clean_date[..10], overlay_lookup_id))
                } else {
                    None
                };
                let ov_key_fallback = if overlay_clean_date.is_empty() {
                    Some(format!("|{}", overlay_lookup_id))
                } else {
                    None
                };
                let ov_entry = ov_idx
                    .get(&ov_key_exact)
                    .or_else(|| ov_key_date_only.as_ref().and_then(|k| ov_idx.get(k)))
                    .or_else(|| ov_key_fallback.as_ref().and_then(|k| ov_idx.get(k)));
                if let Some(&(ov_zip_index, ref ov_ext)) = ov_entry {
                    if let Ok(mut ov_file) = archive.by_index(ov_zip_index) {
                        let ov_path = dest_dir.join(format!("{}-overlay.{}", id, ov_ext));
                        if let Ok(mut ov_out) = std::fs::File::create(&ov_path) {
                            let _ = std::io::copy(&mut ov_file, &mut ov_out);
                        }
                    }
                }
            }
            Ok(paths)
        } else {
            Err("No main index".to_string())
        }
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
    .map_err(|e| crate::error::AppError::Message(e))?;

    let (_, ext) = item.generated_filename_and_ext();
    let final_path = dest_dir_clone.join(format!("{}-raw.{}", id_for_final, ext));

    if segment_paths.len() == 1 {
        if tokio::fs::rename(&segment_paths[0], &final_path).await.is_err() {
            tokio::fs::copy(&segment_paths[0], &final_path).await
                .map_err(|e| crate::error::AppError::Message(e.to_string()))?;
            let _ = tokio::fs::remove_file(&segment_paths[0]).await;
        }
    } else if segment_paths.len() > 1 {
        concat_video_segments(&ctx.app, &segment_paths, &final_path).await?;
        for p in &segment_paths {
            let _ = tokio::fs::remove_file(p).await;
        }
    } else {
        return Err(crate::error::AppError::Message("No segments extracted".to_string()));
    }

    Ok(final_path)
}

async fn concat_video_segments(
    app: &tauri::AppHandle,
    segment_paths: &[PathBuf],
    output_path: &Path,
) -> Result<()> {
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ffmpeg_concat_list");
    let list_filename = format!("{}.txt", stem);

    let list_path = output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(list_filename);
    let list_content: String = segment_paths
        .iter()
        .map(|p| {
            let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            format!("file '{}'", abs.to_string_lossy().replace('\\', "/"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    tokio::fs::write(&list_path, &list_content).await
        .map_err(|e| crate::error::AppError::Message(e.to_string()))?;

    let list_path = list_path.clone();
    let output_path = output_path.to_path_buf();

    let output = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| crate::error::AppError::Message(e.to_string()))?
        .args([
            "-f", "concat", "-safe", "0",
            "-i", &list_path.to_string_lossy(),
            "-c", "copy", "-y",
            &output_path.to_string_lossy(),
        ])
        .output()
        .await
        .map_err(|e| crate::error::AppError::Message(e.to_string()))?;

    let _ = tokio::fs::remove_file(&list_path).await;

    if output.status.success() {
        Ok(())
    } else {
        Err(crate::error::AppError::Message(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}
