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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MetadataOwnership {
    Unprotected,
    Ai(SmartCullingRecord),
    Manual,
}

pub(crate) fn classify_metadata_ownership(snapshot: &MetadataSnapshot) -> MetadataOwnership {
    let color_labels = snapshot
        .tags
        .iter()
        .filter_map(|tag| tag.strip_prefix(COLOR_TAG_PREFIX))
        .collect::<Vec<_>>();
    let top_level_has_result = snapshot.rating > 0 || !color_labels.is_empty();

    let Some(record) = parse_record(snapshot.feature_data.as_ref()) else {
        return if top_level_has_result {
            MetadataOwnership::Manual
        } else {
            MetadataOwnership::Unprotected
        };
    };

    if record.source == "manual" {
        return MetadataOwnership::Manual;
    }
    if record.source != "ai" {
        return if top_level_has_result {
            MetadataOwnership::Manual
        } else {
            MetadataOwnership::Unprotected
        };
    }

    let color_matches = match record.color_label.as_deref() {
        Some(expected) => color_labels.as_slice() == [expected],
        None => color_labels.is_empty(),
    };
    if snapshot.rating == record.rating && color_matches {
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

fn parse_record(feature_data: Option<&Value>) -> Option<SmartCullingRecord> {
    let value = feature_data?.get("smartCullingV2")?;
    let source = value.get("source")?.as_str()?.to_string();
    let rating = u8::try_from(value.get("rating")?.as_u64()?).ok()?;
    if rating > 5 {
        return None;
    }
    let color_label = value
        .get("colorLabel")
        .and_then(Value::as_str)
        .map(str::to_string);

    Some(SmartCullingRecord {
        source,
        rating,
        color_label,
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
}
