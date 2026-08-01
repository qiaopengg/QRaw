use std::time::{Duration, UNIX_EPOCH};

use super::cache::FocusCache;
use super::exiftool::{
    focus_kind_from_mode, normalized_focus_region, numbers_from_string,
    orientation_from_exiftool_text,
};
use super::orientation::apply_orientation_to_box;
use super::types::{FocusKind, FocusRegion};

#[test]
fn parses_exiftool_numeric_text() {
    assert_eq!(numbers_from_string("640x480"), vec![640.0, 480.0]);
    assert_eq!(
        numbers_from_string("-120 40, 320"),
        vec![-120.0, 40.0, 320.0]
    );
    assert_eq!(numbers_from_string("E9 (Center)"), vec![9.0]);
}

#[test]
fn clamps_normalized_focus_region_to_visible_image() {
    let region = normalized_focus_region(-0.01, 0.25, 0.05, 0.1, FocusKind::Point, true)
        .expect("partially visible region should be kept");
    assert_eq!(region.x, 0.0);
    assert!((region.width - 0.04).abs() < f32::EPSILON);
}

#[test]
fn infers_focus_kind_from_af_mode() {
    assert_eq!(
        focus_kind_from_mode(Some("Eye Detection AF")),
        FocusKind::Eye
    );
    assert_eq!(
        focus_kind_from_mode(Some("Face + Tracking")),
        FocusKind::Face
    );
    assert_eq!(
        focus_kind_from_mode(Some("Flexible Spot")),
        FocusKind::Point
    );
    assert_eq!(focus_kind_from_mode(Some("Zone AF")), FocusKind::Area);
}

#[test]
fn maps_all_exif_orientation_boxes() {
    let source = (0.1, 0.2, 0.3, 0.4);
    let cases = [
        (1, (0.1, 0.2, 0.3, 0.4)),
        (2, (0.6, 0.2, 0.3, 0.4)),
        (3, (0.6, 0.4, 0.3, 0.4)),
        (4, (0.1, 0.4, 0.3, 0.4)),
        (5, (0.4, 0.6, 0.4, 0.3)),
        (6, (0.4, 0.1, 0.4, 0.3)),
        (7, (0.2, 0.1, 0.4, 0.3)),
        (8, (0.2, 0.6, 0.4, 0.3)),
    ];

    for (orientation, expected) in cases {
        let actual = apply_orientation_to_box(source.0, source.1, source.2, source.3, orientation);
        assert!((actual.0 - expected.0).abs() < 0.0001, "{orientation}: x");
        assert!((actual.1 - expected.1).abs() < 0.0001, "{orientation}: y");
        assert!(
            (actual.2 - expected.2).abs() < 0.0001,
            "{orientation}: width"
        );
        assert!(
            (actual.3 - expected.3).abs() < 0.0001,
            "{orientation}: height"
        );
    }
}

#[test]
fn parses_exiftool_orientation_text() {
    assert_eq!(orientation_from_exiftool_text(Some("6")), 6);
    assert_eq!(orientation_from_exiftool_text(Some("Rotate 90 CW")), 6);
    assert_eq!(orientation_from_exiftool_text(Some("Rotate 270 CW")), 8);
    assert_eq!(orientation_from_exiftool_text(Some("Rotate 180")), 3);
    assert_eq!(orientation_from_exiftool_text(Some("Mirror horizontal")), 2);
    assert_eq!(
        orientation_from_exiftool_text(Some("Mirror horizontal and rotate 270 CW")),
        5
    );
    assert_eq!(
        orientation_from_exiftool_text(Some("Horizontal (normal)")),
        1
    );
}

#[test]
fn invalidates_cache_when_file_modified_time_changes() {
    let cache = FocusCache::new(10);
    let key = "focus_sample".to_string();
    let first_modified = UNIX_EPOCH + Duration::from_secs(10);
    let second_modified = UNIX_EPOCH + Duration::from_secs(20);
    let regions = vec![FocusRegion {
        x: 0.1,
        y: 0.2,
        width: 0.03,
        height: 0.03,
        kind: FocusKind::Point,
        is_primary: true,
    }];

    cache.insert(key.clone(), regions.clone(), first_modified);
    assert_eq!(cache.get(&key, first_modified).unwrap().len(), 1);
    assert!(cache.get(&key, second_modified).is_none());
    assert!(cache.get(&key, first_modified).is_none());
}
