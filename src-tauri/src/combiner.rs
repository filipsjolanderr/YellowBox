use std::path::Path;
use tracing::info;
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
                let formatted = reader.with_guessed_format().unwrap_or_else(|e| {
                    eprintln!("Warning: Format guess error: {}", e);
                    image::ImageReader::open(&overlay_path).unwrap() // Attempt unformatted fallback if guessing fails
                });

                match formatted.decode() {
                    Ok(overlay_img) => {
                        image::imageops::overlay(&mut main_img, &overlay_img, 0, 0);
                    }
                    Err(e) => {
                        eprintln!("Warning: Skipping overlay due to decode error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Skipping overlay due to open error: {}", e);
            }
        }

        main_img.save(&dest_path).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn ffmpeg_path_arg(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.contains(' ') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.into_owned()
    }
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
    let main_arg = ffmpeg_path_arg(main_path);
    let overlay_arg = ffmpeg_path_arg(overlay_path);
    let dest_arg = ffmpeg_path_arg(dest_path);

    let command = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| e.to_string())?
        .args([
            "-i",
            &main_arg,
            "-i",
            &overlay_arg,
            "-filter_complex",
            "[1:v][0:v]scale2ref[ov][main];[main][ov]overlay=0:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "copy",
            "-y",
            &dest_arg,
        ]);

    let output = command.output().await.map_err(|e| e.to_string())?;

    if output.status.success() {
        info!(dest = %dest_path.display(), "combined video with overlay");
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
