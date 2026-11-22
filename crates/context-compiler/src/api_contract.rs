// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
//! Canonical public DTO and HTTP/IPC contract surface for `crates/context-compiler`.

pub use crate::compiler::{
    assemble_and_record, compile_context_only, inspect_prompt_cli, inspect_prompt_from_args,
};
pub use crate::telemetry::{list_context_telemetry, record_idempotent_or_replay};
pub use crate::types::*;
