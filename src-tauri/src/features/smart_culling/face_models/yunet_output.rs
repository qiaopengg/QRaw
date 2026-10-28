//! YuNet output decoding, separated from inference for contract regression tests.

use anyhow::{Result, anyhow};
use ndarray::ArrayViewD;

use super::{DetectedFace, YUNET_CONF_THRESHOLD, YUNET_INPUT_SIZE, YUNET_STRIDES};

#[derive(Clone, Copy)]
pub(super) struct RegionMapping {
    pub scale: f32,
    pub content_width: f32,
    pub content_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub(super) fn decode_stride(
    outputs: [ArrayViewD<'_, f32>; 4],
    stride: u32,
    mapping: RegionMapping,
) -> Result<Vec<DetectedFace>> {
    if !YUNET_STRIDES.contains(&stride) {
        return Err(anyhow!("YuNet output stride {stride} is unsupported"));
    }
    let RegionMapping {
        scale,
        content_width,
        content_height,
        offset_x,
        offset_y,
    } = mapping;
    if ![scale, content_width, content_height, offset_x, offset_y]
        .iter()
        .all(|value| value.is_finite())
        || scale <= 0.0
        || content_width <= 0.0
        || content_width > YUNET_INPUT_SIZE as f32
        || content_height <= 0.0
        || content_height > YUNET_INPUT_SIZE as f32
        || offset_x < 0.0
        || offset_y < 0.0
    {
        return Err(anyhow!("YuNet region mapping is invalid"));
    }
    let feature_size = (YUNET_INPUT_SIZE / stride) as usize;
    let num_priors = feature_size * feature_size;
    let [cls, obj, bbox, kps] = outputs;
    let cls_slice = validated_values(&cls, "cls", stride, num_priors, 1)?;
    let obj_slice = validated_values(&obj, "obj", stride, num_priors, 1)?;
    let bbox_slice = validated_values(&bbox, "bbox", stride, num_priors, 4)?;
    let kps_slice = validated_values(&kps, "kps", stride, num_priors, 10)?;
    let mut candidates = Vec::new();
    for i in 0..num_priors {
        // Match FaceDetectorYN's geometric mean after probability validation.
        // https://github.com/opencv/opencv/blob/4.8.0/modules/objdetect/src/face_detect.cpp
        let score = (cls_slice[i] * obj_slice[i]).sqrt();
        if score < YUNET_CONF_THRESHOLD {
            continue;
        }
        let col = (i % feature_size) as f32;
        let row = (i / feature_size) as f32;
        let prior_x = col * stride as f32;
        let prior_y = row * stride as f32;
        let cx = bbox_slice[i * 4] * stride as f32 + prior_x;
        let cy = bbox_slice[i * 4 + 1] * stride as f32 + prior_y;
        let w = bbox_slice[i * 4 + 2].exp() * stride as f32;
        let h = bbox_slice[i * 4 + 3].exp() * stride as f32;
        if ![cx, cy, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(anyhow!(
                "YuNet bbox_{stride} prior {i} decodes to invalid geometry"
            ));
        }
        if cx > content_width || cy > content_height {
            continue;
        }
        let x1 = (cx - w / 2.0) / scale + offset_x;
        let y1 = (cy - h / 2.0) / scale + offset_y;
        let box_w = w / scale;
        let box_h = h / scale;
        if ![
            x1,
            y1,
            box_w,
            box_h,
            x1 + box_w,
            y1 + box_h,
            2.0 * box_w * box_h,
        ]
        .iter()
        .all(|value| value.is_finite())
            || box_w <= 0.0
            || box_h <= 0.0
        {
            return Err(anyhow!(
                "YuNet bbox_{stride} prior {i} overflows image coordinates"
            ));
        }
        let mut landmarks = [(0.0f32, 0.0f32); 5];
        for j in 0..5 {
            let lx = kps_slice[i * 10 + j * 2] * stride as f32 + prior_x;
            let ly = kps_slice[i * 10 + j * 2 + 1] * stride as f32 + prior_y;
            landmarks[j] = (lx / scale + offset_x, ly / scale + offset_y);
            if !landmarks[j].0.is_finite() || !landmarks[j].1.is_finite() {
                return Err(anyhow!(
                    "YuNet kps_{stride} prior {i} overflows image coordinates"
                ));
            }
        }
        candidates.push(DetectedFace {
            bbox: [x1, y1, box_w, box_h],
            score,
            landmarks,
        });
    }
    Ok(candidates)
}

fn validated_values<'a>(
    tensor: &'a ArrayViewD<'_, f32>,
    name: &str,
    stride: u32,
    num_priors: usize,
    channels: usize,
) -> Result<&'a [f32]> {
    let expected = [1, num_priors, channels];
    if tensor.shape() != expected {
        return Err(anyhow!(
            "YuNet {name}_{stride} shape {:?} does not match {expected:?}",
            tensor.shape()
        ));
    }
    let values = tensor
        .as_slice()
        .ok_or_else(|| anyhow!("YuNet {name}_{stride} must be contiguous"))?;
    if values.iter().any(|value| !value.is_finite()) {
        return Err(anyhow!("YuNet {name}_{stride} contains non-finite values"));
    }
    if channels == 1 && values.iter().any(|value| !(0.0..=1.0).contains(value)) {
        return Err(anyhow!(
            "YuNet {name}_{stride} probabilities must be within [0, 1]"
        ));
    }
    Ok(values)
}

#[cfg(test)]
mod tests;
