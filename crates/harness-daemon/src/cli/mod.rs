// @story 12.1
// @implements AC-01, AC-02, AC-03, AC-04 (Typed Zero-Dep CLI Argument Parser & Entrypoint for `cowok-design`)
pub mod commands;

use std::path::PathBuf;

use crate::visual_diff::{parse_viewport, VisualDiffMode};

pub const CLI_BIN_NAME: &str = "cowok-design";
pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    Html,
    Pdf,
    Pptx,
    Dev,
}

impl ExportFormat {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "html" | "html-bundle" => Some(Self::Html),
            "pdf" | "vector-pdf" => Some(Self::Pdf),
            "pptx" => Some(Self::Pptx),
            "dev" | "developer-handoff" => Some(Self::Dev),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Pptx => "pptx",
            Self::Dev => "dev",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpSubcommand {
    Serve,
    Install {
        host: String,
        rollback_backup_id: Option<String>,
        config_path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Up {
        port: Option<u16>,
        workspace: Option<PathBuf>,
        no_open: bool,
        json: bool,
    },
    Mcp {
        action: McpSubcommand,
        workspace: Option<PathBuf>,
        json: bool,
    },
    Export {
        format: ExportFormat,
        project_id: String,
        out_path: PathBuf,
        title: Option<String>,
        input_html: Option<PathBuf>,
        override_qc_warning: bool,
        workspace: Option<PathBuf>,
        json: bool,
    },
    Doctor {
        mode: String,
        probe: Option<String>,
        workspace: Option<PathBuf>,
        json: bool,
    },
    Qc {
        project_id: String,
        target_id: String,
        html_file: Option<PathBuf>,
        force_no_browser: bool,
        workspace: Option<PathBuf>,
        json: bool,
    },
    De {
        tool: Option<String>,
        html_file: Option<PathBuf>,
        workspace: Option<PathBuf>,
        json: bool,
    },
    Diff {
        mode: VisualDiffMode,
        out_path: PathBuf,
        baseline: Option<PathBuf>,
        viewport_width: u32,
        viewport_height: u32,
        ssim_threshold: f64,
        force_no_browser: bool,
        json: bool,
    },
    Feedback {
        title: String,
        body: String,
        category: String,
        email: Option<String>,
        include_diagnostics: bool,
        workspace: Option<PathBuf>,
        json: bool,
    },
    InspectPrompt {
        raw_args: Vec<String>,
        workspace: Option<PathBuf>,
    },
    Version {
        json: bool,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliParseError {
    pub code: &'static str,
    pub message: String,
    pub json_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    pub fn ok(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    pub fn err(exit_code: i32, stderr: String) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr,
        }
    }
}

pub fn render_help_text() -> String {
    format!(
        r#"{CLI_BIN_NAME} {CLI_VERSION} — Single-Binary Design Harness CLI & Standalone Local Workbench

USAGE:
  {CLI_BIN_NAME} <COMMAND> [OPTIONS]

COMMANDS:
  up          Start standalone loopback Axum server with /api/* -> /api/v1/* parity & embedded SPA
  mcp         Run Open MCP stdio server (`serve`) or install host config (`install <host>`)
  export      Export offline bundle (`html | pdf | pptx | dev`) sharing SQLite WAL & html-forge
  doctor      Inspect 23+ local AI CLIs & environment health (1:1 /api/v1/doctor/cli-matrix parity)
  qc          Run Pooled 1-Chromium Visual QC gate for a project (1:1 /api/v1/qc/run parity)
  de          Run 7 built-in Rust Design Engineering tools (1:1 /api/v1/de/run parity)
  diff        Run focus-free headless screenshot / suite visual diff with offline HTML report
  feedback    Submit Zero-PII scrubbed local support ticket (1:1 /api/v1/support/tickets parity)

GLOBAL OPTIONS:
  --json      Emit deterministic JSON envelope matching /api/v1/* HTTP schema
  -h, --help  Print this help message
  -V, --version Print version information
"#
    )
}

fn next_flag_value(
    args: &[String],
    idx: &mut usize,
    flag: &str,
    json: bool,
) -> Result<String, CliParseError> {
    *idx += 1;
    args.get(*idx).cloned().ok_or_else(|| CliParseError {
        code: "MISSING_FLAG_VALUE",
        message: format!("Option '{flag}' requires a value"),
        json_requested: json,
    })
}

pub fn parse_cli_args(args: &[String]) -> Result<CliCommand, CliParseError> {
    if args.is_empty() {
        return Ok(CliCommand::Help);
    }

    if args.iter().any(|a| a == "--inspect-prompt") {
        return Ok(CliCommand::InspectPrompt {
            raw_args: args.to_vec(),
            workspace: None,
        });
    }

    let mut global_json = false;
    let mut start = 0usize;
    while start < args.len() && args[start] == "--json" {
        global_json = true;
        start += 1;
    }

    let rest = &args[start..];
    if rest.is_empty() {
        return Ok(CliCommand::Help);
    }

    let json_anywhere = global_json || rest.iter().any(|a| a == "--json");
    let first = rest[0].as_str();
    match first {
        "-h" | "--help" | "help" => Ok(CliCommand::Help),
        "-V" | "--version" | "version" => Ok(CliCommand::Version {
            json: json_anywhere,
        }),
        "up" => parse_up(&rest[1..], global_json, json_anywhere),
        "mcp" => parse_mcp(&rest[1..], global_json, json_anywhere),
        "export" => parse_export(&rest[1..], global_json, json_anywhere),
        "doctor" => parse_doctor(&rest[1..], global_json, json_anywhere),
        "qc" => parse_qc(&rest[1..], global_json, json_anywhere),
        "de" => parse_de(&rest[1..], global_json, json_anywhere),
        "diff" => parse_diff(&rest[1..], global_json, json_anywhere),
        "feedback" => parse_feedback(&rest[1..], global_json, json_anywhere),
        other => Err(CliParseError {
            code: "UNKNOWN_SUBCOMMAND",
            message: format!("Unknown subcommand '{other}'. Run `{CLI_BIN_NAME} --help`."),
            json_requested: json_anywhere,
        }),
    }
}

fn parse_up(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut port = None;
    let mut workspace = None;
    let mut no_open = false;
    let mut json = global_json;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                let val = next_flag_value(args, &mut i, "--port", json_hint)?;
                let parsed = val.parse::<u16>().map_err(|_| CliParseError {
                    code: "INVALID_PORT",
                    message: format!("Invalid --port value '{val}' (expected 0..=65535)"),
                    json_requested: json_hint,
                })?;
                port = Some(parsed);
            }
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--no-open" => no_open = true,
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `up`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }
    Ok(CliCommand::Up {
        port,
        workspace,
        no_open,
        json,
    })
}

fn parse_mcp(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut workspace = None;
    let mut json = global_json;
    let mut action_str: Option<String> = None;
    let mut host: Option<String> = None;
    let mut rollback_backup_id: Option<String> = None;
    let mut config_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--rollback" => {
                rollback_backup_id = Some(next_flag_value(args, &mut i, "--rollback", json_hint)?);
            }
            "--config-path" => {
                config_path = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--config-path",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            arg if arg.starts_with('-') => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{arg}' for `mcp`"),
                    json_requested: json_hint,
                });
            }
            pos => {
                if action_str.is_none() {
                    action_str = Some(pos.to_string());
                } else if host.is_none() {
                    host = Some(pos.to_string());
                } else {
                    return Err(CliParseError {
                        code: "UNEXPECTED_ARGUMENT",
                        message: format!("Unexpected positional argument '{pos}' for `mcp`"),
                        json_requested: json_hint,
                    });
                }
            }
        }
        i += 1;
    }

    let action = match action_str.as_deref().unwrap_or("serve") {
        "serve" => McpSubcommand::Serve,
        "install" => {
            let target_host = host.ok_or_else(|| CliParseError {
                code: "MISSING_HOST",
                message: "Missing <host> for `mcp install <host>` (e.g. cursor, claude_code, codex, antigravity)".to_string(),
                json_requested: json_hint,
            })?;
            McpSubcommand::Install {
                host: target_host,
                rollback_backup_id,
                config_path,
            }
        }
        other => {
            return Err(CliParseError {
                code: "INVALID_MCP_ACTION",
                message: format!(
                    "Unsupported `mcp` action '{other}'; expected `serve` or `install`"
                ),
                json_requested: json_hint,
            });
        }
    };

    Ok(CliCommand::Mcp {
        action,
        workspace,
        json,
    })
}

fn parse_export(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        return Ok(CliCommand::Help);
    }
    let format = ExportFormat::parse(&args[0]).ok_or_else(|| CliParseError {
        code: "INVALID_EXPORT_FORMAT",
        message: format!(
            "Unsupported export format '{}'; expected one of: html, pdf, pptx, dev",
            args[0]
        ),
        json_requested: json_hint,
    })?;

    let mut project_id = None;
    let mut out_path = None;
    let mut title = None;
    let mut input_html = None;
    let mut override_qc_warning = false;
    let mut workspace = None;
    let mut json = global_json;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--project" => {
                project_id = Some(next_flag_value(args, &mut i, "--project", json_hint)?);
            }
            "--out" => {
                out_path = Some(PathBuf::from(next_flag_value(
                    args, &mut i, "--out", json_hint,
                )?));
            }
            "--title" => {
                title = Some(next_flag_value(args, &mut i, "--title", json_hint)?);
            }
            "--input-html" => {
                input_html = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--input-html",
                    json_hint,
                )?));
            }
            "--override-qc-warning" => override_qc_warning = true,
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `export`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }

    let project_id = project_id.ok_or_else(|| CliParseError {
        code: "MISSING_PROJECT",
        message: "Option `--project <id>` is required for `export`".to_string(),
        json_requested: json_hint,
    })?;
    let out_path = out_path.ok_or_else(|| CliParseError {
        code: "MISSING_OUT",
        message: "Option `--out <path>` is required for `export`".to_string(),
        json_requested: json_hint,
    })?;

    Ok(CliCommand::Export {
        format,
        project_id,
        out_path,
        title,
        input_html,
        override_qc_warning,
        workspace,
        json,
    })
}

fn parse_doctor(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut mode = "passive".to_string();
    let mut probe = None;
    let mut workspace = None;
    let mut json = global_json;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                let val = next_flag_value(args, &mut i, "--mode", json_hint)?;
                let lower = val.trim().to_ascii_lowercase();
                if lower != "passive" && lower != "active" {
                    return Err(CliParseError {
                        code: "INVALID_DOCTOR_MODE",
                        message: format!("Invalid --mode '{val}'; expected `passive` or `active`"),
                        json_requested: json_hint,
                    });
                }
                mode = lower;
            }
            "--probe" => {
                probe = Some(next_flag_value(args, &mut i, "--probe", json_hint)?);
            }
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `doctor`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }

    Ok(CliCommand::Doctor {
        mode,
        probe,
        workspace,
        json,
    })
}

fn parse_qc(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut project_id = None;
    let mut target_id = "default-screen".to_string();
    let mut html_file = None;
    let mut force_no_browser = false;
    let mut workspace = None;
    let mut json = global_json;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--project" => {
                project_id = Some(next_flag_value(args, &mut i, "--project", json_hint)?);
            }
            "--target-id" => {
                target_id = next_flag_value(args, &mut i, "--target-id", json_hint)?;
            }
            "--html-file" => {
                html_file = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--html-file",
                    json_hint,
                )?));
            }
            "--force-no-browser" => force_no_browser = true,
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `qc`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }

    let project_id = project_id.ok_or_else(|| CliParseError {
        code: "MISSING_PROJECT",
        message: "Option `--project <id>` is required for `qc`".to_string(),
        json_requested: json_hint,
    })?;

    Ok(CliCommand::Qc {
        project_id,
        target_id,
        html_file,
        force_no_browser,
        workspace,
        json,
    })
}

fn parse_de(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut tool = None;
    let mut html_file = None;
    let mut workspace = None;
    let mut json = global_json;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tool" => {
                tool = Some(next_flag_value(args, &mut i, "--tool", json_hint)?);
            }
            "--html-file" => {
                html_file = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--html-file",
                    json_hint,
                )?));
            }
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            arg if arg.starts_with('-') => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{arg}' for `de`"),
                    json_requested: json_hint,
                });
            }
            pos => {
                if tool.is_none() {
                    tool = Some(pos.to_string());
                } else {
                    return Err(CliParseError {
                        code: "UNEXPECTED_ARGUMENT",
                        message: format!("Unexpected argument '{pos}' for `de`"),
                        json_requested: json_hint,
                    });
                }
            }
        }
        i += 1;
    }

    Ok(CliCommand::De {
        tool,
        html_file,
        workspace,
        json,
    })
}

fn parse_diff(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut suite: Option<PathBuf> = None;
    let mut screenshot: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut viewport_width: u32 = 1440;
    let mut viewport_height: u32 = 900;
    let mut ssim_threshold: f64 = 0.98;
    let mut force_no_browser = false;
    let mut json = global_json;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--suite" => {
                suite = Some(PathBuf::from(next_flag_value(
                    args, &mut i, "--suite", json_hint,
                )?));
            }
            "--screenshot" => {
                screenshot = Some(next_flag_value(args, &mut i, "--screenshot", json_hint)?);
            }
            "--out" => {
                out_path = Some(PathBuf::from(next_flag_value(
                    args, &mut i, "--out", json_hint,
                )?));
            }
            "--baseline" => {
                baseline = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--baseline",
                    json_hint,
                )?));
            }
            "--viewport" => {
                let raw = next_flag_value(args, &mut i, "--viewport", json_hint)?;
                let (w, h) = parse_viewport(&raw).map_err(|msg| CliParseError {
                    code: "INVALID_VIEWPORT",
                    message: msg,
                    json_requested: json_hint,
                })?;
                viewport_width = w;
                viewport_height = h;
            }
            "--ssim-threshold" => {
                let raw = next_flag_value(args, &mut i, "--ssim-threshold", json_hint)?;
                let val = raw.parse::<f64>().map_err(|_| CliParseError {
                    code: "INVALID_SSIM_THRESHOLD",
                    message: format!("Invalid --ssim-threshold '{raw}'"),
                    json_requested: json_hint,
                })?;
                if !(0.0..=1.0).contains(&val) {
                    return Err(CliParseError {
                        code: "INVALID_SSIM_THRESHOLD",
                        message: "--ssim-threshold must be within 0.0..=1.0".to_string(),
                        json_requested: json_hint,
                    });
                }
                ssim_threshold = val;
            }
            "--force-no-browser" => force_no_browser = true,
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `diff`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }

    let mode = match (suite, screenshot) {
        (Some(dir), None) => VisualDiffMode::Suite(dir),
        (None, Some(target)) => VisualDiffMode::Screenshot(target),
        (Some(_), Some(_)) => {
            return Err(CliParseError {
                code: "CONFLICTING_DIFF_MODES",
                message: "Specify either `--suite <dir>` or `--screenshot <target>`, not both"
                    .to_string(),
                json_requested: json_hint,
            });
        }
        (None, None) => {
            return Err(CliParseError {
                code: "MISSING_DIFF_TARGET",
                message: "Either `--suite <dir>` or `--screenshot <target>` is required for `diff`"
                    .to_string(),
                json_requested: json_hint,
            });
        }
    };

    let out_path = out_path.ok_or_else(|| CliParseError {
        code: "MISSING_OUT",
        message: "Option `--out <path>` is required for `diff`".to_string(),
        json_requested: json_hint,
    })?;

    Ok(CliCommand::Diff {
        mode,
        out_path,
        baseline,
        viewport_width,
        viewport_height,
        ssim_threshold,
        force_no_browser,
        json,
    })
}

fn parse_feedback(
    args: &[String],
    global_json: bool,
    json_hint: bool,
) -> Result<CliCommand, CliParseError> {
    let mut title = None;
    let mut body = None;
    let mut category = "bug_report".to_string();
    let mut email = None;
    let mut include_diagnostics = true;
    let mut workspace = None;
    let mut json = global_json;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--title" => {
                title = Some(next_flag_value(args, &mut i, "--title", json_hint)?);
            }
            "--body" => {
                body = Some(next_flag_value(args, &mut i, "--body", json_hint)?);
            }
            "--category" => {
                category = next_flag_value(args, &mut i, "--category", json_hint)?;
            }
            "--email" => {
                email = Some(next_flag_value(args, &mut i, "--email", json_hint)?);
            }
            "--no-diagnostics" => include_diagnostics = false,
            "--workspace" => {
                workspace = Some(PathBuf::from(next_flag_value(
                    args,
                    &mut i,
                    "--workspace",
                    json_hint,
                )?));
            }
            "--json" => json = true,
            "-h" | "--help" => return Ok(CliCommand::Help),
            other => {
                return Err(CliParseError {
                    code: "UNKNOWN_OPTION",
                    message: format!("Unknown option '{other}' for `feedback`"),
                    json_requested: json_hint,
                });
            }
        }
        i += 1;
    }

    let title = title.ok_or_else(|| CliParseError {
        code: "MISSING_TITLE",
        message: "Option `--title <text>` is required for `feedback`".to_string(),
        json_requested: json_hint,
    })?;
    let body = body.ok_or_else(|| CliParseError {
        code: "MISSING_BODY",
        message: "Option `--body <text>` is required for `feedback`".to_string(),
        json_requested: json_hint,
    })?;

    Ok(CliCommand::Feedback {
        title,
        body,
        category,
        email,
        include_diagnostics,
        workspace,
        json,
    })
}

/// Top-level CLI runner invoked by `src/bin/cowok_design.rs`.
pub fn run_cli_from_env() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_cli_with_args(&args)
}

/// Executes parsed CLI args and writes stdout/stderr, returning the process exit code.
pub fn run_cli_with_args(args: &[String]) -> i32 {
    let output = match parse_cli_args(args) {
        Ok(cmd) => commands::execute_cli_command(cmd),
        Err(err) => {
            if err.json_requested {
                let payload = commands::format_json_error(err.code, &err.message, None);
                CliOutput {
                    exit_code: 2,
                    stdout: payload,
                    stderr: String::new(),
                }
            } else {
                CliOutput::err(2, format!("error [{}]: {}", err.code, err.message))
            }
        }
    };

    if !output.stdout.is_empty() {
        println!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprintln!("{}", output.stderr);
    }
    output.exit_code
}
