// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
//! Portal-safe Context Slicer, Static-Prefix/Dynamic-Tail KV-Cache Compiler,
//! and 3-Profile Model Capability Adapter composition crate.

pub mod api_contract;
pub mod compiler;
pub mod outline;
pub mod portal_guard;
pub mod profile_adapter;
pub mod telemetry;
pub mod types;

pub use compiler::{
    assemble_and_record, compile_context_only, inspect_prompt_cli, inspect_prompt_from_args,
    DEFAULT_MAX_IMAGE_TOKENS_PER_TURN, MAX_IMAGE_TOKENS_CEILING, MAX_TEXT_BPE_TOKENS,
};
pub use outline::{MAX_OUTLINE_BPE_TOKENS, MIN_SSOT_DOM_RATIO};
pub use portal_guard::{
    normalize_intent_nfc, scan_text_for_portal, validate_all_portal_inputs,
    validate_portal_isolation,
};
pub use telemetry::{
    compute_request_fingerprint_sha256, list_context_telemetry, list_context_telemetry_scoped,
    record_idempotent_or_replay, verify_project_tenant_for_pool,
};
pub use types::*;
