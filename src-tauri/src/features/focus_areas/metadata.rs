use exif::{Tag, Value};
use rawler::decoders::RawDecodeParams;
use std::collections::HashMap;

pub(super) fn read_native_sensor_size(file_bytes: &[u8]) -> Option<(u32, u32)> {
    let source = rawler::rawsource::RawSource::new_from_slice(file_bytes);
    let decoder = rawler::get_decoder(&source).ok()?;
    let raw = decoder
        .raw_image(&source, &RawDecodeParams::default(), false)
        .ok()?;
    let w = raw.width as u32;
    let h = raw.height as u32;
    Some((w, h))
}

pub(super) fn parse_tiff_exif_fields(file_bytes: &[u8]) -> Option<Vec<exif::Field>> {
    exif::parse_exif(file_bytes)
        .ok()
        .map(|(fields, _le)| fields)
}

pub(super) fn find_field_value<'a>(fields: &'a [exif::Field], tag: Tag) -> Option<&'a Value> {
    fields.iter().find(|f| f.tag == tag).map(|f| &f.value)
}

/// 从 TIFF EXIF 获取图像尺寸
pub(super) fn get_exif_dimensions(file_bytes: &[u8]) -> (u32, u32) {
    let fields = match parse_tiff_exif_fields(file_bytes) {
        Some(f) => f,
        None => return (0, 0),
    };
    let get_u32 = |tag| {
        find_field_value(&fields, tag).and_then(|v| match v {
            Value::Long(vals) if !vals.is_empty() => Some(vals[0]),
            Value::Short(vals) if !vals.is_empty() => Some(vals[0] as u32),
            _ => None,
        })
    };
    let w = get_u32(Tag::PixelXDimension)
        .or_else(|| get_u32(Tag::ImageWidth))
        .unwrap_or(0);
    let h = get_u32(Tag::PixelYDimension)
        .or_else(|| get_u32(Tag::ImageLength))
        .unwrap_or(0);
    (w, h)
}

/// 提取 MakerNote 原始二进制
pub(super) fn extract_makernote_tiff(file_bytes: &[u8]) -> Option<Vec<u8>> {
    let fields = parse_tiff_exif_fields(file_bytes)?;
    match find_field_value(&fields, Tag::MakerNote)? {
        Value::Undefined(data, _) => Some(data.clone()),
        _ => None,
    }
}

/// 解析 MakerNote 中的 TIFF 子 IFD，返回 tag→原始字节 映射
pub(super) fn parse_makernote_ifd(data: &[u8]) -> HashMap<u16, Vec<u8>> {
    let mut result = HashMap::new();
    if data.len() < 8 {
        return result;
    }

    // ── 跳过制造商前缀 ──
    let mut off: usize = 0;
    let mut endian_override: Option<bool> = None;

    if data.len() >= 9 && &data[0..9] == b"SONY DSC " {
        off = 12;
    } else if data.len() >= 8 && &data[0..8] == b"FUJIFILM" {
        off = 12;
    } else if data.len() >= 5 && &data[0..5] == b"OLYMP" {
        off = if data.len() >= 7 && &data[0..7] == b"OLYMPUS" {
            12
        } else {
            8
        };
    } else if data.len() >= 6 && &data[0..6] == b"PENTAX" {
        off = 8;
        if off + 2 <= data.len() {
            endian_override = Some(&data[off..off + 2] == b"II");
            off += 2;
        }
    } else if data.len() >= 7 && &data[0..7] == b"Nikon\0\x02" {
        off = 10;
        if off + 2 <= data.len() {
            endian_override = Some(&data[off..off + 2] == b"II");
            off += 2;
        }
    }
    if off >= data.len() {
        return result;
    }
    let payload = &data[off..];

    let little_endian = if let Some(le) = endian_override {
        le
    } else if payload.len() >= 2 && &payload[0..2] == b"II" {
        true
    } else if payload.len() >= 2 && &payload[0..2] == b"MM" {
        false
    } else {
        true // Sony 默认 LE
    };

    // 跳过 II/MM 标记
    let tiff_start = if endian_override.is_none()
        && payload.len() >= 2
        && (&payload[0..2] == b"II" || &payload[0..2] == b"MM")
    {
        2
    } else {
        0
    };
    let tiff_data = &payload[tiff_start..];
    if tiff_data.len() < 12 {
        return result;
    }

    // 确定 IFD 起始偏移
    let ifd_start = {
        let maybe_magic = if little_endian {
            u16::from_le_bytes([tiff_data[2], tiff_data[3]])
        } else {
            u16::from_be_bytes([tiff_data[2], tiff_data[3]])
        };
        if maybe_magic == 0x002a && tiff_data.len() >= 8 {
            if little_endian {
                u32::from_le_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]])
                    as usize
            } else {
                u32::from_be_bytes([tiff_data[4], tiff_data[5], tiff_data[6], tiff_data[7]])
                    as usize
            }
        } else {
            0
        }
    };
    if ifd_start + 2 > tiff_data.len() {
        return result;
    }

    // 读取 IFD 条目
    let entry_count = if little_endian {
        u16::from_le_bytes([tiff_data[ifd_start], tiff_data[ifd_start + 1]])
    } else {
        u16::from_be_bytes([tiff_data[ifd_start], tiff_data[ifd_start + 1]])
    } as usize;

    for i in 0..entry_count {
        let eo = ifd_start + 2 + i * 12;
        if eo + 12 > tiff_data.len() {
            break;
        }
        let tag = if little_endian {
            u16::from_le_bytes([tiff_data[eo], tiff_data[eo + 1]])
        } else {
            u16::from_be_bytes([tiff_data[eo], tiff_data[eo + 1]])
        };
        let vtype = if little_endian {
            u16::from_le_bytes([tiff_data[eo + 2], tiff_data[eo + 3]])
        } else {
            u16::from_be_bytes([tiff_data[eo + 2], tiff_data[eo + 3]])
        };
        let count = if little_endian {
            u32::from_le_bytes([
                tiff_data[eo + 4],
                tiff_data[eo + 5],
                tiff_data[eo + 6],
                tiff_data[eo + 7],
            ])
        } else {
            u32::from_be_bytes([
                tiff_data[eo + 4],
                tiff_data[eo + 5],
                tiff_data[eo + 6],
                tiff_data[eo + 7],
            ])
        } as usize;
        let val_bytes = &tiff_data[eo + 8..eo + 12];

        let type_size = match vtype {
            1 | 2 | 7 => 1,
            3 => 2,
            4 | 9 => 4,
            5 | 10 => 8,
            _ => 1,
        };
        let total = count * type_size;
        if total <= 4 {
            result.insert(tag, val_bytes[..total.min(4)].to_vec());
        } else {
            let val_off = if little_endian {
                u32::from_le_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]])
                    as usize
            } else {
                u32::from_be_bytes([val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]])
                    as usize
            };
            if val_off + total <= tiff_data.len() {
                result.insert(tag, tiff_data[val_off..val_off + total].to_vec());
            }
        }
    }
    result
}
