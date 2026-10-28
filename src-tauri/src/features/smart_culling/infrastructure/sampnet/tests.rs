use super::{SampNetModel, contract};
use ndarray::{Array2, Array4, ArrayD, IxDyn};

#[test]
fn rejects_unaccepted_model_before_loading_the_gpu_runtime() {
    let directory = tempfile::tempdir().unwrap();
    let model = directory.path().join("samp_net.onnx");
    std::fs::write(&model, b"unaccepted conversion").unwrap();
    let error = SampNetModel::load(&model).err().unwrap();
    assert!(error.to_string().contains("integrity check failed"));
}

#[test]
fn accepts_only_finite_fixed_shape_inputs_and_bounded_saliency() {
    let mut image = Array4::zeros((1, 3, 224, 224));
    let mut saliency = Array4::zeros((1, 1, 224, 224));
    assert!(contract::validate_inputs(&image, &saliency).is_ok());
    for shape in [(2, 3, 224, 224), (1, 1, 224, 224), (1, 3, 223, 224)] {
        assert!(contract::validate_inputs(&Array4::zeros(shape), &saliency).is_err());
    }
    assert!(contract::validate_inputs(&image, &Array4::zeros((1, 1, 224, 223))).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        image[[0, 0, 0, 0]] = value;
        assert!(contract::validate_inputs(&image, &saliency).is_err());
    }
    image[[0, 0, 0, 0]] = 0.0;
    for value in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
        saliency[[0, 0, 0, 0]] = value;
        assert!(contract::validate_inputs(&image, &saliency).is_err());
    }
}

fn outputs(distribution: [f32; 5]) -> [ArrayD<f32>; 3] {
    [
        Array2::from_shape_vec((1, 8), vec![-2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0])
            .unwrap()
            .into_dyn(),
        Array2::from_shape_vec((1, 6), vec![-3.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .unwrap()
            .into_dyn(),
        Array2::from_shape_vec((1, 5), distribution.to_vec())
            .unwrap()
            .into_dyn(),
    ]
}

#[test]
fn preserves_raw_logits_and_attributes_without_manufacturing_a_quality_verdict() {
    let values = outputs([0.2; 5]);
    let observation =
        contract::parse_outputs(values[0].view(), values[1].view(), values[2].view()).unwrap();
    assert_eq!(
        observation.pattern_logits,
        [-2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
    );
    assert_eq!(observation.attributes, [-3.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(observation.composition_distribution, [0.2; 5]);
}

#[test]
fn rejects_wrong_shape_or_nonfinite_values_in_every_output_head() {
    for (head, width) in [(0, 8), (1, 6), (2, 5)] {
        for shape in [vec![width], vec![2, width], vec![1, width + 1]] {
            let mut values = outputs([0.2; 5]);
            values[head] = ArrayD::zeros(IxDyn(&shape));
            assert!(
                contract::parse_outputs(values[0].view(), values[1].view(), values[2].view())
                    .is_err()
            );
        }
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut values = outputs([0.2; 5]);
            values[head][[0, 0]] = invalid;
            assert!(
                contract::parse_outputs(values[0].view(), values[1].view(), values[2].view())
                    .is_err()
            );
        }
    }
}

#[test]
fn enforces_probability_contract_without_renormalizing_outputs() {
    for distribution in [
        [0.0; 5],
        [0.3; 5],
        [-0.1, 0.1, 0.2, 0.3, 0.5],
        [1.1, 0.0, 0.0, 0.0, 0.0],
        [0.2, 0.2, 0.2, 0.2, 0.200002],
    ] {
        let values = outputs(distribution);
        assert!(
            contract::parse_outputs(values[0].view(), values[1].view(), values[2].view()).is_err()
        );
    }
    let distribution = [0.2, 0.2, 0.2, 0.2, 0.2000003];
    let values = outputs(distribution);
    let observed =
        contract::parse_outputs(values[0].view(), values[1].view(), values[2].view()).unwrap();
    assert_eq!(observed.composition_distribution, distribution);
}
