mod asset;
mod metadata;
mod result;
mod task;

pub(crate) use asset::{AssetCandidate, AssetDecision, AssetMemberKind, SkipReason, group_assets};
pub(crate) use metadata::{
    MetadataOwnership, MetadataSnapshot, asset_is_protected, classify_metadata_ownership,
};
pub(crate) use result::{ColorLabel, ConfirmedResult, ResultSource};
pub(crate) use task::TaskState;
