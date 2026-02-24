use std::path::Path;
use filetime::FileTime;
use chrono::{DateTime, Utc, TimeZone};

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

/// Generates a clean output filename replacing spaces and colons with underscores and hyphens.
pub fn generate_clean_filename(date_str: &str, id: &str, ext: &str) -> String {
    let clean_date = date_str.replace(" UTC", "").replace(":", "-").replace(" ", "_");
    format!("{}_{}.{}", clean_date, id, ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_clean_filename_image() {
        let date_str = "2021-12-10 12:55:19 UTC";
        let expected = "2021-12-10_12-55-19_abc123.jpg";
        assert_eq!(generate_clean_filename(date_str, "abc123", "jpg"), expected);
    }

    #[test]
    fn test_generate_clean_filename_video() {
        let date_str = "2023-01-01 00:00:00 UTC";
        let expected = "2023-01-01_00-00-00_xyz789.mp4";
        assert_eq!(generate_clean_filename(date_str, "xyz789", "mp4"), expected);
    }
    #[test]
    fn test_generate_clean_filename_no_utc() {
        let date_str = "2024-02-29 23:59:59";
        let expected = "2024-02-29_23-59-59_foo456.mp4";
        assert_eq!(generate_clean_filename(date_str, "foo456", "mp4"), expected);
    }
}
