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
