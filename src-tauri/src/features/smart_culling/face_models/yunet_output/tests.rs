use ndarray::{ArrayD, IxDyn};

use super::*;

fn fixture() -> [ArrayD<f32>; 4] {
    let mut output = [1, 1, 4, 10].map(|channels| ArrayD::zeros(IxDyn(&[1, 6400, channels])));
    output[0][[0, 81, 0]] = 1.0;
    output[1][[0, 81, 0]] = 0.81;
    output[2][[0, 81, 0]] = 1.0;
    output[2][[0, 81, 1]] = 1.0;
    output
}

fn mapping() -> RegionMapping {
    RegionMapping {
        scale: 0.5,
        content_width: 640.0,
        content_height: 320.0,
        offset_x: 100.0,
        offset_y: 200.0,
    }
}

fn decode(output: &[ArrayD<f32>; 4]) -> Result<Vec<DetectedFace>> {
    decode_stride(output.each_ref().map(|array| array.view()), 8, mapping())
}

#[test]
fn valid_output_uses_official_confidence_and_keeps_region_mapping() {
    let output = decode(&fixture()).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].score, 0.9);
    assert_eq!(output[0].bbox, [124.0, 224.0, 16.0, 16.0]);
    assert_eq!(output[0].landmarks, [(116.0, 216.0); 5]);
}

#[test]
fn official_geometric_mean_drives_the_unchanged_detection_threshold() {
    for (cls, obj, expected) in [(0.7, 0.7, 0.7), (1.0, 1.0, 1.0), (1.0, 0.25, 0.5)] {
        let mut tensors = fixture();
        tensors[0][[0, 81, 0]] = cls;
        tensors[1][[0, 81, 0]] = obj;

        let faces = decode(&tensors).unwrap();

        if expected >= YUNET_CONF_THRESHOLD {
            assert_eq!(faces.len(), 1);
            assert_eq!(faces[0].score, expected);
        } else {
            assert!(faces.is_empty());
        }
    }
}

#[test]
fn malformed_shapes_return_errors_instead_of_panicking_or_reinterpreting_axes() {
    for shape in [&[1, 6399, 1][..], &[1, 1, 6400][..], &[6400][..]] {
        let mut output = fixture();
        output[0] = ArrayD::zeros(IxDyn(shape));
        assert!(decode(&output).is_err(), "shape {shape:?} was accepted");
    }
}

#[test]
fn non_finite_values_in_each_output_fail_even_below_the_detection_threshold() {
    for tensor in 0..4 {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut output = fixture();
            output[tensor][[0, 0, 0]] = bad;
            assert!(decode(&output).is_err(), "tensor {tensor} accepted {bad}");
        }
    }
}

#[test]
fn invalid_probabilities_are_not_used_as_confident_faces() {
    for tensor in 0..2 {
        for bad in [-0.1, 1.1] {
            let mut output = fixture();
            output[tensor][[0, 81, 0]] = bad;
            assert!(decode(&output).is_err());
        }
    }
}

#[test]
fn finite_box_logits_must_not_overflow_or_underflow_the_decoded_dimensions() {
    for bad in [100.0, -200.0] {
        let mut output = fixture();
        output[2][[0, 81, 2]] = bad;
        assert!(decode(&output).is_err());
    }
}

#[test]
fn finite_keypoints_must_not_overflow_in_image_coordinates() {
    let mut output = fixture();
    output[3][[0, 81, 0]] = f32::MAX;
    assert!(decode(&output).is_err());
}

#[test]
fn invalid_mapping_is_an_error_instead_of_empty_or_corrupt_detections() {
    let output = fixture();
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut mapping = mapping();
        mapping.scale = bad;
        assert!(decode_stride(output.each_ref().map(|array| array.view()), 8, mapping).is_err());
    }
}

#[test]
fn legitimate_border_boxes_and_padded_margin_filter_are_preserved() {
    let mut output = fixture();
    output[2][[0, 81, 0]] = -1.0;
    assert_eq!(decode(&output).unwrap()[0].bbox[0], 92.0);
    output[2][[0, 81, 0]] = 100.0;
    assert!(decode(&output).unwrap().is_empty());
}
