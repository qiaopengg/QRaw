use super::super::expression::ExpressionEvidence;
use super::super::expression_quality_poc::ExpressionQualityModelOutputs;
use super::BLENDSHAPE_NAMES;
use super::decision;
use super::evidence::FaceMotionEvidenceDump;
use super::eye_policy::EyeUsability;
use super::roi::FaceRoi;

fn reliable_evidence() -> FaceMotionEvidenceDump {
    FaceMotionEvidenceDump {
        roi: FaceRoi {
            center_x: 50.0,
            center_y: 50.0,
            width: 100.0,
            height: 100.0,
            rotation: 0.0,
        },
        face_presence: 0.99,
        tongue_out: 0.0,
        left_eye_aspect_ratio: Some(0.25),
        right_eye_aspect_ratio: Some(0.25),
        head_pitch_degrees: Some(0.0),
        head_yaw_degrees: Some(0.0),
        landmark_consistency_error: Some(0.01),
        left_eye: EyeUsability::Open,
        right_eye: EyeUsability::Open,
        overall_eye: EyeUsability::Open,
        blendshapes: BLENDSHAPE_NAMES
            .into_iter()
            .map(|name| (name, 0.0))
            .collect(),
    }
}

fn eye_snapshot(evidence: &FaceMotionEvidenceDump) -> (String, String, Option<f32>, Option<f32>) {
    let assessment = decision::assess(evidence, false);
    (
        assessment.left_eye().state.clone(),
        assessment.right_eye().state.clone(),
        assessment.left_eye().open_probability,
        assessment.right_eye().open_probability,
    )
}

#[test]
fn expression_fusion_cannot_change_the_frozen_eye_assessment() {
    let evidence = reliable_evidence();
    let before = eye_snapshot(&evidence);
    let descriptor = evidence.expression_descriptor().unwrap();
    let outputs = ExpressionQualityModelOutputs {
        mtl: [8.0; 10],
        vgaf: [-8.0; 8],
    };

    let expression = ExpressionEvidence::from_single_frame(&descriptor, &outputs);
    let after = eye_snapshot(&evidence);

    assert_eq!(expression.state, "scored");
    assert_eq!(before, after);
    assert_eq!(
        decision::assess(&evidence, false).policy_version(),
        "qraw-eye-policy-1.1"
    );
    assert_eq!(
        decision::assess(&evidence, false).model_contract_version(),
        "qraw-eye-model-contract-1.0"
    );
}
