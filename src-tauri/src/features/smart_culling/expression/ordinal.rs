//! Five-level ordinal expression head calibrated from frozen batches 001-003.
//!
//! This head learns the ordered manual expression target directly. It does not
//! infer eye state and it does not turn "unable to determine" into a sixth
//! quality class. The surrounding reliability gate owns abstention.

use super::ExpressionDescriptor;
use crate::features::smart_culling::expression_quality_poc::ExpressionQualityModelOutputs;

pub(super) const MODEL_VERSION: &str = "qraw-expression-quality-ordinal-calibration-0.3";

const FEATURE_COUNT: usize = 56;

// Exact order: HSE MTL[0..10], HSE VGAF[0..8], then
// log1p(100 * non-eye Blendshape[0..38]). PCA was folded into this standardized
// linear head after selecting 15 components and L2=0.1 by leave-one-batch-out
// class-balanced negative log likelihood across the three frozen batches.
const FEATURE_MEANS: [f64; FEATURE_COUNT] = [
    -0.6776933329897298,
    1.184409535229996,
    -1.6331251449488078,
    -1.605222063106172,
    1.9105626502906896,
    1.1176623176559544,
    0.054756063191292566,
    -0.1532497686653171,
    0.1770700725460221,
    0.08989793509771461,
    -1.387937189728127,
    0.9589789595731366,
    -3.4351699271176814,
    -2.907618679119399,
    2.2293631568182484,
    1.6602138067618697,
    -0.12739581591038315,
    -0.010765478594143061,
    0.0005075178374300137,
    2.492954499865681,
    2.457992956047239,
    1.2001675852391407,
    1.3272767378344157,
    1.2010080867862616,
    0.015886191017663295,
    0.0000928508545737336,
    0.00006274107639452718,
    0.03465707517621524,
    0.18224442609091832,
    1.1290801959933243,
    0.05855861762869034,
    0.27866798600576365,
    0.778932310700538,
    0.6073881299327238,
    0.17534771519148457,
    0.19549410071486276,
    0.688391369696045,
    0.3390216451245074,
    0.528448279958487,
    0.6245367986309265,
    1.7228379368648974,
    1.6061660197190146,
    1.5728297818194492,
    0.17677250934026636,
    0.7060128860729005,
    0.6554115142355296,
    1.4542123250762502,
    1.3940424278812955,
    2.45164983365567,
    2.4158231833162254,
    0.6909715947805314,
    0.9808942803459167,
    1.2421188517885913,
    1.2729864983770336,
    0.00020247438661323474,
    0.00019205267346803368,
];

const FEATURE_STDS: [f64; FEATURE_COUNT] = [
    2.1507878950808346,
    2.459515018576604,
    2.097415360805679,
    2.4985503337518384,
    3.585898036416735,
    2.1624225501653775,
    2.160583621163251,
    2.202033547654112,
    0.5112967314712527,
    0.3156089360811979,
    4.021608141953725,
    3.5283807922100374,
    4.252086227343572,
    3.936906578208196,
    3.345167473697786,
    3.383544635247512,
    3.5737575328114723,
    2.349119907877074,
    0.0006062474620512806,
    1.2990488165378975,
    1.3049130421665314,
    1.2867536208611803,
    1.1606396494903652,
    1.056822075345676,
    0.027475938754420774,
    0.0001357040532252286,
    0.00007850294911329295,
    0.05116997819726031,
    0.46588079751808026,
    1.2715526931969654,
    0.2873330860603346,
    0.3044160316432315,
    0.6104907181211648,
    0.5960250949117198,
    0.4051245087177487,
    0.4266274803646595,
    0.8879413556444933,
    0.6084836972719376,
    0.8317132795515476,
    0.9750665223816007,
    0.9327070155014519,
    0.9865689521455306,
    1.3319393757991371,
    0.5402903880357024,
    0.7527108009120699,
    0.7467731380117434,
    1.2050016881818792,
    0.7628434908788729,
    1.8098450729040665,
    1.7814970287904048,
    0.7369528275411869,
    0.8972966393461734,
    1.5443936681887938,
    1.5492905388358147,
    0.0003110349154129115,
    0.00024677011683147375,
];

const WEIGHTS: [f64; FEATURE_COUNT] = [
    -0.12359022573672448,
    0.0152151814851741,
    -0.4179182058709327,
    -0.10643032372683786,
    0.2727310232009107,
    0.10844047212321728,
    -0.1022235623515595,
    0.15773488961854418,
    0.18069977906205592,
    -0.1345258947590752,
    -0.3672803131059654,
    -0.30903993044852257,
    -0.5940491472747922,
    -0.39503776218619935,
    0.2575265442818682,
    -0.13966495173888785,
    -0.2891889526889013,
    -0.12219632877848843,
    -0.04643625622070664,
    0.07164219570632692,
    0.03179479669007717,
    -0.11965086698153839,
    -0.07222750776992996,
    -0.049194156597168824,
    0.0744698495895519,
    -0.03392225240041175,
    -0.08515749183485867,
    0.023306000488663315,
    -0.13021562325583688,
    -0.07158105893213347,
    -0.2672314995327129,
    -0.03661729215708878,
    -0.15512062231692414,
    0.03520036769365072,
    -0.06745674706944571,
    -0.07368472092046222,
    -0.004754774477358346,
    -0.16925534698297715,
    0.007906812717430928,
    0.05518925710907491,
    0.06405684935229589,
    0.10622550845170357,
    0.02150590112502598,
    -0.22211860541986148,
    -0.10951718137187937,
    -0.08890767422981477,
    0.05056882532555102,
    0.03717273755643272,
    -0.07205904524217632,
    -0.064047131560391,
    -0.08016217297774769,
    -0.19031304075355293,
    0.051431788701022496,
    0.05660228667799692,
    -0.007032900147206664,
    -0.1432684502413161,
];

const THRESHOLDS: [f64; 4] = [
    -3.0280060905921395,
    -1.0432086965905214,
    0.9752735616806021,
    3.439702785940014,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QualityLevel {
    SevereFailure,
    NotRecommended,
    Natural,
    Excellent,
    Outstanding,
}

impl QualityLevel {
    pub(super) const fn state(self) -> &'static str {
        match self {
            Self::SevereFailure => "severe_failure",
            Self::NotRecommended => "not_recommended",
            Self::Natural => "natural",
            Self::Excellent => "excellent",
            Self::Outstanding => "outstanding",
        }
    }

    pub(super) const fn normalized_score(self) -> f32 {
        match self {
            Self::SevereFailure => 0.0,
            Self::NotRecommended => 0.25,
            Self::Natural => 0.5,
            Self::Excellent => 0.75,
            Self::Outstanding => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Classification {
    pub(super) level: QualityLevel,
    pub(super) probability: f32,
}

pub(super) fn classify(
    descriptor: &ExpressionDescriptor,
    model_outputs: &ExpressionQualityModelOutputs,
) -> Option<Classification> {
    let values = model_outputs
        .mtl
        .iter()
        .chain(&model_outputs.vgaf)
        .map(|value| f64::from(*value))
        .chain(
            descriptor
                .non_eye_blendshapes()
                .iter()
                .map(|value| (1.0 + 100.0 * f64::from(*value)).ln()),
        );
    let mut count = 0usize;
    let score = values
        .zip(FEATURE_MEANS)
        .zip(FEATURE_STDS)
        .zip(WEIGHTS)
        .try_fold(0.0, |sum, (((value, mean), std), weight)| {
            count += 1;
            value
                .is_finite()
                .then_some(sum + ((value - mean) / std) * weight)
        })?;
    if count != FEATURE_COUNT || !score.is_finite() {
        return None;
    }
    classify_score(score)
}

fn classify_score(score: f64) -> Option<Classification> {
    let cumulative = THRESHOLDS.map(|threshold| sigmoid(threshold - score));
    let probabilities = [
        cumulative[0],
        cumulative[1] - cumulative[0],
        cumulative[2] - cumulative[1],
        cumulative[3] - cumulative[2],
        1.0 - cumulative[3],
    ];
    if probabilities
        .iter()
        .any(|value| !value.is_finite() || *value < -1e-9)
    {
        return None;
    }
    let (index, probability) = probabilities
        .into_iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(&right.1))?;
    let level = match index {
        0 => QualityLevel::SevereFailure,
        1 => QualityLevel::NotRecommended,
        2 => QualityLevel::Natural,
        3 => QualityLevel::Excellent,
        4 => QualityLevel::Outstanding,
        _ => return None,
    };
    Some(Classification {
        level,
        probability: probability as f32,
    })
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value.clamp(-40.0, 40.0)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_score_moves_only_from_worse_to_better_levels() {
        let levels = [-8.0, -2.0, 0.0, 2.0, 8.0].map(|score| classify_score(score).unwrap().level);

        assert_eq!(
            levels,
            [
                QualityLevel::SevereFailure,
                QualityLevel::NotRecommended,
                QualityLevel::Natural,
                QualityLevel::Excellent,
                QualityLevel::Outstanding,
            ]
        );
    }

    #[test]
    fn every_frozen_threshold_is_strictly_ordered() {
        assert!(THRESHOLDS.windows(2).all(|values| values[0] < values[1]));
        assert_eq!(
            MODEL_VERSION,
            "qraw-expression-quality-ordinal-calibration-0.3"
        );
    }
}
