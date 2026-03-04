use chrono::{DateTime, TimeZone, Utc};
use filetime::FileTime;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::error::AppError;

/// Parses Snapchat date strings in common export formats. Returns None if unparseable.
pub fn parse_date_flexible(date_str: &str) -> Option<DateTime<Utc>> {
    let s = date_str.trim();
    if s.is_empty() {
        return None;
    }
    // RFC3339: 2020-04-28T06:32:52Z
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // 2020-04-28 06:32:52 UTC (chrono's DateTime::parse_from_str can fail on literal " UTC";
    // strip suffix and parse as naive, then assume UTC)
    let s_no_utc = s.strip_suffix(" UTC").unwrap_or(s);
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s_no_utc, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&naive));
    }
    // Apr 28, 2020, 06:32
    if let Ok(dt) = DateTime::parse_from_str(s, "%b %d, %Y, %H:%M") {
        return Some(dt.with_timezone(&Utc));
    }
    None
}

/// Returns Unix timestamp for clustering (memories within N seconds = same video).
pub fn timestamp_seconds(date_str: &str) -> Option<i64> {
    parse_date_flexible(date_str).map(|dt| dt.timestamp())
}

pub async fn set_file_times(path: &Path, date_str: &str) -> crate::error::Result<()> {
    let dt_utc = parse_date_flexible(date_str)
        .ok_or_else(|| AppError::Parse(format!("Failed to parse date '{}'", date_str)))?;

    let ft = FileTime::from_unix_time(dt_utc.timestamp(), 0);

    filetime::set_file_mtime(path, ft)?;
    filetime::set_file_atime(path, ft)?;

    Ok(())
}

pub fn parse_location(location: &str) -> Option<(f32, f32)> {
    // Strip "Latitude, Longitude: " prefix used by Snapchat exports
    let s = location
        .trim()
        .strip_prefix("Latitude, Longitude:")
        .unwrap_or(location)
        .trim();

    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() >= 2 {
        // Take last two parts (handles "lat, lon" and "Latitude, Longitude: lat, lon")
        let lat = parts[parts.len() - 2].parse::<f32>().ok()?;
        let lon = parts[parts.len() - 1].parse::<f32>().ok()?;
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

fn set_gps_tags(metadata: &mut little_exif::metadata::Metadata, lat: f32, lon: f32) {
    use little_exif::exif_tag::ExifTag;
    use little_exif::rational::uR64;

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
}

pub async fn apply_image_location_metadata(path: &Path, lat: f32, lon: f32) -> crate::error::Result<()> {
    let path_buf = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use little_exif::metadata::Metadata;

        // Load existing metadata to preserve it when present; fall back to empty for images without EXIF.
        // Some JPEGs have EXIF that little_exif can read but not write back (e.g. corrupt/malformed tags).
        // If write fails, retry with fresh metadata so we at least get GPS written.
        let mut metadata = Metadata::new_from_path(&path_buf).unwrap_or_else(|_| Metadata::new());
        set_gps_tags(&mut metadata, lat, lon);

        if metadata.write_to_file(&path_buf).is_ok() {
            return Ok(());
        }

        // Fallback: write with fresh metadata (no existing EXIF). Handles JPEGs where
        // loading+rewriting metadata fails (little_exif issue #93).
        let mut fresh = Metadata::new();
        set_gps_tags(&mut fresh, lat, lon);
        fresh
            .write_to_file(&path_buf)
            .map_err(|e| AppError::Metadata(format!("Failed to write EXIF GPS metadata: {}", e)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Metadata task panicked: {}", e)))?
}

fn is_png_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}

pub async fn apply_location_metadata(
    app: &AppHandle,
    path: &Path,
    location: &str,
    is_video: bool,
) -> crate::error::Result<()> {
    let Some((lat, lon)) = parse_location(location) else {
        return Ok(()); // Invalid location format, just skip
    };

    if is_video || is_png_ext(path) {
        // Use FFmpeg for video (mp4, mov) and PNG (little_exif doesn't support PNG)
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
        let temp_ext = if ext.eq_ignore_ascii_case("png") {
            "tmp.png"
        } else {
            "tmp.mp4"
        };
        let temp_dest = path.with_extension(temp_ext);
        let args = get_ffmpeg_location_args(path, &temp_dest, lat, lon);

        let command = app
            .shell()
            .sidecar("ffmpeg")
            .map_err(|e| AppError::Internal(format!("Failed to get ffmpeg sidecar: {}", e)))?
            .args(args);

        let output = command.output().await.map_err(|e| AppError::Internal(format!("Failed to execute ffmpeg: {}", e)))?;

        if output.status.success() && temp_dest.exists() {
            let _ = tokio::fs::rename(&temp_dest, path).await;
        } else {
            let _ = tokio::fs::remove_file(&temp_dest).await;
            if is_video {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AppError::Metadata(format!("FFmpeg metadata application failed: {}", stderr)));
            }
            // PNG: FFmpeg may not support metadata; skip silently
        }
    } else {
        // Use little_exif for JPEG only. Skip on failure (e.g. "Wrong signature" for corrupted
        // or mislabeled files). Pipeline continues; file is exported without GPS in EXIF.
        if let Err(_) = apply_image_location_metadata(path, lat, lon).await {
            // Skip silently - file may be corrupted, wrong format, or little_exif can't handle it
        }
    }

    Ok(())
}

/// Generates a clean output filename. Uses parse_date_flexible when possible so all date formats
/// produce consistent sortable filenames (YYYY-MM-DD_HH-MM-SS).
pub fn generate_clean_filename(date_str: &str, id: &str, ext: &str) -> String {
    let clean_date = get_clean_date_prefix(date_str);
    format!("{}_{}.{}", clean_date, id, ext)
}

/// Normalizes date string to YYYY-MM-DD_HH-MM-SS for filenames. Handles multiple Snapchat export
/// formats; falls back to string replacement if parsing fails.
pub fn get_clean_date_prefix(date_str: &str) -> String {
    if let Some(dt) = parse_date_flexible(date_str) {
        return dt.format("%Y-%m-%d_%H-%M-%S").to_string();
    }
    // Fallback for unparseable dates
    date_str
        .replace(" UTC", "")
        .replace(":", "-")
        .replace(" ", "_")
}
