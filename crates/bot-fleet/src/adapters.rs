// @story 11.1
// @implements AC-01, AC-04 (26+ Data-Driven BYOS CLI Adapters Catalog & Sandbox Spec Builder)
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::posix_sandbox::{PromptTransport, SandboxExecutionSpec};
use crate::BotFleetError;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Validates `modelId` against `^[A-Za-z0-9][A-Za-z0-9._:@/-]{0,127}$` (or without `/` when
/// `allow_slash == false`, e.g. Gemini URL path interpolation), rejecting `..`, `?`, `#`,
/// whitespace, control chars, and leading `-`.
pub fn validate_model_id(raw: &str, allow_slash: bool) -> Result<String, BotFleetError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed.contains("..")
        || trimmed.contains('?')
        || trimmed.contains('#')
        || trimmed.starts_with('-')
    {
        return Err(BotFleetError::InvalidModelId(
            "Invalid modelId format".into(),
        ));
    }
    let bytes = trimmed.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() {
        return Err(BotFleetError::InvalidModelId(
            "modelId must start with an ASCII alphanumeric character".into(),
        ));
    }
    for &b in &bytes[1..] {
        let allowed = b.is_ascii_alphanumeric()
            || matches!(b, b'.' | b'_' | b':' | b'@' | b'-')
            || (allow_slash && b == b'/');
        if !allowed {
            return Err(BotFleetError::InvalidModelId(
                "modelId contains unsupported character".into(),
            ));
        }
    }
    Ok(trimmed.to_string())
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        (meta.permissions().mode() & 0o111) != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliAdapterCategory {
    Cli,
    Ide,
    Local,
    Builtin,
}

impl CliAdapterCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Ide => "ide",
            Self::Local => "local",
            Self::Builtin => "builtin",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CliAdapterSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub category: CliAdapterCategory,
    pub prompt_transport: PromptTransport,
    pub base_args: &'static [&'static str],
    pub model_flag: Option<&'static str>,
    pub sandbox_flags: &'static [&'static str],
    pub env_keys: &'static [&'static str],
    pub config_subpaths: &'static [&'static str],
    pub supports_mcp_config: bool,
    pub default_model: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliAdapterStatusDto {
    pub id: String,
    pub name: String,
    pub command: String,
    pub category: String,
    pub prompt_transport: String,
    pub installed: bool,
    pub authenticated: bool,
    pub sandbox_ready: bool,
    pub supports_mcp_config: bool,
    pub default_model: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_reason: Option<String>,
}

/// Declarative catalog of 27 BYOS CLI adapters (`claude`, `codex`, `agy`, `gemini`, `cursor`,
/// `opencode`, `omp`, `ollama`, `aider`, `cline`, `goose`, `continue`, `windsurf`, `roo`, `kilo`,
/// `copilot`, `qwen`, `deepseek-cli`, `mistral-vibe`, `amazon-q`, `augment`, `amp`,
/// `factory-droid`, `kiro`, `trae`, `zed-ai`, `pi`).
/// Every adapter strictly transmits prompts via `0o600` stdin FD or `$COWOK_PROMPT_FILE` (NEVER `argv`).
pub const CLI_ADAPTER_CATALOG: &[CliAdapterSpec] = &[
    CliAdapterSpec {
        id: "claude",
        name: "Claude Code",
        command: "claude",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--print", "--output-format", "text"],
        model_flag: Some("--model"),
        sandbox_flags: &["--permission-mode", "plan"],
        env_keys: &["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"],
        config_subpaths: &[".claude/.credentials.json", ".claude.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "codex",
        name: "OpenAI Codex CLI",
        command: "codex",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["exec", "-"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox", "read-only"],
        env_keys: &["OPENAI_API_KEY", "CODEX_API_KEY"],
        config_subpaths: &[".codex/auth.json"],
        supports_mcp_config: true,
        default_model: "gpt-5.3-codex",
    },
    CliAdapterSpec {
        id: "agy",
        name: "Google Antigravity CLI",
        command: "agy",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["run", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox"],
        env_keys: &["GEMINI_API_KEY", "GOOGLE_API_KEY", "ANTIGRAVITY_API_KEY"],
        config_subpaths: &[
            ".gemini/oauth_creds.json",
            ".gemini/antigravity/mcp_config.json",
        ],
        supports_mcp_config: true,
        default_model: "gemini-2.5-pro",
    },
    CliAdapterSpec {
        id: "gemini",
        name: "Gemini CLI",
        command: "gemini",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--output-format", "text"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox"],
        env_keys: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        config_subpaths: &[
            ".gemini/oauth_creds.json",
            ".config/gemini/oauth_creds.json",
        ],
        supports_mcp_config: true,
        default_model: "gemini-2.5-pro",
    },
    CliAdapterSpec {
        id: "cursor",
        name: "Cursor Agent CLI",
        command: "cursor",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["agent", "--print"],
        model_flag: Some("--model"),
        sandbox_flags: &["--non-interactive"],
        env_keys: &["CURSOR_API_KEY", "OPENAI_API_KEY"],
        config_subpaths: &[".cursor/cli-config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "opencode",
        name: "OpenCode CLI",
        command: "opencode",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["run"],
        model_flag: Some("--model"),
        sandbox_flags: &["--non-interactive"],
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENCODE_API_KEY"],
        config_subpaths: &[
            ".local/share/opencode/auth.json",
            ".config/opencode/opencode.jsonc",
        ],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "omp",
        name: "Oh My Prompt CLI",
        command: "omp",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["exec"],
        model_flag: Some("--model"),
        sandbox_flags: &["--quiet"],
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY"],
        config_subpaths: &[".omp/config.json", ".omp/auth.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "ollama",
        name: "Ollama Local LLM",
        command: "ollama",
        category: CliAdapterCategory::Local,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["run"],
        model_flag: None,
        sandbox_flags: &[],
        env_keys: &["OLLAMA_API_KEY", "OLLAMA_HOST"],
        config_subpaths: &[".ollama/id_ed25519"],
        supports_mcp_config: false,
        default_model: "qwen2.5-coder:7b",
    },
    CliAdapterSpec {
        id: "aider",
        name: "Aider CLI",
        command: "aider",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StagedFileFlag("--message-file"),
        base_args: &["--yes", "--no-git"],
        model_flag: Some("--model"),
        sandbox_flags: &["--no-auto-commits"],
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".aider.conf.yml"],
        supports_mcp_config: false,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "cline",
        name: "Cline CLI",
        command: "cline",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["task", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--read-only"],
        env_keys: &["CLINE_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".cline/config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "goose",
        name: "Block Goose CLI",
        command: "goose",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StagedFileFlag("--instructions"),
        base_args: &["run", "--no-session"],
        model_flag: Some("--model"),
        sandbox_flags: &[],
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "DATABRICKS_HOST"],
        config_subpaths: &[".config/goose/config.yaml", ".config/goose/profiles.yaml"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "continue",
        name: "Continue Dev CLI",
        command: "cn",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--print"],
        model_flag: Some("--model"),
        sandbox_flags: &["--readonly"],
        env_keys: &["CONTINUE_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"],
        config_subpaths: &[".continue/config.json", ".continue/config.yaml"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "windsurf",
        name: "Windsurf CLI",
        command: "windsurf",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StagedFileEnvVar("COWOK_PROMPT_FILE"),
        base_args: &["chat", "--headless"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox"],
        env_keys: &["WINDSURF_API_KEY", "CODEIUM_API_KEY"],
        config_subpaths: &[".codeium/windsurf/mcp_config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "roo",
        name: "Roo Code CLI",
        command: "roo",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["run", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--mode", "ask"],
        env_keys: &["ROO_API_KEY", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".roo/config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "kilo",
        name: "Kilo Code CLI",
        command: "kilo",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["run", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--auto"],
        env_keys: &["KILOCODE_API_KEY", "OPENROUTER_API_KEY"],
        config_subpaths: &[".kilocode/config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "copilot",
        name: "GitHub Copilot CLI",
        command: "copilot",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--silent"],
        model_flag: Some("--model"),
        sandbox_flags: &["--deny-tool", "shell"],
        env_keys: &["GITHUB_TOKEN", "GH_TOKEN"],
        config_subpaths: &[".config/github-copilot/hosts.json", ".config/gh/hosts.yml"],
        supports_mcp_config: true,
        default_model: "gpt-4.1",
    },
    CliAdapterSpec {
        id: "qwen",
        name: "Qwen Code CLI",
        command: "qwen",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--output-format", "text"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox"],
        env_keys: &["QWEN_API_KEY", "DASHSCOPE_API_KEY", "OPENAI_API_KEY"],
        config_subpaths: &[".qwen/oauth_creds.json", ".qwen/settings.json"],
        supports_mcp_config: true,
        default_model: "qwen3-coder-plus",
    },
    CliAdapterSpec {
        id: "deepseek-cli",
        name: "DeepSeek CLI",
        command: "deepseek",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["chat", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--no-stream"],
        env_keys: &["DEEPSEEK_API_KEY"],
        config_subpaths: &[".deepseek/config.json"],
        supports_mcp_config: false,
        default_model: "deepseek-reasoner",
    },
    CliAdapterSpec {
        id: "mistral-vibe",
        name: "Mistral Vibe CLI",
        command: "vibe",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--non-interactive"],
        model_flag: Some("--model"),
        sandbox_flags: &["--read-only"],
        env_keys: &["MISTRAL_API_KEY"],
        config_subpaths: &[".vibe/config.toml"],
        supports_mcp_config: true,
        default_model: "devstral-latest",
    },
    CliAdapterSpec {
        id: "amazon-q",
        name: "Amazon Q Developer CLI",
        command: "q",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["chat", "--no-interactive"],
        model_flag: Some("--model"),
        sandbox_flags: &["--trust-no-tools"],
        env_keys: &["AWS_ACCESS_KEY_ID", "AWS_PROFILE"],
        config_subpaths: &[".aws/amazonq/config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "augment",
        name: "Augment Code CLI",
        command: "auggie",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--print"],
        model_flag: Some("--model"),
        sandbox_flags: &["--read-only"],
        env_keys: &["AUGMENT_API_TOKEN"],
        config_subpaths: &[".augment/settings.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "amp",
        name: "Sourcegraph Amp CLI",
        command: "amp",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--execute"],
        model_flag: Some("--model"),
        sandbox_flags: &["--no-color"],
        env_keys: &["AMP_API_KEY", "SRC_ACCESS_TOKEN"],
        config_subpaths: &[".config/amp/secrets.json", ".local/share/amp/secrets.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "factory-droid",
        name: "Factory Droid CLI",
        command: "droid",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["exec", "--stdin"],
        model_flag: Some("--model"),
        sandbox_flags: &["--auto", "low"],
        env_keys: &["FACTORY_API_KEY"],
        config_subpaths: &[".factory/config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "kiro",
        name: "Kiro CLI",
        command: "kiro-cli",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["chat", "--no-interactive"],
        model_flag: Some("--model"),
        sandbox_flags: &["--read-only"],
        env_keys: &["AWS_ACCESS_KEY_ID", "KIRO_API_KEY"],
        config_subpaths: &[".kiro/settings.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "trae",
        name: "Trae IDE CLI",
        command: "trae",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StagedFileEnvVar("COWOK_PROMPT_FILE"),
        base_args: &["agent", "--headless"],
        model_flag: Some("--model"),
        sandbox_flags: &["--sandbox"],
        env_keys: &["TRAE_API_KEY"],
        config_subpaths: &[".trae/user_config.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "zed-ai",
        name: "Zed AI CLI",
        command: "zed",
        category: CliAdapterCategory::Ide,
        prompt_transport: PromptTransport::StagedFileEnvVar("COWOK_PROMPT_FILE"),
        base_args: &["--foreground"],
        model_flag: Some("--model"),
        sandbox_flags: &[],
        env_keys: &["ZED_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"],
        config_subpaths: &[".config/zed/settings.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
    CliAdapterSpec {
        id: "pi",
        name: "Pi Coding Agent",
        command: "pi",
        category: CliAdapterCategory::Cli,
        prompt_transport: PromptTransport::StdinFd,
        base_args: &["--print"],
        model_flag: Some("--model"),
        sandbox_flags: &["--no-tools"],
        env_keys: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "PI_API_KEY"],
        config_subpaths: &[".pi/agent/auth.json", ".pi/agent/settings.json"],
        supports_mcp_config: true,
        default_model: "claude-sonnet-4-5",
    },
];

pub struct CliAdapterRegistry;

impl CliAdapterRegistry {
    pub fn catalog() -> &'static [CliAdapterSpec] {
        CLI_ADAPTER_CATALOG
    }

    pub fn get(adapter_id: &str) -> Option<&'static CliAdapterSpec> {
        let needle = adapter_id.trim().to_ascii_lowercase();
        CLI_ADAPTER_CATALOG
            .iter()
            .find(|a| a.id == needle || a.command == needle)
    }

    pub fn resolve_binary(command: &str) -> Option<PathBuf> {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                if !dir.is_absolute() {
                    continue;
                }
                let candidate = dir.join(command);
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        let mut standard_dirs = vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/bin"),
        ];
        if let Ok(home) = std::env::var("HOME") {
            let h = Path::new(&home);
            if h.is_absolute() {
                standard_dirs.push(h.join(".local/bin"));
                standard_dirs.push(h.join(".npm-global/bin"));
                standard_dirs.push(h.join(".cargo/bin"));
            }
        }
        for dir in standard_dirs {
            let candidate = dir.join(command);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn check_passive_auth(spec: &CliAdapterSpec) -> (bool, Option<String>) {
        if spec.id == "ollama" {
            return (true, None);
        }
        for key in spec.env_keys {
            if let Ok(val) = std::env::var(key) {
                if !val.trim().is_empty() {
                    return (true, None);
                }
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let home_path = Path::new(&home);
            for rel in spec.config_subpaths {
                let candidate = home_path.join(rel);
                if candidate.is_file() {
                    if let Ok(raw) = std::fs::read_to_string(&candidate) {
                        let trimmed = raw.trim();
                        if !trimmed.is_empty() && trimmed != "{}" && trimmed != "[]" {
                            return (true, None);
                        }
                    }
                }
            }
        }
        let expected = spec.env_keys.first().copied().unwrap_or("API_KEY");
        (
            false,
            Some(format!(
                "Missing {expected} or local credential configuration"
            )),
        )
    }

    /// Probes all 27 registered adapters passively (no subprocess spawn, zero home mutation).
    pub fn probe_all() -> Vec<CliAdapterStatusDto> {
        CLI_ADAPTER_CATALOG
            .iter()
            .map(|spec| {
                let installed = Self::resolve_binary(spec.command).is_some();
                let (authenticated, auth_reason) = if installed {
                    Self::check_passive_auth(spec)
                } else {
                    (
                        false,
                        Some(format!("Command '{}' not found in PATH", spec.command)),
                    )
                };
                let status = if !installed {
                    "not_installed"
                } else if authenticated {
                    "ready"
                } else {
                    "auth_needed"
                };
                CliAdapterStatusDto {
                    id: spec.id.to_string(),
                    name: spec.name.to_string(),
                    command: spec.command.to_string(),
                    category: spec.category.as_str().to_string(),
                    prompt_transport: spec.prompt_transport.label().to_string(),
                    installed,
                    authenticated,
                    sandbox_ready: installed && authenticated,
                    supports_mcp_config: spec.supports_mcp_config,
                    default_model: spec.default_model.to_string(),
                    status: status.to_string(),
                    auth_reason,
                }
            })
            .collect()
    }

    /// Builds a hardened `SandboxExecutionSpec` for a registered adapter.
    /// Prompt bytes are NEVER placed into `args`.
    pub fn build_sandbox_spec(
        adapter_id: &str,
        prompt: String,
        model_override: Option<&str>,
        timeout_ms: u64,
        binary_override: Option<PathBuf>,
    ) -> Result<SandboxExecutionSpec, BotFleetError> {
        let spec = Self::get(adapter_id).ok_or_else(|| {
            BotFleetError::AdapterNotFound(format!("Unknown CLI adapter '{adapter_id}'"))
        })?;
        let binary = match binary_override {
            Some(path) => path,
            None => Self::resolve_binary(spec.command).ok_or_else(|| {
                BotFleetError::BinaryNotInstalled(format!(
                    "CLI adapter '{}' requires '{}' in PATH",
                    spec.id, spec.command
                ))
            })?,
        };

        let mut args: Vec<String> = spec.base_args.iter().map(|s| (*s).to_string()).collect();
        let chosen_model = match model_override.map(str::trim).filter(|s| !s.is_empty()) {
            Some(m) => validate_model_id(m, true)?,
            None => spec.default_model.to_string(),
        };
        if let Some(flag) = spec.model_flag {
            args.push(flag.to_string());
            args.push(chosen_model);
        } else if spec.id == "ollama" {
            args.push(chosen_model);
        }
        for flag in spec.sandbox_flags {
            args.push((*flag).to_string());
        }

        Ok(SandboxExecutionSpec {
            binary,
            args,
            prompt: Zeroizing::new(prompt),
            prompt_transport: spec.prompt_transport,
            working_dir: None,
            extra_env: Vec::new(),
            timeout_ms: timeout_ms.clamp(500, 120_000),
            max_output_bytes: 512 * 1024,
            redact_secrets: Vec::new(),
        })
    }
}
