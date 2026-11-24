// @story 01.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
//! # Spec Registry Crate (`crates/spec-registry`) for `cowok-design`
//!
//! Implements the 16-table embedded SQLite WAL database (`SqlitePool::open_wal`),
//! per-spec inter-process advisory locking, durable `spec_write_intents` crash recovery,
//! atomic `.tmp.<uuid>` -> `sync_all()` -> `rename()` two-way file synchronization,
//! `StaleFile` SHA-256 optimistic concurrency control (OCC), and `notify` Hash-Gated
//! Echo Suppression.

pub mod context_slicer;
pub mod errors;
pub mod exports;
pub mod lock;
pub mod locks;
pub mod plugin_store;
pub mod pool;
pub mod presentation_repo;
pub mod recovery;
pub mod registry;
pub mod reverse_sync;
pub mod skill_router;
pub mod sync;
pub mod watcher;
pub mod write_actor;

pub use context_slicer::{estimate_tokens, ContextSlicer};
pub use errors::{is_sqlite_busy_or_locked, map_sqlite_error, retry_on_busy};
pub use lock::SpecLockGuard;
pub use pool::{SqlitePool, SqliteWalConfig};
pub use recovery::recover_pending_spec;
pub use registry::{SpecRecord, SpecRegistry, SpecRevisionRecord, SyncOutcome, UpsertSpecInput};
pub use sync::{
    atomic_write_spec_file, compute_sha256_hex, compute_summary_digest, delete_jailed_spec_file,
    read_jailed_spec_bytes, remove_jailed_temp_file, resolve_jailed_spec_path,
    validate_candidate_spec_path,
};
pub use watcher::SpecWatcher;
pub use write_actor::{
    SqliteActorError, SqliteWriteActor, SqliteWriteActorHandle, WriteReceipt,
    DEFAULT_BACKPRESSURE_RETRY_AFTER_SECS, DEFAULT_WRITE_ACTOR_BUSY_TIMEOUT_MS,
    DEFAULT_WRITE_ACTOR_CAPACITY,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
