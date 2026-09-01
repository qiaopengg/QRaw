use std::ffi::CStr;
use std::ptr;

use anyhow::{Result, anyhow};
use objc::runtime::{BOOL, Class, Object, YES};
use objc::{Encode, Encoding, msg_send, sel, sel_impl};

use super::VisionQualitySignals;

const FACE_CAPTURE_QUALITY_REVISION: usize = 3;

#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}

pub(super) fn is_supported() -> bool {
    Class::get("VNCalculateImageAestheticsScoresRequest").is_some()
        && Class::get("VNDetectFaceCaptureQualityRequest").is_some()
        && Class::get("VNDetectHumanRectanglesRequest").is_some()
}

pub(super) fn perform_requests(
    image_bytes: &[u8],
    face_boxes: &[[f32; 4]],
    width: u32,
    height: u32,
) -> Result<VisionQualitySignals> {
    let normalized_faces = face_boxes
        .iter()
        .map(|bbox| normalize_face_bbox(*bbox, width, height))
        .collect::<Vec<_>>();

    // SAFETY: Every Objective-C class and selector is part of the public
    // Foundation/Vision API. Classes are resolved dynamically before use so
    // the macOS 13 deployment target remains loadable on systems lacking the
    // macOS 15 aesthetics class. Inputs are copied into NSData, all temporary
    // objects live inside an autorelease pool, and returned scalars are copied
    // before the pool is drained.
    unsafe { perform_normalized_requests(image_bytes, &normalized_faces) }
}

fn normalize_face_bbox(bbox: [f32; 4], width: u32, height: u32) -> Option<CGRect> {
    if bbox.iter().any(|value| !value.is_finite()) || bbox[2] <= 0.0 || bbox[3] <= 0.0 {
        return None;
    }
    let width = width as f64;
    let height = height as f64;
    let left = (f64::from(bbox[0]) / width).clamp(0.0, 1.0);
    let right = (f64::from(bbox[0] + bbox[2]) / width).clamp(0.0, 1.0);
    let top = (f64::from(bbox[1]) / height).clamp(0.0, 1.0);
    let bottom = (f64::from(bbox[1] + bbox[3]) / height).clamp(0.0, 1.0);
    (right > left && bottom > top).then_some(CGRect {
        origin: CGPoint {
            x: left,
            y: 1.0 - bottom,
        },
        size: CGSize {
            width: right - left,
            height: bottom - top,
        },
    })
}

unsafe fn perform_normalized_requests(
    image_bytes: &[u8],
    normalized_faces: &[Option<CGRect>],
) -> Result<VisionQualitySignals> {
    let pool_class = required_class("NSAutoreleasePool")?;
    // SAFETY: `new` is a valid NSObject initializer for NSAutoreleasePool.
    let pool: *mut Object = unsafe { msg_send![pool_class, new] };
    let result = (|| {
        let data_class = required_class("NSData")?;
        let dictionary_class = required_class("NSDictionary")?;
        let array_class = required_class("NSArray")?;
        let handler_class = required_class("VNImageRequestHandler")?;
        let aesthetics_class = required_class("VNCalculateImageAestheticsScoresRequest")?;
        let face_request_class = required_class("VNDetectFaceCaptureQualityRequest")?;
        let human_request_class = required_class("VNDetectHumanRectanglesRequest")?;
        let face_observation_class = required_class("VNFaceObservation")?;

        // SAFETY: NSData copies the bytes for the duration of the request.
        let data: *mut Object = unsafe {
            msg_send![data_class, dataWithBytes:image_bytes.as_ptr() length:image_bytes.len()]
        };
        let options: *mut Object = unsafe { msg_send![dictionary_class, dictionary] };
        let handler: *mut Object = unsafe { msg_send![handler_class, alloc] };
        let handler: *mut Object = unsafe { msg_send![handler, initWithData:data options:options] };
        if handler.is_null() {
            return Err(anyhow!(
                "Apple Vision failed to create an image request handler"
            ));
        }
        let _: *mut Object = unsafe { msg_send![handler, autorelease] };

        let aesthetics_request: *mut Object = unsafe { msg_send![aesthetics_class, new] };
        let human_request: *mut Object = unsafe { msg_send![human_request_class, new] };
        let _: () = unsafe { msg_send![human_request, setUpperBodyOnly:YES] };
        let _: *mut Object = unsafe { msg_send![aesthetics_request, autorelease] };
        let _: *mut Object = unsafe { msg_send![human_request, autorelease] };

        let mut requests = vec![aesthetics_request, human_request];
        let mut face_request = ptr::null_mut();
        let mut input_face_indices = Vec::new();
        let mut face_observations = Vec::new();
        for (index, rect) in normalized_faces.iter().enumerate() {
            if let Some(rect) = rect {
                let observation: *mut Object =
                    unsafe { msg_send![face_observation_class, observationWithBoundingBox:*rect] };
                if !observation.is_null() {
                    input_face_indices.push(index);
                    face_observations.push(observation);
                }
            }
        }
        if !face_observations.is_empty() {
            face_request = unsafe { msg_send![face_request_class, new] };
            let _: () =
                unsafe { msg_send![face_request, setRevision:FACE_CAPTURE_QUALITY_REVISION] };
            let input_faces: *mut Object = unsafe {
                msg_send![array_class, arrayWithObjects:face_observations.as_ptr() count:face_observations.len()]
            };
            let _: () = unsafe { msg_send![face_request, setInputFaceObservations:input_faces] };
            let _: *mut Object = unsafe { msg_send![face_request, autorelease] };
            requests.push(face_request);
        }

        let request_array: *mut Object = unsafe {
            msg_send![array_class, arrayWithObjects:requests.as_ptr() count:requests.len()]
        };
        let mut error: *mut Object = ptr::null_mut();
        let succeeded: BOOL =
            unsafe { msg_send![handler, performRequests:request_array error:&mut error] };
        if succeeded != YES {
            return Err(anyhow!("Apple Vision request failed: {}", unsafe {
                error_description(error)
            }));
        }

        let aesthetics_results: *mut Object = unsafe { msg_send![aesthetics_request, results] };
        let aesthetics_observation: *mut Object =
            unsafe { msg_send![aesthetics_results, firstObject] };
        let (aesthetics_score, is_utility) = if aesthetics_observation.is_null() {
            (None, None)
        } else {
            let score: f32 = unsafe { msg_send![aesthetics_observation, overallScore] };
            let utility: BOOL = unsafe { msg_send![aesthetics_observation, isUtility] };
            (score.is_finite().then_some(score), Some(utility == YES))
        };

        let human_results: *mut Object = unsafe { msg_send![human_request, results] };
        let human_count: usize = unsafe { msg_send![human_results, count] };
        let mut max_human_confidence: Option<f32> = None;
        for index in 0..human_count {
            let observation: *mut Object = unsafe { msg_send![human_results, objectAtIndex:index] };
            let confidence: f32 = unsafe { msg_send![observation, confidence] };
            if confidence.is_finite() {
                max_human_confidence = Some(
                    max_human_confidence.map_or(confidence, |current| current.max(confidence)),
                );
            }
        }

        let mut face_capture_qualities = vec![None; normalized_faces.len()];
        if !face_request.is_null() {
            let face_results: *mut Object = unsafe { msg_send![face_request, results] };
            let face_count: usize = unsafe { msg_send![face_results, count] };
            for (result_index, input_index) in input_face_indices
                .iter()
                .copied()
                .take(face_count)
                .enumerate()
            {
                let observation: *mut Object =
                    unsafe { msg_send![face_results, objectAtIndex:result_index] };
                let quality_number: *mut Object =
                    unsafe { msg_send![observation, faceCaptureQuality] };
                if !quality_number.is_null() {
                    let quality: f32 = unsafe { msg_send![quality_number, floatValue] };
                    if quality.is_finite() && (0.0..=1.0).contains(&quality) {
                        face_capture_qualities[input_index] = Some(quality);
                    }
                }
            }
        }

        Ok(VisionQualitySignals {
            aesthetics_score,
            is_utility,
            face_capture_qualities,
            human_count,
            max_human_confidence,
            unavailable_reason: None,
        })
    })();
    // SAFETY: Draining the pool releases all temporary Foundation/Vision objects.
    let _: () = unsafe { msg_send![pool, drain] };
    result
}

fn required_class(name: &'static str) -> Result<&'static Class> {
    Class::get(name).ok_or_else(|| anyhow!("Apple Vision class is unavailable: {name}"))
}

unsafe fn error_description(error: *mut Object) -> String {
    if error.is_null() {
        return "unknown error".to_string();
    }
    let description: *mut Object = unsafe { msg_send![error, localizedDescription] };
    if description.is_null() {
        return "unknown error".to_string();
    }
    let bytes: *const std::os::raw::c_char = unsafe { msg_send![description, UTF8String] };
    if bytes.is_null() {
        "unknown error".to_string()
    } else {
        // SAFETY: NSString guarantees a NUL-terminated UTF-8 view for UTF8String.
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(target_pointer_width = "64")]
type CGFloat = f64;
#[cfg(target_pointer_width = "32")]
type CGFloat = f32;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CGPoint {
    x: CGFloat,
    y: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CGSize {
    width: CGFloat,
    height: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

unsafe impl Encode for CGRect {
    fn encode() -> Encoding {
        #[cfg(target_pointer_width = "64")]
        let encoding = "{CGRect={CGPoint=dd}{CGSize=dd}}";
        #[cfg(target_pointer_width = "32")]
        let encoding = "{CGRect={CGPoint=ff}{CGSize=ff}}";
        // SAFETY: CGRect is repr(C) and CGFloat matches the target pointer width.
        unsafe { Encoding::from_str(encoding) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_left_pixel_bbox_converts_to_clamped_vision_coordinates() {
        let rect = normalize_face_bbox([20.0, 10.0, 40.0, 30.0], 100, 100).unwrap();
        assert_eq!(rect.origin, CGPoint { x: 0.2, y: 0.6 });
        assert!((rect.size.width - 0.4).abs() < f64::EPSILON);
        assert!((rect.size.height - 0.3).abs() < f64::EPSILON);

        let clamped = normalize_face_bbox([-10.0, -5.0, 30.0, 25.0], 100, 100).unwrap();
        assert_eq!(clamped.origin, CGPoint { x: 0.0, y: 0.8 });
        assert!((clamped.size.width - 0.2).abs() < f64::EPSILON);
        assert!((clamped.size.height - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_or_empty_face_boxes_are_not_sent_to_vision() {
        assert!(normalize_face_bbox([0.0, 0.0, 0.0, 10.0], 100, 100).is_none());
        assert!(normalize_face_bbox([f32::NAN, 0.0, 10.0, 10.0], 100, 100).is_none());
        assert!(normalize_face_bbox([150.0, 0.0, 10.0, 10.0], 100, 100).is_none());
    }
}
