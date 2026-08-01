use exif::{Tag, Value};

use super::metadata::{find_field_value, parse_tiff_exif_fields};
use super::types::{FocusKind, FocusRegion};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  标准 EXIF 兜底: SubjectArea / SubjectLocation
//  这是最通用的对焦信息来源 — 几乎所有的相机都可能写入此 tag
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub(super) fn extract_subject_area(
    file_bytes: &[u8],
    image_w: f32,
    image_h: f32,
) -> Vec<FocusRegion> {
    if image_w <= 0.0 || image_h <= 0.0 {
        return Vec::new();
    }

    let fields = match parse_tiff_exif_fields(file_bytes) {
        Some(f) => f,
        None => return Vec::new(),
    };

    if let Some(value) = find_field_value(&fields, Tag::SubjectArea) {
        if let Value::Short(vals) = value {
            if vals.len() >= 2 {
                // SubjectArea 坐标基于 EXIF 图像尺寸（与 normalize 维度一致）
                let x = vals[0] as f32 / image_w;
                let y = vals[1] as f32 / image_h;
                if x < 1.0 && y < 1.0 {
                    let w = if vals.len() >= 4 {
                        (vals[2] as f32 / image_w).max(0.02)
                    } else {
                        0.05
                    };
                    let h = if vals.len() >= 4 {
                        (vals[3] as f32 / image_h).max(0.02)
                    } else {
                        0.05
                    };
                    log::info!(
                        "SubjectArea → AF: ({:.4},{:.4},{:.4},{:.4}), image={:.0}x{:.0}",
                        x,
                        y,
                        w,
                        h,
                        image_w,
                        image_h
                    );
                    return vec![FocusRegion {
                        x,
                        y,
                        width: w,
                        height: h,
                        kind: FocusKind::Area,
                        is_primary: true,
                    }];
                }
            }
        }
    }

    if let Some(value) = find_field_value(&fields, Tag::SubjectLocation) {
        if let Value::Short(vals) = value {
            if vals.len() >= 2 {
                let x = vals[0] as f32 / image_w;
                let y = vals[1] as f32 / image_h;
                if x < 1.0 && y < 1.0 {
                    log::info!("SubjectLocation → AF: ({:.4},{:.4})", x, y);
                    return vec![FocusRegion {
                        x,
                        y,
                        width: 0.05,
                        height: 0.05,
                        kind: FocusKind::Point,
                        is_primary: true,
                    }];
                }
            }
        }
    }

    Vec::new()
}
