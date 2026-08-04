use std::sync::Mutex;

use anyhow::{Result, anyhow};
use image::{DynamicImage, Rgb, RgbImage};
use imageproc::geometric_transformations::{Border, Interpolation, Projection, warp_into};
use nalgebra::{Matrix2, Vector2, linalg::SVD};
use ndarray::Array4;
use ort::{session::Session, value::Tensor};

use super::types::{FaceResult, KeyPersonEvidence};

const SFACE_SIZE: u32 = 112;
const SFACE_SUSPECTED_THRESHOLD: f32 = 0.363;
const SFACE_CLEAR_MISSING_THRESHOLD: f32 = 0.25;
const AMBIGUITY_MARGIN: f32 = 0.05;
const SFACE_TEMPLATE: [(f64, f64); 5] = [
    (38.2946, 51.6963),
    (73.5318, 51.5014),
    (56.0252, 71.7366),
    (41.5493, 92.3655),
    (70.7299, 92.2041),
];

#[derive(Debug, Clone)]
pub struct KeyPersonReference {
    pub priority: usize,
    pub embedding: Vec<f32>,
}

pub fn run_sface_embedding(
    image: &DynamicImage,
    landmarks: &[(f32, f32); 5],
    session: &Mutex<Session>,
) -> Result<Vec<f32>> {
    let aligned = align_face(image, landmarks)?;
    let mut input = Array4::<f32>::zeros((1, 3, SFACE_SIZE as usize, SFACE_SIZE as usize));
    for (x, y, pixel) in aligned.enumerate_pixels() {
        let x = x as usize;
        let y = y as usize;
        input[[0, 0, y, x]] = pixel[0] as f32;
        input[[0, 1, y, x]] = pixel[1] as f32;
        input[[0, 2, y, x]] = pixel[2] as f32;
    }

    let tensor = Tensor::from_array(input.into_dyn().as_standard_layout().into_owned())?;
    let mut session = session
        .lock()
        .map_err(|_| anyhow!("SFace inference session lock is poisoned"))?;
    let outputs = session.run(ort::inputs!["data" => tensor])?;
    let embedding = outputs[0]
        .try_extract_array::<f32>()?
        .iter()
        .copied()
        .collect::<Vec<_>>();
    normalize_embedding(embedding)
}

fn align_face(image: &DynamicImage, landmarks: &[(f32, f32); 5]) -> Result<RgbImage> {
    if landmarks
        .iter()
        .flat_map(|point| [point.0, point.1])
        .any(|value| !value.is_finite())
    {
        return Err(anyhow!("SFace landmarks must be finite"));
    }
    let transform = similarity_transform(landmarks)?;
    let projection = Projection::from_matrix(transform)
        .ok_or_else(|| anyhow!("SFace alignment transform is not invertible"))?;
    let source = image.to_rgb8();
    let mut aligned = RgbImage::from_pixel(SFACE_SIZE, SFACE_SIZE, Rgb([0, 0, 0]));
    warp_into(
        &source,
        projection,
        Interpolation::Bilinear,
        Border::Constant(Rgb([0, 0, 0])),
        &mut aligned,
    );
    Ok(aligned)
}

fn similarity_transform(landmarks: &[(f32, f32); 5]) -> Result<[f32; 9]> {
    let source = landmarks.map(|(x, y)| Vector2::new(x as f64, y as f64));
    let target = SFACE_TEMPLATE.map(|(x, y)| Vector2::new(x, y));
    let source_mean = source.iter().copied().sum::<Vector2<f64>>() / source.len() as f64;
    let target_mean = target.iter().copied().sum::<Vector2<f64>>() / target.len() as f64;

    let mut covariance = Matrix2::<f64>::zeros();
    let mut source_variance = 0.0;
    for (source, target) in source.iter().zip(target) {
        let source = source - source_mean;
        let target = target - target_mean;
        covariance += target * source.transpose();
        source_variance += source.dot(&source);
    }
    covariance /= source.len() as f64;
    source_variance /= source.len() as f64;
    if source_variance <= f64::EPSILON {
        return Err(anyhow!("SFace landmarks are degenerate"));
    }

    let svd = SVD::new(covariance, true, true);
    let u = svd
        .u
        .ok_or_else(|| anyhow!("SFace alignment SVD has no U"))?;
    let v_t = svd
        .v_t
        .ok_or_else(|| anyhow!("SFace alignment SVD has no Vt"))?;
    let mut diagonal = Matrix2::<f64>::identity();
    if covariance.determinant() < 0.0 {
        diagonal[(1, 1)] = -1.0;
    }
    let rotation = u * diagonal * v_t;
    let scale = svd.singular_values.dot(&diagonal.diagonal()) / source_variance;
    let translation = target_mean - scale * rotation * source_mean;

    Ok([
        (scale * rotation[(0, 0)]) as f32,
        (scale * rotation[(0, 1)]) as f32,
        translation[0] as f32,
        (scale * rotation[(1, 0)]) as f32,
        (scale * rotation[(1, 1)]) as f32,
        translation[1] as f32,
        0.0,
        0.0,
        1.0,
    ])
}

fn normalize_embedding(mut embedding: Vec<f32>) -> Result<Vec<f32>> {
    if embedding.len() != 128 || embedding.iter().any(|value| !value.is_finite()) {
        return Err(anyhow!("SFace output must contain 128 finite values"));
    }
    let norm = embedding
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err(anyhow!("SFace output norm is zero"));
    }
    embedding.iter_mut().for_each(|value| *value /= norm);
    Ok(embedding)
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    (left.len() == 128 && right.len() == 128).then(|| {
        left.iter()
            .zip(right)
            .map(|(left, right)| left * right)
            .sum()
    })
}

pub fn match_key_people(
    references: &[KeyPersonReference],
    faces: &mut [FaceResult],
) -> Vec<KeyPersonEvidence> {
    if references.is_empty() {
        return Vec::new();
    }
    let scores = references
        .iter()
        .map(|reference| {
            faces
                .iter()
                .map(|face| {
                    face.identity_embedding
                        .as_deref()
                        .and_then(|embedding| cosine_similarity(&reference.embedding, embedding))
                        .filter(|value| value.is_finite())
                        .unwrap_or(-1.0)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let assignments = maximum_weight_assignment(&scores, SFACE_SUSPECTED_THRESHOLD);

    references
        .iter()
        .enumerate()
        .map(|(reference_index, reference)| {
            let assigned = assignments[reference_index];
            let has_embedding = faces.iter().any(|face| face.identity_embedding.is_some());
            let best_similarity = scores[reference_index]
                .iter()
                .copied()
                .filter(|value| *value >= -1.0)
                .max_by(f32::total_cmp);
            let similarity = assigned
                .map(|face_index| scores[reference_index][face_index])
                .or(best_similarity);
            let mut sorted = scores[reference_index]
                .iter()
                .copied()
                .filter(|value| *value >= SFACE_SUSPECTED_THRESHOLD)
                .collect::<Vec<_>>();
            sorted.sort_by(|left, right| right.total_cmp(left));
            let ambiguous = sorted.len() > 1 && sorted[0] - sorted[1] < AMBIGUITY_MARGIN;
            let status = if !has_embedding {
                "unknown"
            } else if ambiguous {
                "ambiguous"
            } else if assigned.is_some() {
                // The bundled threshold has not passed the independent
                // real-photo identity release gate, so high-scoring matches
                // remain suspected instead of being auto-confirmed.
                "suspected"
            } else if best_similarity.is_some_and(|value| value < SFACE_CLEAR_MISSING_THRESHOLD) {
                "missing"
            } else {
                "unknown"
            };
            KeyPersonEvidence {
                priority: reference.priority,
                face_index: assigned,
                similarity,
                status: status.to_string(),
                auto_score_eligible: false,
                performance_rank: None,
            }
        })
        .collect()
}

fn maximum_weight_assignment(scores: &[Vec<f32>], threshold: f32) -> Vec<Option<usize>> {
    let row_count = scores.len();
    if row_count == 0 {
        return Vec::new();
    }
    let face_count = scores.first().map_or(0, Vec::len);
    let column_count = face_count + row_count;
    let mut u = vec![0.0f64; row_count + 1];
    let mut v = vec![0.0f64; column_count + 1];
    let mut p = vec![0usize; column_count + 1];
    let mut way = vec![0usize; column_count + 1];

    for row in 1..=row_count {
        p[0] = row;
        let mut column = 0;
        let mut min_value = vec![f64::INFINITY; column_count + 1];
        let mut used = vec![false; column_count + 1];
        loop {
            used[column] = true;
            let active_row = p[column];
            let mut delta = f64::INFINITY;
            let mut next_column = 0;
            for candidate_column in 1..=column_count {
                if used[candidate_column] {
                    continue;
                }
                let weight = if candidate_column <= face_count {
                    scores[active_row - 1][candidate_column - 1]
                } else {
                    threshold
                };
                let cost = -(weight as f64) - u[active_row] - v[candidate_column];
                if cost < min_value[candidate_column] {
                    min_value[candidate_column] = cost;
                    way[candidate_column] = column;
                }
                if min_value[candidate_column] < delta {
                    delta = min_value[candidate_column];
                    next_column = candidate_column;
                }
            }
            for candidate_column in 0..=column_count {
                if used[candidate_column] {
                    u[p[candidate_column]] += delta;
                    v[candidate_column] -= delta;
                } else {
                    min_value[candidate_column] -= delta;
                }
            }
            column = next_column;
            if p[column] == 0 {
                break;
            }
        }
        loop {
            let previous = way[column];
            p[column] = p[previous];
            column = previous;
            if column == 0 {
                break;
            }
        }
    }

    let mut assignments = vec![None; row_count];
    for column in 1..=face_count {
        if p[column] > 0 && scores[p[column] - 1][column - 1] >= threshold {
            assignments[p[column] - 1] = Some(column - 1);
        }
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_only_finite_128_value_embeddings() {
        let normalized = normalize_embedding(vec![2.0; 128]).unwrap();
        let norm = normalized.iter().map(|value| value * value).sum::<f32>();
        assert!((norm - 1.0).abs() < 1e-5);
        assert!(normalize_embedding(vec![1.0; 127]).is_err());
        assert!(normalize_embedding(vec![f32::NAN; 128]).is_err());
    }

    #[test]
    fn assignment_is_one_face_per_person_and_prefers_global_weight() {
        let scores = vec![vec![0.91, 0.90], vec![0.89, 0.40]];
        assert_eq!(
            maximum_weight_assignment(&scores, SFACE_SUSPECTED_THRESHOLD),
            vec![Some(1), Some(0)]
        );
    }

    #[test]
    fn assignment_uses_unknown_when_every_face_is_below_threshold() {
        let scores = vec![vec![0.20, 0.30], vec![0.10, 0.20]];
        assert_eq!(
            maximum_weight_assignment(&scores, SFACE_SUSPECTED_THRESHOLD),
            vec![None, None]
        );
    }

    #[test]
    fn official_template_maps_to_itself() {
        let landmarks = SFACE_TEMPLATE.map(|(x, y)| (x as f32, y as f32));
        let transform = similarity_transform(&landmarks).unwrap();
        assert!((transform[0] - 1.0).abs() < 1e-4);
        assert!(transform[1].abs() < 1e-4);
        assert!(transform[2].abs() < 1e-3);
        assert!(transform[3].abs() < 1e-4);
        assert!((transform[4] - 1.0).abs() < 1e-4);
        assert!(transform[5].abs() < 1e-3);
    }
}
