use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use crate::error::{AppError, Result};

pub async fn generate_thumbnail(
    app: &AppHandle,
    src_path: &Path,
    dest_path: &Path,
    is_video: bool,
) -> Result<()> {
    let src_path = src_path.to_owned();
    let dest_path = dest_path.to_owned();
    let app = app.clone();

    if is_video {
        generate_video_thumbnail(&app, &src_path, &dest_path).await
    } else {
        generate_image_thumbnail(&src_path, &dest_path).await
    }
}

async fn generate_image_thumbnail(src: &Path, dest: &Path) -> Result<()> {
    let src = src.to_owned();
    let dest = dest.to_owned();

    tokio::task::spawn_blocking(move || {
        let img = image::open(&src)
            .map_err(|e| AppError::Image(e))?;
        
        // Thumbnail size (9:16 vertical)
        let thumbnail = img.thumbnail(400, 711);
        
        thumbnail.save(&dest)
            .map_err(|e| AppError::Image(e))?;
        
        Ok(())
    }).await.map_err(|e| AppError::Internal(e.to_string()))?
}

async fn generate_video_thumbnail(app: &AppHandle, src: &Path, dest: &Path) -> Result<()> {
    let command = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| AppError::Internal(format!("Sidecar error: {}", e)))?
        .args([
            "-ss", "00:00:00.500", // at 0.5s
            "-i", &src.to_string_lossy(),
            "-vframes", "1",
            "-q:v", "2",
            "-s", "400x711", // 9:16 vertical scale
            "-f", "image2",
            "-y",
            &dest.to_string_lossy(),
        ]);

    let output = command.output().await.map_err(|e| AppError::Internal(format!("FFmpeg execution error: {}", e)))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AppError::Message(format!("FFmpeg thumb error: {}", stderr)))
    }
}
