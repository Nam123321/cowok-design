// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (POSIX BYOS CLI Sandbox, 26+ Adapters, Unified BYOK Gateway & Post-Execution Normalizer)
//! Execution kernel for `cowok-design`:
//! - Isolated `$COWOK_SESSION_DIR` (`0o700`, `O_NOFOLLOW`, Zero-Global-Pollution)
//! - POSIX `0o600` FD prompt staging with `O_CLOEXEC` and RAII zeroize + unlink on drop/panic
//! - Strict `env_clear()` allowlist + `setpgid(0, 0)` + `kill(-pgid, SIGKILL)` + `waitpid` reaping
//! - 26+ data-driven BYOS CLI adapters
//! - Unified BYOK Gateway with DNS-pinned SSRF protection, tenant-scoped 429 cooldown & failover
//! - Post-Execution Normalizer (ANSI strip, JSON-first coercion, hardened SVG XSS + `viewBox` parity)

pub mod adapters;
pub mod byok_gateway;
pub mod normalizer;
pub mod posix_sandbox;
pub mod session_dir;

use thiserror::Error;

pub use adapters::{
    validate_model_id, CliAdapterCategory, CliAdapterRegistry, CliAdapterSpec, CliAdapterStatusDto,
};
pub use byok_gateway::{
    canonical_provider_id, default_model_for_provider, resolve_and_pin_provider_endpoint,
    ByokCredential, ByokExecutionResult, ByokGatewayRouter, CooldownTracker, ProviderCooldownDto,
    CANONICAL_BYOK_PROVIDERS, PROHIBITED_LOCAL_PORTS,
};
pub use normalizer::{
    coerce_json_value, extract_fenced_or_raw_json, normalize_svg_viewboxes, strip_ansi_sequences,
    NormalizationReport, PostExecutionNormalizer,
};
pub use posix_sandbox::{
    redact_secrets_from_text, PgidProcessSupervisor, PromptTransport, SandboxExecutionOutput,
    SandboxExecutionSpec, StagedPromptFile, SANDBOX_PARENT_ENV_ALLOWLIST,
};
pub use session_dir::{validate_session_id, IsolatedSessionDir};

#[derive(Debug, Error)]
pub enum BotFleetError {
    #[error("INVALID_SESSION_SEGMENT: {0}")]
    InvalidSessionSegment(String),
    #[error("INVALID_MODEL_ID: {0}")]
    InvalidModelId(String),
    #[error("PERMISSION_VIOLATION: {0}")]
    PermissionViolation(String),
    #[error("SYMLINK_VIOLATION: {0}")]
    SymlinkViolation(String),
    #[error("PATH_TRAVERSAL: {0}")]
    PathTraversal(String),
    #[error("INVALID_PATH: {0}")]
    InvalidPath(String),
    #[error("ARGV_SECRET_LEAK_BLOCKED: {0}")]
    ArgvSecretLeakBlocked(String),
    #[error("ADAPTER_NOT_FOUND: {0}")]
    AdapterNotFound(String),
    #[error("BINARY_NOT_INSTALLED: {0}")]
    BinaryNotInstalled(String),
    #[error("SPAWN_FAILED: {0}")]
    SpawnFailed(String),
    #[error("NO_CONFIGURED_PROVIDER: {0}")]
    NoConfiguredProvider(String),
    #[error("INVALID_ENDPOINT: {0}")]
    InvalidEndpoint(String),
    #[error("RATE_LIMITED_OR_UNAVAILABLE: {message}")]
    RateLimitedOrUnavailable {
        message: String,
        cooldown_applied: bool,
        failover_providers: Vec<String>,
        retry_after_secs: Option<u64>,
    },
    #[error("GATEWAY_ERROR: {0}")]
    GatewayError(String),
    #[error("SERIALIZATION_ERROR: {0}")]
    Serialization(String),
    #[error("IO_ERROR: {0}")]
    Io(String),
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
