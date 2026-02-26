use chrono::{DateTime, TimeZone, Utc};
use filetime::FileTime;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

pub async fn set_file_times(path: &Path, date_str: &str) -> Result<(), String> {
    // Parse "2021-12-10 12:55:19 UTC" to a timestamp
    let dt = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S UTC")
        .map_err(|e| format!("Failed to parse date '{}': {}", date_str, e))?;

    let dt_utc: DateTime<Utc> = Utc.from_utc_datetime(&dt);
    let ft = FileTime::from_unix_time(dt_utc.timestamp(), 0);

    filetime::set_file_mtime(path, ft).map_err(|e| e.to_string())?;
    filetime::set_file_atime(path, ft).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn parse_location(location: &str) -> Option<(f32, f32)> {
    let parts: Vec<&str> = location.split(',').collect();
    if parts.len() == 2 {
        let lat = parts[0].trim().parse::<f32>().ok()?;
        let lon = parts[1].trim().parse::<f32>().ok()?;
        Some((lat, lon))
    } else {
        None
    }
}

pub fn get_ffmpeg_location_args(path: &Path, temp_dest: &Path, lat: f32, lon: f32) -> Vec<String> {
    let iso6709_loc = format!("{lat:+08.4}{lon:+09.4}/");
    vec![
        "-i".to_string(),
        path.to_string_lossy().into_owned(),
        "-metadata".to_string(),
        format!("location={}", iso6709_loc),
        "-metadata".to_string(),
        format!("location-eng={}", iso6709_loc),
        "-c".to_string(),
        "copy".to_string(),
        "-y".to_string(),
        temp_dest.to_string_lossy().into_owned(),
    ]
}

pub async fn apply_image_location_metadata(path: &Path, lat: f32, lon: f32) -> Result<(), String> {
    let path_buf = path.to_path_buf();
    let _ = tokio::task::spawn_blocking(move || {
        use little_exif::exif_tag::ExifTag;
        use little_exif::metadata::Metadata;
        use little_exif::rational::uR64;

        let mut metadata = Metadata::new();

        let lat_ref = if lat < 0.0 { "S" } else { "N" };
        let lon_ref = if lon < 0.0 { "W" } else { "E" };

        let lat_abs = lat.abs();
        let lon_abs = lon.abs();

        let lat_deg = lat_abs.trunc() as u32;
        let lat_min = ((lat_abs - lat_deg as f32) * 60.0).trunc() as u32;
        let lat_sec = ((lat_abs - lat_deg as f32 - lat_min as f32 / 60.0) * 3600.0) * 1000.0;

        let lon_deg = lon_abs.trunc() as u32;
        let lon_min = ((lon_abs - lon_deg as f32) * 60.0).trunc() as u32;
        let lon_sec = ((lon_abs - lon_deg as f32 - lon_min as f32 / 60.0) * 3600.0) * 1000.0;

        metadata.set_tag(ExifTag::GPSLatitudeRef(lat_ref.to_string()));
        metadata.set_tag(ExifTag::GPSLongitudeRef(lon_ref.to_string()));

        metadata.set_tag(ExifTag::GPSLatitude(vec![
            uR64 { nominator: lat_deg, denominator: 1 },
            uR64 { nominator: lat_min, denominator: 1 },
            uR64 { nominator: lat_sec as u32, denominator: 1000 },
        ]));
        metadata.set_tag(ExifTag::GPSLongitude(vec![
            uR64 { nominator: lon_deg, denominator: 1 },
            uR64 { nominator: lon_min, denominator: 1 },
            uR64 { nominator: lon_sec as u32, denominator: 1000 },
        ]));

        let _ = metadata.write_to_file(&path_buf);
    })
    .await;

    Ok(())
}

pub async fn apply_location_metadata(
    app: &AppHandle,
    path: &Path,
    location: &str,
    is_video: bool,
) -> Result<(), String> {
    let Some((lat, lon)) = parse_location(location) else {
        return Ok(()); // Invalid location format, just skip
    };

    if is_video {
        // Use FFmpeg for video (mp4, mov)
        let temp_dest = path.with_extension("tmp.mp4");
        let args = get_ffmpeg_location_args(path, &temp_dest, lat, lon);

        let command = app
            .shell()
            .sidecar("ffmpeg")
            .map_err(|e| e.to_string())?
            .args(args);

        let output = command.output().await.map_err(|e| e.to_string())?;

        if output.status.success() && temp_dest.exists() {
            let _ = tokio::fs::rename(&temp_dest, path).await;
        } else {
            let _ = tokio::fs::remove_file(&temp_dest).await;
        }
    } else {
        // Use little_exif for images (jpg) natively
        apply_image_location_metadata(path, lat, lon).await?;
    }

    Ok(())
}

/// Generates a clean output filename replacing spaces and colons with underscores and hyphens.
pub fn generate_clean_filename(date_str: &str, id: &str, ext: &str) -> String {
    let clean_date = date_str
        .replace(" UTC", "")
        .replace(":", "-")
        .replace(" ", "_");
    format!("{}_{}.{}", clean_date, id, ext)
}
