mod asset;
mod metadata;
mod result;
mod task;

pub(crate) use asset::{AssetCandidate, AssetDecision, AssetMemberKind, SkipReason, group_assets};
pub(crate) use metadata::{
    MetadataOwnership, MetadataSnapshot, asset_has_conflicting_results, asset_is_protected,
    classify_metadata_ownership, metadata_has_unknown_source,
};
pub(crate) use result::{ColorLabel, ConfirmedResult, ResultSource};
pub(crate) use task::TaskState;
