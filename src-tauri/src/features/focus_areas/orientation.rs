use exif::{Tag, Value};

use super::exiftool::normalized_focus_region;
use super::metadata::{find_field_value, parse_tiff_exif_fields};
use super::types::FocusRegion;

pub(super) fn apply_orientation_to_box(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    orientation: u16,
) -> (f32, f32, f32, f32) {
    match orientation {
        2 => (1.0 - x - width, y, width, height),
        3 => (1.0 - x - width, 1.0 - y - height, width, height),
        4 => (x, 1.0 - y - height, width, height),
        5 => (1.0 - y - height, 1.0 - x - width, height, width),
        6 => (1.0 - y - height, x, height, width),
        7 => (y, x, height, width),
        8 => (y, 1.0 - x - width, height, width),
        _ => (x, y, width, height),
    }
}

fn orientation_from_value(value: &Value) -> Option<u16> {
    match value {
        Value::Short(vals) if !vals.is_empty() => Some(vals[0]),
        Value::Long(vals) if !vals.is_empty() => u16::try_from(vals[0]).ok(),
        _ => None,
    }
}

pub(super) fn get_exif_orientation(file_bytes: &[u8]) -> u16 {
    parse_tiff_exif_fields(file_bytes)
        .and_then(|fields| {
            find_field_value(&fields, Tag::Orientation).and_then(orientation_from_value)
        })
        .filter(|code| (1..=8).contains(code))
        .unwrap_or(1)
}

fn orient_focus_region(region: FocusRegion, orientation: u16) -> Option<FocusRegion> {
    let (x, y, width, height) =
        apply_orientation_to_box(region.x, region.y, region.width, region.height, orientation);
    normalized_focus_region(x, y, width, height, region.kind.clone(), region.is_primary)
}

pub(super) fn orient_focus_regions(
    regions: Vec<FocusRegion>,
    orientation: u16,
) -> Vec<FocusRegion> {
    if orientation <= 1 {
        return regions;
    }

    regions
        .into_iter()
        .filter_map(|region| orient_focus_region(region, orientation))
        .collect()
}
