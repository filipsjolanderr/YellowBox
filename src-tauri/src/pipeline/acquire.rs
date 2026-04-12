//! Acquire stage: 1. Check memories folder first, 2. Source ZIP, 3. CDN download.
//! For split videos (multiple segments), extracts and concatenates with ffmpeg.
//! Uses direct lookup by date|segment_id - no canonical substitution.

use crate::db::MemoryRepository;
use crate::downloader;
use crate::error::Result;
use crate::metadata;
use crate::models::{ProcessingState};
use crate::pipeline::types::{PipelineContext, PipelineMessage};
use crate::pipeline::zip::{
    extract_id_from_filename,
    find_raw_file_fast,
    id_appears_as_token,
    scan_main_by_best_date_match,
    scan_main_by_date_and_id,
};
use std::path::{Path, PathBuf};
use tauri_plugin_shell::ShellExt;
use tracing::info;
use tauri::Emitter;
use std::fs::File;
use zip::ZipArchive;

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
        let base_name = format!("{}-raw.", msg.item.id);
        if let Ok(mut entries) = tokio::fs::read_dir(&ctx.dest_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&base_name) && !name.ends_with(".tmp") {
                        info!(id = %msg.item.id, path = %entry.path().display(), "acquire: found raw file by prefix");
                        msg.raw_path = Some(entry.path());
                        return Ok(());
                    }
                }
            }
        }
        if !ctx.export_paths.is_empty() {
            let _permit = ctx
                .io_sem
                .acquire()
                .await
                .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
            match try_extract_from_source_zip(ctx, &msg.item).await {
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
        let _ = ctx.app.emit(&format!("pipeline-status-{}", ctx.session_id), format!("Downloading {} from CDN...", msg.item.id));
        let _permit = ctx
            .download_sem
            .acquire()
            .await
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
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
    let base_name = format!("{}-raw.", msg.item.id);
    let mut entries = tokio::fs::read_dir(&ctx.dest_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with(&base_name) && !name.ends_with(".tmp") {
                info!(id = %msg.item.id, path = %entry.path().display(), "acquire: found raw by prefix (non-pending)");
                msg.raw_path = Some(entry.path());
                return Ok(());
            }
        }
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
) -> Result<PathBuf> {
    let overall_start = std::time::Instant::now();
    let segment_ids: Vec<String> = item
        .segment_ids
        .as_ref()
        .filter(|s| s.len() > 1)
        .cloned()
        .unwrap_or_else(|| vec![item.id.clone()]);
    let clean_date = metadata::get_clean_date_prefix(&item.original_date);
    let target_epoch = metadata::parse_date_flexible(&item.original_date).map(|dt| dt.timestamp());
    let overlay_clean_date = clean_date.clone();

    let dest_dir = ctx.dest_dir.clone();
    let dest_dir_clone = dest_dir.clone();
    let main_index = ctx.export_zip_index.clone();
    let overlay_index = ctx.export_overlay_index.clone();
    let zip_access = ctx.zip_access.clone();
    let id = item.id.clone();
    let id_for_final = id.clone();

    let export_paths = ctx.export_paths.clone();
    let segment_paths = tokio::task::spawn_blocking(move || -> std::result::Result<Vec<PathBuf>, String> {
        let mut paths = Vec::new();
        let mut id_from_main_filename: Option<String> = None;

        if let Some(ref idx) = main_index {
            info!(dest_dir = %dest_dir.display(), segment_count = segment_ids.len(), "acquire: starting extraction from ZIP pool");
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
                let (zip_file_idx, zip_entry_idx, ext) = match idx
                    .get(&key_exact)
                    .or_else(|| key_date_only.as_ref().and_then(|k| idx.get(k)))
                    .or_else(|| key_fallback.as_ref().and_then(|k| idx.get(k)))
                {
                    Some(r) => (r.0, r.1, r.2.clone()),
                    None => {
                        // Fallback scan across all zips if not in index. 
                        // WARNING: This is O(N) where N is entries in ZIP.
                        let mut found = None;
                        for (z_idx, z_path) in export_paths.iter().enumerate() {
                            tracing::warn!(id = %seg_id, zip = %z_path.display(), "acquire: item not in index, performing full fallback scan. This may be slow!");
                            let file = std::fs::File::open(z_path).map_err(|e| e.to_string())?;
                            let reader = std::io::BufReader::new(file);
                            let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
                                if let Some((e_idx, ext)) = scan_main_by_date_and_id(
                                &mut archive,
                                &clean_date,
                                if clean_date.len() >= 10 {
                                    Some(&clean_date[..10])
                                } else {
                                    None
                                },
                                seg_id,
                            ) {
                                found = Some((z_idx, e_idx, ext));
                                break;
                            }
                        }
                            if found.is_none() {
                                // Last resort: date mismatch between JSON and ZIP filename; pick closest date match by ID.
                                for (z_idx, z_path) in export_paths.iter().enumerate() {
                                    tracing::warn!(id = %seg_id, zip = %z_path.display(), "acquire: date mismatch, performing best-match fallback scan.");
                                    let file = std::fs::File::open(z_path).map_err(|e| e.to_string())?;
                                    let reader = std::io::BufReader::new(file);
                                    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
                                    if let Some((e_idx, ext)) =
                                        scan_main_by_best_date_match(&mut archive, target_epoch, seg_id)
                                    {
                                        found = Some((z_idx, e_idx, ext));
                                        break;
                                    }
                                }
                            }
                        found.ok_or_else(|| {
                            format!(
                                "Segment {} not in ZIP index and scan found no match",
                                seg_id
                            )
                        })?
                    }
                };

                let z_path = export_paths.get(zip_file_idx).ok_or_else(|| "Zip index out of bounds".to_string())?;
                
                // Use shared, blocking ZIP cache (no async locks in blocking context).
                let z_path_owned = z_path.clone();
                let archive_mutex = zip_access
                    .get_or_open(&z_path_owned)
                    .map_err(|e| e.to_string())?;

                {
                    let mut archive_guard = archive_mutex.lock().map_err(|e| e.to_string())?;
                    let entry_name = {
                        let zip_file = archive_guard.by_index(zip_entry_idx).map_err(|e| e.to_string())?;
                        zip_file.name().to_string()
                    };
                    
                    if !id_appears_as_token(&entry_name, seg_id) {
                        return Err(format!(
                            "ZIP entry {} does not contain segment id {} - wrong file would overwrite",
                            entry_name, seg_id
                        ));
                    }
                    if i == 0 {
                        id_from_main_filename = extract_id_from_filename(&entry_name).map(|s| s.to_string());
                    }
                    let out_path = dest_dir.join(format!("{}-seg{}.{}", id, i, ext));
                    info!(out_path = %out_path.display(), "acquire: extracting segment");
                    extract_zip_entry_to_file(&mut archive_guard, zip_entry_idx, &out_path)?;
                    paths.push(out_path);
                }
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
                if let Some(&(ov_zip_file_idx, ov_zip_entry_idx, ref ov_ext)) = ov_entry {
                    let z_path = export_paths.get(ov_zip_file_idx).ok_or_else(|| "Overlay zip index out of bounds".to_string())?;
                    
                    let z_path_owned = z_path.clone();
                    let archive_mutex = zip_access
                        .get_or_open(&z_path_owned)
                        .map_err(|e| e.to_string())?;

                    {
                        let mut archive_guard = archive_mutex.lock().map_err(|e| e.to_string())?;
                        let out_path = dest_dir.join(format!("{}-overlay.{}", id, ov_ext));
                        let mut zip_file = archive_guard.by_index(ov_zip_entry_idx).map_err(|e| e.to_string())?;
                        let mut outfile = File::create(&out_path).map_err(|e| format!("Create file failed ({}): {}", out_path.display(), e))?;
                        std::io::copy(&mut zip_file, &mut outfile).map_err(|e| e.to_string())?;
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
        concat_video_segments(&ctx.app, &ctx.ffmpeg_sem, &segment_paths, &final_path).await?;
        for p in &segment_paths {
            let _ = tokio::fs::remove_file(p).await;
        }
    } else {
        return Err(crate::error::AppError::Message("No segments extracted".to_string()));
    }

    info!(
        id = %item.id,
        segments = segment_paths.len(),
        elapsed_ms = overall_start.elapsed().as_millis(),
        "acquire: extracted from source ZIP"
    );
    Ok(final_path)
}

fn extract_zip_entry_to_file(
    archive: &mut ZipArchive<std::io::BufReader<File>>,
    entry_idx: usize,
    out_path: &Path,
) -> std::result::Result<(), String> {
    let mut zip_file = archive.by_index(entry_idx).map_err(|e| e.to_string())?;
    let mut outfile = File::create(out_path).map_err(|e| format!("Create file failed ({}): {}", out_path.display(), e))?;
    std::io::copy(&mut zip_file, &mut outfile).map_err(|e| e.to_string())?;
    Ok(())
}

async fn concat_video_segments(
    app: &tauri::AppHandle,
    ffmpeg_sem: &tokio::sync::Semaphore,
    segment_paths: &[PathBuf],
    output_path: &Path,
) -> Result<()> {
    let _permit = ffmpeg_sem
        .acquire()
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
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
            let path_str = p.to_string_lossy().replace('\\', "/");
            // On Windows, canonicalize can add \\?\ prefix which ffmpeg may not like.
            // We use the raw path if it's already absolute enough.
            format!("file '{}'", path_str)
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
