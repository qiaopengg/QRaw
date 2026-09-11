mod baseline;
mod catalog;
mod clarity_input;
mod decode_snapshot;
mod locking;
mod manual_reconciliation;
mod persistence;
mod render_input;
mod sidecar_transaction;

#[cfg(test)]
mod test_images;

pub(crate) use baseline::capture_sidecar_baseline;
pub(crate) use catalog::{
    Catalog, CatalogAsset, CatalogAssetStatus, CatalogSkipReason, scan_catalog,
};
pub(crate) use clarity_input::{ClarityInput, ClarityInputUnavailable};
pub(crate) use locking::change_asset_lock_state;
pub(crate) use manual_reconciliation::reconcile_manual_ownership;
pub(crate) use persistence::{
    ApplyFailureReason, ApplyReport, ConfirmedWrite, apply_confirmed_results,
};
pub(crate) use render_input::{render_analysis_input, render_current_state};
