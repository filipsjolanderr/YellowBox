use std::path::Path;
use tauri_plugin_shell::ShellExt;
use tauri::AppHandle;

pub async fn combine_image(main_path: &Path, overlay_path: &Path, dest_path: &Path) -> Result<(), String> {
    let main_path = main_path.to_owned();
    let overlay_path = overlay_path.to_owned();
    let dest_path = dest_path.to_owned();

    tokio::task::spawn_blocking(move || {
        let mut main_img = image::open(&main_path).map_err(|e| format!("Main decode error: {}", e))?;
        
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
    }).await.map_err(|e| e.to_string())?
}

pub async fn combine_video(app: &AppHandle, main_path: &Path, overlay_path: &Path, dest_path: &Path) -> Result<(), String> {
    let command = app.shell().sidecar("ffmpeg")
        .map_err(|e| e.to_string())?
        .args([
            "-i", &main_path.to_string_lossy(),
            "-i", &overlay_path.to_string_lossy(),
            "-filter_complex", "[0:v][1:v]overlay=0:0",
            "-c:a", "copy",
            "-y", &dest_path.to_string_lossy()
        ]);
    
    let output = command.output().await.map_err(|e| e.to_string())?;
    
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
