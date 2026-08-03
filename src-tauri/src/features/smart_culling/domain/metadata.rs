use std::collections::BTreeSet;

use serde_json::Value;

const COLOR_TAG_PREFIX: &str = "color:";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MetadataSnapshot {
    pub rating: u8,
    pub tags: Vec<String>,
    pub feature_data: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SmartCullingRecord {
    pub source: String,
    pub rating: u8,
    pub color_label: Option<String>,
    pub locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MetadataOwnership {
    Unprotected,
    Ai(SmartCullingRecord),
    Manual,
}

pub(crate) fn classify_metadata_ownership(snapshot: &MetadataSnapshot) -> MetadataOwnership {
    if snapshot
        .feature_data
        .as_ref()
        .is_some_and(|value| !value.is_object())
    {
        return MetadataOwnership::Manual;
    }
    let color_labels = snapshot
        .tags
        .iter()
        .filter_map(|tag| tag.strip_prefix(COLOR_TAG_PREFIX))
        .collect::<Vec<_>>();
    let top_level_has_result = snapshot.rating > 0 || !color_labels.is_empty();

    let stored_record = snapshot
        .feature_data
        .as_ref()
        .and_then(|value| value.get("smartCullingV2"));
    let Some(record) = parse_record(snapshot.feature_data.as_ref()) else {
        return if top_level_has_result || stored_record.is_some() {
            MetadataOwnership::Manual
        } else {
            MetadataOwnership::Unprotected
        };
    };

    let color_matches = match record.color_label.as_deref() {
        Some(expected) => color_labels.as_slice() == [expected],
        None => color_labels.is_empty(),
    };
    let values_match = snapshot.rating == record.rating && color_matches;

    if record.locked {
        return MetadataOwnership::Manual;
    }
    if record.source == "manual" {
        return if values_match {
            MetadataOwnership::Unprotected
        } else {
            MetadataOwnership::Manual
        };
    }
    if record.source != "ai" {
        return MetadataOwnership::Manual;
    }

    if values_match {
        MetadataOwnership::Ai(record)
    } else {
        MetadataOwnership::Manual
    }
}

pub(crate) fn asset_is_protected<'a>(
    member_metadata: impl IntoIterator<Item = &'a MetadataSnapshot>,
) -> bool {
    member_metadata
        .into_iter()
        .any(|snapshot| classify_metadata_ownership(snapshot) == MetadataOwnership::Manual)
}

pub(crate) fn metadata_has_unknown_source(snapshot: &MetadataSnapshot) -> bool {
    let Some(feature_data) = snapshot.feature_data.as_ref() else {
        return false;
    };
    if !feature_data.is_object() {
        return true;
    }
    let Some(_) = feature_data.get("smartCullingV2") else {
        return false;
    };
    parse_record(Some(feature_data))
        .is_none_or(|record| record.source != "ai" && record.source != "manual")
}

pub(crate) fn asset_has_conflicting_results<'a>(
    member_metadata: impl IntoIterator<Item = &'a MetadataSnapshot>,
) -> bool {
    let mut results = BTreeSet::new();
    for snapshot in member_metadata {
        let has_record = snapshot
            .feature_data
            .as_ref()
            .and_then(|value| value.get("smartCullingV2"))
            .is_some();
        let mut colors = snapshot
            .tags
            .iter()
            .filter_map(|tag| tag.strip_prefix(COLOR_TAG_PREFIX))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !has_record && snapshot.rating == 0 && colors.is_empty() {
            continue;
        }
        colors.sort();
        results.insert((snapshot.rating, colors));
        if results.len() > 1 {
            return true;
        }
    }
    false
}

fn parse_record(feature_data: Option<&Value>) -> Option<SmartCullingRecord> {
    let value = feature_data?.get("smartCullingV2")?;
    let source = value.get("source")?.as_str()?.to_string();
    let rating = u8::try_from(value.get("rating")?.as_u64()?).ok()?;
    if rating > 5 {
        return None;
    }
    if value
        .get("colorLabel")
        .is_some_and(|color| !color.is_null() && !color.is_string())
        || value
            .get("locked")
            .is_some_and(|locked| !locked.is_boolean())
    {
        return None;
    }
    let color_label = value
        .get("colorLabel")
        .and_then(Value::as_str)
        .map(str::to_string);
    let locked = value
        .get("locked")
        .and_then(Value::as_bool)
        .unwrap_or(source == "manual");

    Some(SmartCullingRecord {
        source,
        rating,
        color_label,
        locked,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn snapshot(rating: u8, tags: &[&str], record: Option<Value>) -> MetadataSnapshot {
        MetadataSnapshot {
            rating,
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            feature_data: record.map(|record| json!({ "smartCullingV2": record })),
        }
    }

    #[test]
    fn protects_historical_result_without_reliable_ai_source() {
        assert_eq!(
            classify_metadata_ownership(&snapshot(4, &["color:green"], None)),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn accepts_ai_record_only_when_top_level_values_still_match() {
        let record = json!({ "source": "ai", "rating": 4, "colorLabel": "green" });
        assert!(matches!(
            classify_metadata_ownership(&snapshot(4, &["color:green"], Some(record))),
            MetadataOwnership::Ai(_)
        ));
    }

    #[test]
    fn turns_changed_ai_result_into_manual_protection() {
        let record = json!({ "source": "ai", "rating": 4, "colorLabel": "green" });
        assert_eq!(
            classify_metadata_ownership(&snapshot(0, &[], Some(record))),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn remembers_manual_cancellation_even_when_top_level_values_are_empty() {
        let record = json!({ "source": "manual", "rating": 0, "colorLabel": null });
        assert_eq!(
            classify_metadata_ownership(&snapshot(0, &[], Some(record))),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn does_not_treat_adjustments_only_metadata_as_protected() {
        assert_eq!(
            classify_metadata_ownership(&snapshot(0, &[], None)),
            MetadataOwnership::Unprotected
        );
    }

    #[test]
    fn protects_the_whole_raw_jpeg_asset_when_either_member_is_manual() {
        let raw = snapshot(
            4,
            &["color:green"],
            Some(json!({ "source": "ai", "rating": 4, "colorLabel": "green" })),
        );
        let jpeg = snapshot(5, &[], None);

        assert!(asset_is_protected([&raw, &jpeg]));
    }

    #[test]
    fn matching_unlocked_manual_metadata_is_eligible_for_future_ai_updates() {
        let record = json!({
            "source": "manual",
            "rating": 4,
            "colorLabel": "green",
            "locked": false
        });
        assert_eq!(
            classify_metadata_ownership(&snapshot(4, &["color:green"], Some(record))),
            MetadataOwnership::Unprotected
        );
    }

    #[test]
    fn a_new_user_edit_relocks_previously_unlocked_metadata() {
        let record = json!({
            "source": "manual",
            "rating": 4,
            "colorLabel": "green",
            "locked": false
        });
        assert_eq!(
            classify_metadata_ownership(&snapshot(5, &["color:green"], Some(record))),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn an_explicit_user_lock_protects_an_unchanged_ai_result() {
        let record = json!({
            "source": "ai",
            "rating": 4,
            "colorLabel": "green",
            "locked": true
        });
        assert_eq!(
            classify_metadata_ownership(&snapshot(4, &["color:green"], Some(record))),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn protects_an_unknown_or_malformed_smart_culling_source() {
        for record in [
            json!({ "source": "other", "rating": 0, "colorLabel": null }),
            json!({ "source": "ai" }),
            json!({ "source": "ai", "rating": 0, "colorLabel": 1 }),
            json!({ "source": "ai", "rating": 0, "locked": "yes" }),
        ] {
            assert_eq!(
                classify_metadata_ownership(&snapshot(0, &[], Some(record))),
                MetadataOwnership::Manual
            );
        }
        assert_eq!(
            classify_metadata_ownership(&MetadataSnapshot {
                rating: 0,
                tags: Vec::new(),
                feature_data: Some(json!("invalid-feature-data")),
            }),
            MetadataOwnership::Manual
        );
    }

    #[test]
    fn reports_unknown_sources_without_misclassifying_unrelated_feature_data() {
        let unrelated = MetadataSnapshot {
            rating: 0,
            tags: Vec::new(),
            feature_data: Some(json!({ "otherFeature": true })),
        };
        let unknown = snapshot(0, &[], Some(json!({ "source": "other", "rating": 0 })));

        assert!(!metadata_has_unknown_source(&unrelated));
        assert!(metadata_has_unknown_source(&unknown));
    }

    #[test]
    fn detects_conflicting_member_results_but_allows_an_empty_legacy_member() {
        let four_stars = snapshot(
            4,
            &["color:green"],
            Some(json!({
                "source": "ai",
                "rating": 4,
                "colorLabel": "green",
                "locked": false
            })),
        );
        let five_stars = snapshot(
            5,
            &["color:green"],
            Some(json!({
                "source": "ai",
                "rating": 5,
                "colorLabel": "green",
                "locked": false
            })),
        );
        let empty = snapshot(0, &[], None);

        assert!(asset_has_conflicting_results([&four_stars, &five_stars]));
        assert!(!asset_has_conflicting_results([&four_stars, &empty]));
    }
}
