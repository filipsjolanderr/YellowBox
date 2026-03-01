use yellowbox_lib::metadata;

#[test]
fn test_metadata_filename_generation_integration() {
    let date_str = "2024-05-15 10:15:30 UTC";
    let id = "integ_test";
    let ext = "png";
    let clean_name = metadata::generate_clean_filename(date_str, id, ext);
    assert_eq!(clean_name, "2024-05-15_10-15-30_integ_test.png");
}

#[test]
fn test_parse_location_valid() {
    assert_eq!(
        metadata::parse_location("40.7128, -74.0060"),
        Some((40.7128, -74.0060))
    );
    assert_eq!(
        metadata::parse_location(" 12.34 , 56.78 "),
        Some((12.34, 56.78))
    );
}

#[test]
fn test_parse_location_invalid() {
    assert_eq!(metadata::parse_location("invalid"), None);
    assert_eq!(metadata::parse_location("40.7128"), None);
    assert_eq!(metadata::parse_location("40.7128, abc"), None);
}

#[test]
fn test_generate_clean_filename_image() {
    let date_str = "2021-12-10 12:55:19 UTC";
    let expected = "2021-12-10_12-55-19_abc123.jpg";
    assert_eq!(
        metadata::generate_clean_filename(date_str, "abc123", "jpg"),
        expected
    );
}

#[test]
fn test_generate_clean_filename_video() {
    let date_str = "2023-01-01 00:00:00 UTC";
    let expected = "2023-01-01_00-00-00_xyz789.mp4";
    assert_eq!(
        metadata::generate_clean_filename(date_str, "xyz789", "mp4"),
        expected
    );
}
#[test]
fn test_generate_clean_filename_no_utc() {
    let date_str = "2024-02-29 23:59:59";
    let expected = "2024-02-29_23-59-59_foo456.mp4";
    assert_eq!(
        metadata::generate_clean_filename(date_str, "foo456", "mp4"),
        expected
    );
}

#[test]
fn test_parse_location_extra_spaces() {
    assert_eq!(
        metadata::parse_location("  80.00 ,  -12.00  "),
        Some((80.0, -12.0))
    );
}

#[test]
fn test_parse_location_snapchat_format() {
    // Snapchat exports use "Latitude, Longitude: lat, lon" format
    assert_eq!(
        metadata::parse_location("Latitude, Longitude: 57.686493, 11.977872"),
        Some((57.686493, 11.977872))
    );
    assert_eq!(
        metadata::parse_location("Latitude, Longitude: 55.6709, 12.576744"),
        Some((55.6709, 12.576744))
    );
}

#[tokio::test]
async fn test_apply_image_location_metadata() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_metadata_image.jpg");

    // 1. Create a blank image using the `image` crate
    let img = image::RgbImage::new(10, 10);
    img.save(&file_path).unwrap();

    // 2. Apply metadata
    let lat = 40.7128;
    let lon = -74.0060;
    metadata::apply_image_location_metadata(&file_path, lat, lon)
        .await
        .unwrap();

    // 3. Read metadata back
    // little_exif might not expose easy getter structs, so we verify by parsing using little_exif 
    // and asserting it compiles, then we can read the raw bytes to ensure our EXIF payload is there.
    let parsed_metadata = little_exif::metadata::Metadata::new_from_path(&file_path);
    assert!(parsed_metadata.is_ok(), "Failed to read the written EXIF metadata!");

    let raw_bytes = std::fs::read(&file_path).unwrap();
    // EXIF header includes 'Exif\0\0'
    let has_exif = raw_bytes.windows(4).any(|window| window == b"Exif");
    assert!(has_exif, "EXIF header not found in the output JPEG");

    let _ = std::fs::remove_file(file_path);
}

#[tokio::test]
async fn test_apply_video_location_metadata() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_metadata_video.mp4");

    // 1. Create a blank video using ffmpeg
    let create_cmd = tokio::process::Command::new("ffmpeg")
        .args([
            "-f", "lavfi",
            "-i", "color=c=black:s=16x16:d=1",
            "-c:v", "libx264",
            "-t", "1",
            "-y",
            file_path.to_str().unwrap()
        ])
        .output()
        .await
        .unwrap();
    
    assert!(create_cmd.status.success(), "Failed to create dummy video!");

    // 2. Apply metadata using our extracted args
    let lat = 40.7128;
    let lon = -74.0060;
    let temp_dest = file_path.with_extension("tmp.mp4");
    
    let args = metadata::get_ffmpeg_location_args(&file_path, &temp_dest, lat, lon);
    
    let output = tokio::process::Command::new("ffmpeg")
        .args(args)
        .output()
        .await
        .unwrap();
        
    assert!(output.status.success(), "Failed to write metadata with ffmpeg!");
    if temp_dest.exists() {
        tokio::fs::rename(&temp_dest, &file_path).await.unwrap();
    }

    // 3. Verify metadata using ffprobe
    let probe_output = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-show_format",
            "-show_streams",
            "-print_format", "json",
            file_path.to_str().unwrap()
        ])
        .output()
        .await
        .unwrap();
        
    let output_str = String::from_utf8(probe_output.stdout).unwrap();
    assert!(output_str.contains("+40.7128-074.0060/"), "Metadata not found in the ffprobe output:\n{}", output_str);

    let _ = std::fs::remove_file(file_path);
}
