use std::path::{Path, PathBuf};
use tokio::task;

/// Extracts a memory ZIP archive and returns the paths to the main media and (optional) overlay file.
pub async fn extract_memory(zip_path: &Path, id: &str, dest_dir: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    let zip_path = zip_path.to_owned();
    let id = id.to_owned();
    let dest_dir = dest_dir.to_owned();

    task::spawn_blocking(move || {
        if let Some(ext) = zip_path.extension() {
            if ext != "zip" {
                let outpath = dest_dir.join(format!("{}-main.{}", id, ext.to_string_lossy()));
                std::fs::copy(&zip_path, &outpath).map_err(|e| e.to_string())?;
                return Ok((outpath, None));
            }
        }

        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        let mut main_file = None;
        let mut overlay_file = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let file_name = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => continue,
            };

            let name_str = file_name.to_string_lossy().to_string();

            if name_str.contains("overlay") {
                let outpath = dest_dir.join(format!("{}-overlay.png", id));
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                overlay_file = Some(outpath);
            } else if name_str.ends_with(".mp4") || name_str.ends_with(".mov") {
                let ext = if name_str.ends_with(".mp4") { "mp4" } else { "mov" };
                let outpath = dest_dir.join(format!("{}-main.{}", id, ext));
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                main_file = Some(outpath);
            } else if name_str.ends_with(".jpg") || name_str.ends_with(".jpeg") || name_str.ends_with(".png") {
                let ext = if name_str.ends_with(".png") { "png" } else { "jpg" };
                let outpath = dest_dir.join(format!("{}-main.{}", id, ext));
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                main_file = Some(outpath);
            } else {
                // If it's something else, just extract it normally
                let outpath = dest_dir.join(name_str);
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
            }
        }

        let main = main_file.ok_or_else(|| "Main media file not found in ZIP".to_string())?;
        Ok((main, overlay_file))
    }).await.map_err(|e| e.to_string())?
}
