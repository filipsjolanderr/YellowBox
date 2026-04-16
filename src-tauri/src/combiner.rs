use std::path::Path;
use tracing::{info, warn};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

pub async fn combine_image(
    main_path: &Path,
    overlay_path: &Path,
    dest_path: &Path,
) -> Result<(), String> {
    let main_path = main_path.to_owned();
    let overlay_path = overlay_path.to_owned();
    let dest_path = dest_path.to_owned();

    tokio::task::spawn_blocking(move || {
        let mut main_img =
            image::open(&main_path).map_err(|e| format!("Main decode error: {}", e))?;

        // Try reading overlay dynamically avoiding strict PNG assertions causing crashes
        match image::ImageReader::open(&overlay_path) {
            Ok(reader) => {
                let formatted = match reader.with_guessed_format() {
                    Ok(f) => Some(f),
                    Err(e) => {
                        warn!(error = %e, "combine_image: format guess failed, retrying without guess");
                        match image::ImageReader::open(&overlay_path) {
                            Ok(r) => Some(r),
                            Err(e2) => {
                                warn!(error = %e2, "combine_image: fallback open failed, skipping overlay");
                                None
                            }
                        }
                    }
                };

                if let Some(formatted) = formatted {
                    match formatted.decode() {
                        Ok(overlay_img) => {
                            image::imageops::overlay(&mut main_img, &overlay_img, 0, 0);
                        }
                        Err(e) => {
                            warn!(error = %e, "combine_image: overlay decode failed, skipping overlay");
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "combine_image: overlay open failed, skipping overlay");
            }
        }

        main_img.save(&dest_path).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn combine_video(
    app: &AppHandle,
    main_path: &Path,
    overlay_path: &Path,
    dest_path: &Path,
) -> Result<(), String> {
    if !overlay_path.exists() {
        return Err(format!(
            "Overlay file not found: {}",
            overlay_path.display()
        ));
    }

    // NOTE: Tauri sidecar .args() is NOT shell-interpreted — paths must NOT be shell-quoted.
    let main_str = main_path.to_string_lossy().into_owned();
    let overlay_str = overlay_path.to_string_lossy().into_owned();
    let dest_str = dest_path.to_string_lossy().into_owned();

    let output = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| e.to_string())?
        .args([
            "-i",
            &main_str,
            "-i",
            &overlay_str,
            "-filter_complex",
            "[1:v][0:v]scale2ref[ov][main];[main][ov]overlay=0:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "copy",
            "-y",
            &dest_str,
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        info!(dest = %dest_path.display(), "combined video with overlay");
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
