use once_cell::sync::Lazy;
use serde_json::{Value, json};

pub struct ExpertPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub tags: &'static [&'static str],
    pub adjustments: Value,
}

static EXPERT_PRESETS: Lazy<Vec<ExpertPreset>> = Lazy::new(|| {
    let zero_hsl = json!({
        "aquas": { "hue": 0, "saturation": 0, "luminance": 0 },
        "blues": { "hue": 0, "saturation": 0, "luminance": 0 },
        "greens": { "hue": 0, "saturation": 0, "luminance": 0 },
        "magentas": { "hue": 0, "saturation": 0, "luminance": 0 },
        "oranges": { "hue": 0, "saturation": 0, "luminance": 0 },
        "purples": { "hue": 0, "saturation": 0, "luminance": 0 },
        "reds": { "hue": 0, "saturation": 0, "luminance": 0 },
        "yellows": { "hue": 0, "saturation": 0, "luminance": 0 }
    });

    vec![
        ExpertPreset {
            id: "clean_natural",
            name: "清透自然",
            tags: &["mid_key", "neutral_temp", "neutral_sat", "soft_contrast"],
            adjustments: json!({
                "contrast": -6,
                "highlights": -6,
                "shadows": 6,
                "whites": -4,
                "blacks": 4,
                "saturation": -4,
                "vibrance": 2,
                "clarity": 0
            }),
        },
        ExpertPreset {
            id: "bright_airy",
            name: "明亮通透",
            tags: &["high_key", "neutral_temp", "low_sat", "soft_contrast"],
            adjustments: json!({
                "contrast": -10,
                "highlights": -8,
                "shadows": 14,
                "whites": -6,
                "blacks": 12,
                "saturation": -8,
                "vibrance": -2
            }),
        },
        ExpertPreset {
            id: "moody_matte",
            name: "暗调哑光",
            tags: &["low_key", "neutral_temp", "low_sat", "high_contrast"],
            adjustments: json!({
                "contrast": 10,
                "highlights": -10,
                "shadows": -6,
                "whites": -10,
                "blacks": 18,
                "saturation": -10,
                "vibrance": -6,
                "clarity": 4
            }),
        },
        ExpertPreset {
            id: "film_soft",
            name: "胶片柔和",
            tags: &["mid_key", "warm_temp", "low_sat", "soft_contrast"],
            adjustments: json!({
                "contrast": -12,
                "highlights": -8,
                "shadows": 8,
                "whites": -8,
                "blacks": 10,
                "temperature": 8,
                "saturation": -10,
                "vibrance": -6,
                "clarity": -4
            }),
        },
        ExpertPreset {
            id: "cinematic_teal_orange",
            name: "电影青橙",
            tags: &["mid_key", "warm_temp", "neutral_sat", "high_contrast"],
            adjustments: json!({
                "contrast": 8,
                "highlights": -10,
                "shadows": 6,
                "whites": -8,
                "blacks": 8,
                "temperature": 10,
                "tint": -4,
                "saturation": -4,
                "vibrance": 6,
                "hsl": {
                    "aquas": { "hue": -12, "saturation": -6, "luminance": 0 },
                    "blues": { "hue": -8, "saturation": -6, "luminance": 0 },
                    "greens": { "hue": -4, "saturation": -4, "luminance": 0 },
                    "magentas": { "hue": 0, "saturation": 0, "luminance": 0 },
                    "oranges": { "hue": 6, "saturation": 10, "luminance": 0 },
                    "purples": { "hue": 0, "saturation": 0, "luminance": 0 },
                    "reds": { "hue": 0, "saturation": 6, "luminance": 0 },
                    "yellows": { "hue": 4, "saturation": 6, "luminance": 0 }
                }
            }),
        },
        ExpertPreset {
            id: "cool_clean",
            name: "冷调干净",
            tags: &["mid_key", "cool_temp", "neutral_sat", "soft_contrast"],
            adjustments: json!({
                "contrast": -6,
                "highlights": -4,
                "shadows": 6,
                "whites": -4,
                "blacks": 6,
                "temperature": -10,
                "tint": 2,
                "saturation": -6,
                "vibrance": -2,
                "hsl": zero_hsl
            }),
        },
    ]
});

pub fn derive_style_tags(
    mean_luminance: f64,
    p10: f64,
    p90: f64,
    contrast_spread: f64,
    mean_saturation: f64,
    rb_ratio: f64,
) -> Vec<&'static str> {
    let mut tags = Vec::new();

    let tonal = if mean_luminance > 0.62 && p10 > 0.35 && p90 > 0.78 {
        "high_key"
    } else if mean_luminance < 0.42 && p90 < 0.74 {
        "low_key"
    } else {
        "mid_key"
    };
    tags.push(tonal);

    let contrast = if contrast_spread > 0.26 {
        "high_contrast"
    } else {
        "soft_contrast"
    };
    tags.push(contrast);

    let sat = if mean_saturation < 0.34 {
        "low_sat"
    } else if mean_saturation > 0.56 {
        "high_sat"
    } else {
        "neutral_sat"
    };
    tags.push(sat);

    let temp = if rb_ratio > 1.06 {
        "warm_temp"
    } else if rb_ratio < 0.96 {
        "cool_temp"
    } else {
        "neutral_temp"
    };
    tags.push(temp);

    tags
}

pub fn select_expert_preset(tags: &[&'static str]) -> Option<&'static ExpertPreset> {
    let mut best: Option<&ExpertPreset> = None;
    let mut best_score = -1i32;

    for preset in EXPERT_PRESETS.iter() {
        let mut score = 0i32;
        for t in tags {
            if preset.tags.contains(t) {
                score += 3;
            }
        }
        if score > best_score {
            best_score = score;
            best = Some(preset);
        }
    }

    best
}

pub fn get_expert_preset_by_id(id: &str) -> Option<&'static ExpertPreset> {
    EXPERT_PRESETS.iter().find(|p| p.id == id)
}

