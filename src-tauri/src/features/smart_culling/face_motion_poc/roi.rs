//! MediaPipe-equivalent first-frame face ROI geometry for the isolated POC.
//!
//! MediaPipe's FaceDetectorGraph rotates a detection rectangle from its two
//! eye keypoints and expands both axes by 1.5 before FaceMesh preprocessing.
//! ImageToTensor then stretches that rotated rectangle to 256x256 with bilinear
//! sampling and replicated borders. This module reproduces that geometry for a
//! YuNet detection without making any eye or expression decision.

use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbImage};
use imageproc::geometric_transformations::{Border, Interpolation, warp_into_with};

use super::super::face_models::DetectedFace;
use super::FACE_MESH_INPUT_SIZE;

const DETECTION_ROI_SCALE: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FaceRoi {
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
    /// Clockwise rotation in image coordinates, matching MediaPipe's
    /// NormalizedRect convention.
    pub rotation: f32,
}

impl FaceRoi {
    pub(super) fn from_detection(detection: &DetectedFace) -> Result<Self> {
        let values = detection.bbox.into_iter().chain(
            detection
                .landmarks
                .iter()
                .flat_map(|point| [point.0, point.1]),
        );
        if values.into_iter().any(|value| !value.is_finite()) {
            return Err(anyhow!("face ROI input contains non-finite coordinates"));
        }
        let [x, y, width, height] = detection.bbox;
        if width <= 0.0 || height <= 0.0 {
            return Err(anyhow!("face ROI requires a positive detection box"));
        }

        // YuNet order starts with the image-left eye and then image-right eye,
        // which matches the two keypoints used by MediaPipe's face detector.
        let start = detection.landmarks[0];
        let end = detection.landmarks[1];
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        if dx.hypot(dy) <= f32::EPSILON {
            return Err(anyhow!("face ROI eye keypoints are degenerate"));
        }

        // Exact DetectionsToRectsCalculator formula for target angle 0:
        // target - atan2(-(y1-y0), x1-x0).
        let rotation = -(-dy).atan2(dx);
        Ok(Self {
            center_x: x + width * 0.5,
            center_y: y + height * 0.5,
            width: width * DETECTION_ROI_SCALE,
            height: height * DETECTION_ROI_SCALE,
            rotation,
        })
    }

    pub(super) fn extract(&self, image: &DynamicImage) -> Result<RgbImage> {
        self.validate()?;
        let source = image.to_rgb8();
        if source.width() == 0 || source.height() == 0 {
            return Err(anyhow!("face ROI source image is empty"));
        }

        let size = FACE_MESH_INPUT_SIZE as u32;
        let scale = FACE_MESH_INPUT_SIZE as f32;
        let mut output = RgbImage::new(size, size);
        warp_into_with(
            &source,
            |x, y| self.roi_to_image(x / scale, y / scale).into(),
            Interpolation::Bilinear,
            Border::Replicate,
            &mut output,
        );
        Ok(output)
    }

    pub(super) fn project_landmarks(&self, landmarks: &[[f32; 3]]) -> Result<Vec<[f32; 2]>> {
        self.validate()?;
        if landmarks
            .iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(anyhow!("face ROI landmarks contain non-finite coordinates"));
        }
        Ok(landmarks
            .iter()
            .map(|point| self.roi_to_image(point[0], point[1]))
            .collect())
    }

    fn validate(&self) -> Result<()> {
        if [
            self.center_x,
            self.center_y,
            self.width,
            self.height,
            self.rotation,
        ]
        .into_iter()
        .any(|value| !value.is_finite())
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return Err(anyhow!("face ROI geometry is invalid"));
        }
        Ok(())
    }

    fn roi_to_image(&self, x: f32, y: f32) -> [f32; 2] {
        let local_x = (x - 0.5) * self.width;
        let local_y = (y - 0.5) * self.height;
        let (sin, cos) = self.rotation.sin_cos();
        [
            self.center_x + local_x * cos - local_y * sin,
            self.center_y + local_x * sin + local_y * cos,
        ]
    }

    #[cfg(test)]
    fn image_to_roi(&self, x: f32, y: f32) -> [f32; 2] {
        let dx = x - self.center_x;
        let dy = y - self.center_y;
        let (sin, cos) = self.rotation.sin_cos();
        [
            (dx * cos + dy * sin) / self.width + 0.5,
            (-dx * sin + dy * cos) / self.height + 0.5,
        ]
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;

    fn detection() -> DetectedFace {
        DetectedFace {
            bbox: [10.0, 20.0, 100.0, 80.0],
            score: 0.95,
            landmarks: [
                (30.0, 40.0),
                (70.0, 40.0),
                (50.0, 60.0),
                (38.0, 80.0),
                (62.0, 80.0),
            ],
        }
    }

    #[test]
    fn detection_rect_uses_official_eye_rotation_and_expansion() {
        let roi = FaceRoi::from_detection(&detection()).unwrap();

        assert_eq!(roi.center_x, 60.0);
        assert_eq!(roi.center_y, 60.0);
        assert_eq!(roi.width, 150.0);
        assert_eq!(roi.height, 120.0);
        assert!(roi.rotation.abs() < f32::EPSILON);
    }

    #[test]
    fn sloped_eye_line_becomes_horizontal_inside_the_roi() {
        let mut face = detection();
        face.landmarks[1] = (70.0, 60.0);
        let roi = FaceRoi::from_detection(&face).unwrap();

        let left = roi.image_to_roi(face.landmarks[0].0, face.landmarks[0].1);
        let right = roi.image_to_roi(face.landmarks[1].0, face.landmarks[1].1);

        assert!((left[1] - right[1]).abs() < 1e-6);
        assert!(right[0] > left[0]);
    }

    #[test]
    fn extraction_maps_tensor_center_to_roi_center() {
        let mut source = RgbImage::from_pixel(121, 121, Rgb([0, 0, 0]));
        source.put_pixel(60, 60, Rgb([255, 10, 20]));
        let roi = FaceRoi::from_detection(&detection()).unwrap();

        let crop = roi.extract(&DynamicImage::ImageRgb8(source)).unwrap();

        assert_eq!(crop.get_pixel(128, 128), &Rgb([255, 10, 20]));
    }

    #[test]
    fn invalid_or_degenerate_detection_fails_loudly() {
        let mut face = detection();
        face.bbox[2] = 0.0;
        assert!(FaceRoi::from_detection(&face).is_err());

        let mut face = detection();
        face.landmarks[1] = face.landmarks[0];
        assert!(FaceRoi::from_detection(&face).is_err());
    }
}
