// @story 12.1
// @implements AC-01, AC-02, AC-03, AC-04 (CLI Subcommand Execution Engine with 1:1 /api/v1/* JSON Parity)
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Extension, Query};
use axum::Json;
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use html_forge::export_bundler::{
    build_developer_handoff, build_vector_pdf, bundle_offline_html, DeveloperHandoffInput,
    OfflineHtmlInput, VectorPdfInput,
};
use html_forge::pptx_ir::PptxExportMode;
use html_forge::pptx_layout::PptxExportInput;
use html_forge::pptx_ooxml::build_pptx_deck;
use html_forge::visual_qc::{QcTargetKind, CANONICAL_APP_VIEWPORTS, DEFAULT_SSIM_PASS_THRESHOLD};
use mcp_gateway::{
    default_tool_policies, format_stdio_frame, install_host_config, parse_stdio_frame,
    reject_symlinks_in_path, rollback_host_install, ActorKind, HostClientId, HostInstallOptions,
    McpGatewayEngine, PermissionTier, ToolPolicyRecord,
};
use plugin_host::{run_builtin_de_suite, BuiltInDeTool, DeSuiteInput};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use spec_registry::{retry_on_busy, SpecRegistry, SqlitePool, SqliteWalConfig};
use uuid::Uuid;

use crate::cli::{
    render_help_text, CliCommand, CliOutput, ExportFormat, McpSubcommand, CLI_BIN_NAME, CLI_VERSION,
};
use crate::daemon_auth::FlowPrincipal;
use crate::lockfile::{reject_if_symlink, CURRENT_SCHEMA_VERSION, DEFAULT_DAEMON_PORT};
use crate::port_binder::{resolve_runtime_storage_paths, ChildProcessReaper, RuntimeStoragePaths};
use crate::routes::doctor_vault::{
    handle_get_doctor_cli_matrix, handle_probe_cli_tool, DoctorMatrixQuery, ProbeCliToolRequest,
};
use crate::routes::mcp_gateway::McpHostInstallData;
use crate::routes::split_export::ensure_v30_tables;
use crate::routes::visual_qc::{evaluate_qc_for_project, QcRunRequest};
use crate::routes::DaemonAppState;
use crate::static_spa::{EmbeddedBuiltinSkillsRegistry, StaticSpaHandler};
use crate::support_edge::{
    create_support_ticket, ensure_support_edge_tables, CreateSupportTicketInput,
};
use crate::visual_diff::{run_visual_diff, VisualDiffRequest};
use crate::DaemonServer;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    request_id: String,
}

#[derive(Debug, Serialize)]
struct CliErrorEnvelope<'a> {
    error: CliErrorDetail<'a>,
}

pub fn format_json_success<T: Serialize>(data: &T) -> String {
    serde_json::to_string_pretty(&CliSuccessEnvelope {
        ok: true,
        data,
        request_id: Uuid::new_v4().to_string(),
    })
    .unwrap_or_else(|_| "{\"ok\":true}".to_string())
}

pub fn format_json_error(code: &str, message: &str, field: Option<&str>) -> String {
    serde_json::to_string_pretty(&CliErrorEnvelope {
        error: CliErrorDetail {
            code,
            message,
            field,
            request_id: Uuid::new_v4().to_string(),
        },
    })
    .unwrap_or_else(|_| format!("{{\"error\":{{\"code\":\"{code}\"}}}}"))
}

fn cli_err(
    exit_code: i32,
    code: &str,
    message: &str,
    field: Option<&str>,
    json: bool,
) -> CliOutput {
    if json {
        CliOutput {
            exit_code,
            stdout: format_json_error(code, message, field),
            stderr: String::new(),
        }
    } else {
        CliOutput::err(exit_code, format!("error [{code}]: {message}"))
    }
}

fn open_cli_storage(
    workspace: Option<&Path>,
) -> Result<(RuntimeStoragePaths, SqlitePool, Arc<SpecRegistry>), String> {
    let storage = resolve_runtime_storage_paths(workspace, None).map_err(|e| e.to_string())?;
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&storage.db_path, &storage.state_root))
        .map_err(|e| {
            format!(
                "Failed opening SQLite WAL at '{}': {e}",
                storage.db_path.display()
            )
        })?;
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    Ok((storage, pool, registry))
}

fn is_os_root_symlink(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        path == Path::new("/var") || path == Path::new("/tmp") || path == Path::new("/etc")
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), String> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!(
            "Output path '{}' cannot contain '..' traversal segments",
            path.display()
        ));
    }
    let anchored = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut current = PathBuf::new();
    for comp in anchored.components() {
        match comp {
            Component::RootDir => current.push(Path::new("/")),
            Component::Prefix(p) => current.push(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "Output path '{}' cannot contain '..' traversal segments",
                    path.display()
                ));
            }
            Component::Normal(seg) => {
                current.push(seg);
                if is_os_root_symlink(&current) {
                    continue;
                }
                match fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            return Err(format!(
                                "Symlink rejected: '{}' is a symlink",
                                current.display()
                            ));
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                    Err(err) => {
                        return Err(format!(
                            "Failed inspecting path '{}': {err}",
                            current.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn escape_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn validate_safe_out_path(path: &Path) -> Result<(), String> {
    reject_symlink_ancestors(path)?;
    reject_if_symlink(path).map_err(|e| format!("Symlink rejected: {e}"))?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        reject_symlink_ancestors(parent)?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed creating directory '{}': {e}", parent.display()))?;
        reject_symlink_ancestors(parent)?;
    }
    Ok(())
}

fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), String> {
    validate_safe_out_path(target)?;
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = parent.join(format!(".cowok-out-tmp-{}", Uuid::new_v4().simple()));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o644);
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let write_res = opts
        .open(&tmp_path)
        .and_then(|mut f| f.write_all(bytes).and_then(|_| f.sync_all()));
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed writing temp file: {err}"));
    }
    if let Err(err) = reject_if_symlink(target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Target became symlink: {err}"));
    }
    if let Err(err) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Failed renaming output '{}': {err}",
            target.display()
        ));
    }
    Ok(())
}

fn local_cli_principal() -> FlowPrincipal {
    FlowPrincipal {
        tenant_id: "local-default".to_string(),
        user_id: "local-cli".to_string(),
        role: "admin".to_string(),
    }
}

/// Executes any parsed `CliCommand` synchronously (spinning a local Tokio runtime only when needed).
pub fn execute_cli_command(cmd: CliCommand) -> CliOutput {
    match cmd {
        CliCommand::Help => CliOutput::ok(render_help_text()),
        CliCommand::Version { json } => {
            let skills = EmbeddedBuiltinSkillsRegistry::load();
            if json {
                CliOutput::ok(format_json_success(&serde_json::json!({
                    "name": CLI_BIN_NAME,
                    "version": CLI_VERSION,
                    "schemaVersion": CURRENT_SCHEMA_VERSION,
                    "embeddedSkillsCount": skills.skills.len(),
                    "deToolsCount": skills.de_tools.len(),
                })))
            } else {
                CliOutput::ok(format!("{CLI_BIN_NAME} {CLI_VERSION}"))
            }
        }
        CliCommand::Install {
            target,
            install_root,
            workspace,
            binary_path,
            json,
        } => {
            let ws_root = workspace.unwrap_or_else(|| {
                std::env::var_os("UXUI_OS_WORKSPACE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    })
            });
            let target_root = install_root.unwrap_or_else(|| ws_root.clone());
            match crate::plugin_pack::install_universal_pack(
                &target,
                &target_root,
                &ws_root,
                binary_path.as_deref(),
            ) {
                Ok(report) => {
                    if json {
                        CliOutput::ok(format_json_success(&report))
                    } else {
                        CliOutput::ok(format!(
                            "Installed universal plugin pack for {} target(s) into {}",
                            report.targets.len(),
                            target_root.display()
                        ))
                    }
                }
                Err(err) => {
                    let field = match err {
                        crate::plugin_pack::PluginPackError::UnsupportedTarget(_) => Some("target"),
                        _ => None,
                    };
                    cli_err(2, err.code(), &err.to_string(), field, json)
                }
            }
        }
        CliCommand::CiGate {
            project_id,
            target_id,
            html_path,
            max_memory_mb,
            force_no_browser,
            fail_on_unverified,
            workspace,
            json,
        } => {
            let req = crate::ci_gate::CiGateRequest {
                project_id: project_id
                    .unwrap_or_else(|| crate::ci_gate::DEFAULT_CI_PROJECT_ID.to_string()),
                target_id: target_id
                    .unwrap_or_else(|| crate::ci_gate::DEFAULT_CI_TARGET_ID.to_string()),
                html_path,
                max_memory_mb,
                force_no_browser,
                fail_on_unverified,
                workspace,
            };
            crate::ci_gate::execute_ci_gate_cli(req, json)
        }
        CliCommand::InspectPrompt {
            raw_args,
            workspace,
        } => {
            let root = workspace.unwrap_or_else(|| {
                std::env::var_os("UXUI_OS_WORKSPACE_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    })
            });
            match context_compiler::inspect_prompt_from_args(&raw_args, &root) {
                Ok(out) => CliOutput::ok(out),
                Err(err) => CliOutput::err(1, format!("inspect-prompt failed: {err}")),
            }
        }
        CliCommand::Doctor {
            mode,
            probe,
            workspace: _,
            json,
        } => run_async(exec_doctor(mode, probe, json)),
        CliCommand::Feedback {
            title,
            body,
            category,
            email,
            include_diagnostics,
            workspace,
            json,
        } => exec_feedback(
            title,
            body,
            category,
            email,
            include_diagnostics,
            workspace.as_deref(),
            json,
        ),
        CliCommand::Export {
            format,
            project_id,
            out_path,
            title,
            input_html,
            override_qc_warning,
            workspace,
            json,
        } => exec_export(
            format,
            project_id,
            out_path,
            title,
            input_html.as_deref(),
            override_qc_warning,
            workspace.as_deref(),
            json,
        ),
        CliCommand::Mcp {
            action,
            workspace,
            json,
        } => exec_mcp(action, workspace.as_deref(), json),
        CliCommand::Qc {
            project_id,
            target_id,
            html_file,
            force_no_browser,
            workspace,
            json,
        } => run_async(exec_qc(
            project_id,
            target_id,
            html_file,
            force_no_browser,
            workspace,
            json,
        )),
        CliCommand::De {
            tool,
            html_file,
            workspace,
            json,
        } => exec_de(
            tool.as_deref(),
            html_file.as_deref(),
            workspace.as_deref(),
            json,
        ),
        CliCommand::Diff {
            mode,
            out_path,
            baseline,
            viewport_width,
            viewport_height,
            ssim_threshold,
            force_no_browser,
            json,
        } => exec_diff(
            VisualDiffRequest {
                mode,
                out_path,
                baseline,
                viewport_width,
                viewport_height,
                ssim_threshold,
                force_no_browser,
            },
            json,
        ),
        CliCommand::Up {
            port,
            workspace,
            no_open,
            json,
        } => run_async(exec_up(port, workspace, no_open, json)),
    }
}

fn run_async<F: std::future::Future<Output = CliOutput>>(fut: F) -> CliOutput {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(fut))
    } else {
        match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(fut),
            Err(err) => CliOutput::err(1, format!("Failed starting async runtime: {err}")),
        }
    }
}

async fn exec_doctor(mode: String, probe: Option<String>, json: bool) -> CliOutput {
    let principal = local_cli_principal();
    if let Some(cli_id) = probe {
        match handle_probe_cli_tool(
            Extension(principal),
            Json(ProbeCliToolRequest {
                tool_id: cli_id.clone(),
                cli_id: String::new(),
            }),
        )
        .await
        {
            Ok(Json(dto)) => {
                if json {
                    CliOutput::ok(
                        serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".to_string()),
                    )
                } else {
                    CliOutput::ok(format!(
                        "{} ({}) — status={} installed={} auth={}",
                        dto.name, dto.command, dto.status, dto.installed, dto.authenticated
                    ))
                }
            }
            Err(_) => cli_err(
                2,
                "CLI_NOT_FOUND",
                &format!("CLI tool '{cli_id}' is not in supported doctor registry"),
                Some("probe"),
                json,
            ),
        }
    } else {
        match handle_get_doctor_cli_matrix(
            Extension(principal),
            Query(DoctorMatrixQuery { mode: Some(mode) }),
        )
        .await
        {
            Ok(Json(matrix)) => {
                if json {
                    CliOutput::ok(
                        serde_json::to_string_pretty(&matrix).unwrap_or_else(|_| "{}".to_string()),
                    )
                } else {
                    let ready = matrix.tools.iter().filter(|t| t.status == "ready").count();
                    let installed = matrix.tools.iter().filter(|t| t.installed).count();
                    CliOutput::ok(format!(
                        "cowok-design doctor (mode={}): {} tools scanned, {} installed, {} ready",
                        matrix.scan_mode,
                        matrix.tools.len(),
                        installed,
                        ready
                    ))
                }
            }
            Err(_) => cli_err(
                1,
                "DOCTOR_SCAN_FAILED",
                "Doctor matrix scan failed",
                None,
                json,
            ),
        }
    }
}

fn exec_feedback(
    title: String,
    body: String,
    category: String,
    email: Option<String>,
    include_diagnostics: bool,
    workspace: Option<&Path>,
    json: bool,
) -> CliOutput {
    let (_, pool, _) = match open_cli_storage(workspace) {
        Ok(v) => v,
        Err(msg) => return cli_err(1, "STORAGE_UNAVAILABLE", &msg, None, json),
    };
    if let Err(err) = ensure_support_edge_tables(&pool) {
        return cli_err(1, "STORAGE_INIT_FAILED", &err.to_string(), None, json);
    }

    let skills = EmbeddedBuiltinSkillsRegistry::load();
    let input = CreateSupportTicketInput {
        category,
        title,
        description: body,
        contact_email: email,
        screenshot_data_url: None,
        channel: "cli".to_string(),
        locale: "en-US".to_string(),
        include_diagnostics,
        recent_error_codes: Vec::new(),
        active_skills: skills.skills.iter().map(|s| s.id.clone()).collect(),
    };

    match create_support_ticket(&pool, &input) {
        Ok(ticket) => {
            if json {
                CliOutput::ok(format_json_success(&ticket))
            } else {
                CliOutput::ok(format!(
                    "Submitted feedback ticket {} [{}] (status={}, redactions={})",
                    ticket.ticket_code,
                    ticket.category,
                    ticket.status,
                    ticket.diagnostics.redaction_count
                ))
            }
        }
        Err(err) => cli_err(2, "INVALID_FEEDBACK", &err.to_string(), None, json),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliHtmlExportData {
    export_id: String,
    project_id: String,
    audience_mode: String,
    format: String,
    file_name: String,
    sha256: String,
    byte_size: usize,
    external_cdn_refs_count: usize,
    stripped_external_refs_count: usize,
    presenter_enabled: bool,
    slide_count: usize,
    svg_count: usize,
    broken_svg_count: usize,
    inline_assets_count: usize,
    broken_cas_assets_count: usize,
    csp_header: String,
    qc_gate_status: String,
    qc_override_acknowledged: bool,
    warnings: Vec<String>,
    output_path: String,
    created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliPdfExportData {
    export_id: String,
    project_id: String,
    audience_mode: String,
    format: String,
    file_name: String,
    sha256: String,
    byte_size: usize,
    page_count: usize,
    width_pt: f64,
    height_pt: f64,
    engine: String,
    qc_gate_status: String,
    qc_override_acknowledged: bool,
    warnings: Vec<String>,
    output_path: String,
    created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliPptxExportData {
    export_id: String,
    project_id: String,
    audience_mode: String,
    format: String,
    plugin_installed: bool,
    plugin_id: String,
    plugin_status: String,
    install_action: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    byte_size: usize,
    qc_gate_status: String,
    qc_override_acknowledged: bool,
    created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliPptxExportResponse {
    ok: bool,
    plugin_installed: bool,
    plugin_id: String,
    install_action: String,
    message: String,
    data: CliPptxExportData,
    request_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliDevExportData {
    export_id: String,
    project_id: String,
    audience_mode: String,
    format: String,
    file_name: String,
    file_count: usize,
    sha256: String,
    byte_size: usize,
    files: Vec<html_forge::export_bundler::DeveloperHandoffFile>,
    qc_gate_status: String,
    qc_override_acknowledged: bool,
    output_path: String,
    warnings: Vec<String>,
    created_at: i64,
}

fn inspect_cli_qc_gate(
    conn: &rusqlite::Connection,
    project_id: &str,
    override_qc_warning: bool,
) -> Result<(String, bool), String> {
    let row_opt: Option<(i64, String)> = conn
        .query_row(
            "SELECT p0_count, verdict FROM qc_gate_runs WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![project_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);

    let Some((p0, verdict)) = row_opt else {
        return Ok(("UNSCANNED".to_string(), false));
    };
    if p0 > 0 && !override_qc_warning {
        return Err(format!(
            "QC_GATE_P0_BLOCKED: {p0} unresolved P0 issue(s). Pass --override-qc-warning to override."
        ));
    }
    if p0 > 0 && override_qc_warning {
        Ok(("FAIL_OVERRIDDEN".to_string(), true))
    } else {
        Ok((verdict, false))
    }
}

struct CliExportAuditEntry<'a> {
    export_id: &'a str,
    project_id: &'a str,
    audience_mode: &'a str,
    format: &'a str,
    file_name: &'a str,
    sha256: &'a str,
    byte_size: usize,
    external_cdn_refs_count: usize,
    qc_gate_status: &'a str,
    qc_override_acknowledged: bool,
    plugin_status: &'a str,
    created_at: i64,
}

fn persist_cli_export_audit(conn: &mut rusqlite::Connection, entry: &CliExportAuditEntry<'_>) {
    if ensure_v30_tables(conn).is_err() {
        return;
    }
    let _ = retry_on_busy("cli_insert_export_bundle_run", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO export_bundle_runs (
                id, tenant_id, project_id, audience_mode, format, file_name, artifact_sha256,
                byte_size, external_cdn_refs_count, qc_gate_status,
                qc_override_acknowledged, plugin_status, created_at
             ) VALUES (?1, 'local-default', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                entry.export_id,
                entry.project_id,
                entry.audience_mode,
                entry.format,
                entry.file_name,
                entry.sha256,
                entry.byte_size as i64,
                entry.external_cdn_refs_count as i64,
                entry.qc_gate_status,
                if entry.qc_override_acknowledged {
                    1i64
                } else {
                    0i64
                },
                entry.plugin_status,
                entry.created_at,
            ],
        )?;
        tx.commit()
    });
}

#[allow(clippy::too_many_arguments)]
fn exec_export(
    format: ExportFormat,
    project_id: String,
    out_path: PathBuf,
    title: Option<String>,
    input_html: Option<&Path>,
    override_qc_warning: bool,
    workspace: Option<&Path>,
    json: bool,
) -> CliOutput {
    if let Err(msg) = validate_safe_out_path(&out_path) {
        return cli_err(2, "INVALID_OUT_PATH", &msg, Some("out"), json);
    }

    let (_storage, pool, _) = match open_cli_storage(workspace) {
        Ok(v) => v,
        Err(msg) => return cli_err(1, "STORAGE_UNAVAILABLE", &msg, None, json),
    };
    let mut conn = match pool.connection() {
        Ok(c) => c,
        Err(err) => return cli_err(1, "STORAGE_BUSY", &err.to_string(), None, json),
    };

    let (qc_gate_status, qc_override_acknowledged) =
        match inspect_cli_qc_gate(&conn, &project_id, override_qc_warning) {
            Ok(v) => v,
            Err(msg) => return cli_err(3, "QC_GATE_P0_BLOCKED", &msg, None, json),
        };

    let project_name: String = title.unwrap_or_else(|| {
        conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| format!("Project {project_id}"))
    });

    let html_source = if let Some(html_path) = input_html {
        if reject_if_symlink(html_path).is_err() {
            return cli_err(
                2,
                "SYMLINK_REJECTED",
                "Input HTML path cannot be a symlink",
                Some("inputHtml"),
                json,
            );
        }
        match fs::read_to_string(html_path) {
            Ok(s) => s,
            Err(err) => {
                return cli_err(
                    2,
                    "INPUT_HTML_UNREADABLE",
                    &format!("Failed reading '{}': {err}", html_path.display()),
                    Some("inputHtml"),
                    json,
                );
            }
        }
    } else {
        let escaped_name = escape_html(&project_name);
        let escaped_id = escape_html(&project_id);
        format!(
            "<!DOCTYPE html><html><head><title>{escaped_name}</title></head><body><section data-slide=\"1\"><h1>{escaped_name}</h1><p>Standalone export for project {escaped_id}.</p></section></body></html>"
        )
    };

    let export_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();

    match format {
        ExportFormat::Html => {
            let bundle = match bundle_offline_html(&OfflineHtmlInput {
                title: project_name,
                html: html_source,
                inline_css: None,
                cas_assets: BTreeMap::new(),
                enable_presenter: true,
                max_bytes: None,
            }) {
                Ok(b) => b,
                Err(err) => return cli_err(1, "EXPORT_HTML_FAILED", &err.to_string(), None, json),
            };
            if let Err(msg) = write_atomic_file(&out_path, bundle.html_content.as_bytes()) {
                return cli_err(1, "EXPORT_WRITE_FAILED", &msg, Some("out"), json);
            }
            persist_cli_export_audit(
                &mut conn,
                &CliExportAuditEntry {
                    export_id: &export_id,
                    project_id: &project_id,
                    audience_mode: "presentation",
                    format: "html_bundle",
                    file_name: &bundle.file_name,
                    sha256: &bundle.sha256,
                    byte_size: bundle.byte_size,
                    external_cdn_refs_count: bundle.external_cdn_refs_count,
                    qc_gate_status: &qc_gate_status,
                    qc_override_acknowledged,
                    plugin_status: "not_required",
                    created_at,
                },
            );
            let data = CliHtmlExportData {
                export_id,
                project_id,
                audience_mode: "presentation".to_string(),
                format: "html_bundle".to_string(),
                file_name: bundle.file_name,
                sha256: bundle.sha256,
                byte_size: bundle.byte_size,
                external_cdn_refs_count: bundle.external_cdn_refs_count,
                stripped_external_refs_count: bundle.stripped_external_refs_count,
                presenter_enabled: bundle.presenter_enabled,
                slide_count: bundle.slide_count,
                svg_count: bundle.svg_count,
                broken_svg_count: bundle.broken_svg_count,
                inline_assets_count: bundle.inline_assets_count,
                broken_cas_assets_count: bundle.broken_cas_assets_count,
                csp_header: bundle.csp_header,
                qc_gate_status,
                qc_override_acknowledged,
                warnings: bundle.warnings,
                output_path: out_path.display().to_string(),
                created_at,
            };
            if json {
                CliOutput::ok(format_json_success(&data))
            } else {
                CliOutput::ok(format!(
                    "Exported offline HTML bundle to {} ({} bytes, sha256={})",
                    data.output_path, data.byte_size, data.sha256
                ))
            }
        }
        ExportFormat::Pdf => {
            let bundle = match build_vector_pdf(&VectorPdfInput {
                title: project_name,
                slides: Vec::new(),
                html: Some(html_source),
                max_bytes: None,
            }) {
                Ok(b) => b,
                Err(err) => return cli_err(1, "EXPORT_PDF_FAILED", &err.to_string(), None, json),
            };
            let pdf_bytes = match B64_STD.decode(&bundle.pdf_base64) {
                Ok(b) => b,
                Err(err) => {
                    return cli_err(
                        1,
                        "EXPORT_PDF_DECODE_FAILED",
                        &format!("Invalid PDF base64 output: {err}"),
                        None,
                        json,
                    );
                }
            };
            if let Err(msg) = write_atomic_file(&out_path, &pdf_bytes) {
                return cli_err(1, "EXPORT_WRITE_FAILED", &msg, Some("out"), json);
            }
            persist_cli_export_audit(
                &mut conn,
                &CliExportAuditEntry {
                    export_id: &export_id,
                    project_id: &project_id,
                    audience_mode: "presentation",
                    format: "vector_pdf",
                    file_name: &bundle.file_name,
                    sha256: &bundle.sha256,
                    byte_size: bundle.byte_size,
                    external_cdn_refs_count: 0,
                    qc_gate_status: &qc_gate_status,
                    qc_override_acknowledged,
                    plugin_status: "not_required",
                    created_at,
                },
            );
            let data = CliPdfExportData {
                export_id,
                project_id,
                audience_mode: "presentation".to_string(),
                format: "vector_pdf".to_string(),
                file_name: bundle.file_name,
                sha256: bundle.sha256,
                byte_size: bundle.byte_size,
                page_count: bundle.page_count,
                width_pt: bundle.width_pt,
                height_pt: bundle.height_pt,
                engine: bundle.engine,
                qc_gate_status,
                qc_override_acknowledged,
                warnings: bundle.warnings,
                output_path: out_path.display().to_string(),
                created_at,
            };
            if json {
                CliOutput::ok(format_json_success(&data))
            } else {
                CliOutput::ok(format!(
                    "Exported 16:9 vector PDF to {} ({} pages, {} bytes)",
                    data.output_path, data.page_count, data.byte_size
                ))
            }
        }
        ExportFormat::Pptx => {
            let pptx_bundle = match build_pptx_deck(
                &PptxExportInput {
                    title: project_name,
                    html: html_source,
                    inline_css: None,
                    cas_assets: BTreeMap::new(),
                    export_mode: PptxExportMode::Editable,
                    locale: None,
                    slide_count_hint: None,
                    cdp_snapshot: None,
                    max_bytes: None,
                },
                None,
            ) {
                Ok(b) => b,
                Err(err) => return cli_err(1, "EXPORT_PPTX_FAILED", &err.to_string(), None, json),
            };
            let pptx_bytes = match B64_STD.decode(&pptx_bundle.pptx_base64) {
                Ok(b) => b,
                Err(err) => {
                    return cli_err(
                        1,
                        "EXPORT_PPTX_DECODE_FAILED",
                        &format!("Invalid PPTX base64 output: {err}"),
                        None,
                        json,
                    );
                }
            };
            if let Err(msg) = write_atomic_file(&out_path, &pptx_bytes) {
                return cli_err(1, "EXPORT_WRITE_FAILED", &msg, Some("out"), json);
            }
            persist_cli_export_audit(
                &mut conn,
                &CliExportAuditEntry {
                    export_id: &export_id,
                    project_id: &project_id,
                    audience_mode: "SHARE_PRESENT",
                    format: "PLUGIN_PPTX",
                    file_name: &pptx_bundle.file_name,
                    sha256: &pptx_bundle.sha256,
                    byte_size: pptx_bundle.byte_size,
                    external_cdn_refs_count: 0,
                    qc_gate_status: &qc_gate_status,
                    qc_override_acknowledged,
                    plugin_status: "BUILTIN_ACTIVE",
                    created_at,
                },
            );
            let message = format!(
                "Exported PowerPoint deck ({} slide(s)) to {} ({} bytes) with built-in native Rust exporter.",
                pptx_bundle.slide_count,
                out_path.display(),
                pptx_bundle.byte_size
            );
            let resp = CliPptxExportResponse {
                ok: true,
                plugin_installed: true,
                plugin_id: "builtin-native-rust-pptx-exporter".to_string(),
                install_action: "/plugins?builtin=pptx-exporter".to_string(),
                message: message.clone(),
                data: CliPptxExportData {
                    export_id,
                    project_id,
                    audience_mode: "SHARE_PRESENT".to_string(),
                    format: "PLUGIN_PPTX".to_string(),
                    plugin_installed: true,
                    plugin_id: "builtin-native-rust-pptx-exporter".to_string(),
                    plugin_status: "BUILTIN_ACTIVE".to_string(),
                    install_action: "/plugins?builtin=pptx-exporter".to_string(),
                    message,
                    file_name: Some(pptx_bundle.file_name),
                    sha256: Some(pptx_bundle.sha256),
                    byte_size: pptx_bundle.byte_size,
                    qc_gate_status,
                    qc_override_acknowledged,
                    created_at,
                },
                request_id: Uuid::new_v4().to_string(),
            };
            if json {
                CliOutput::ok(
                    serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
                )
            } else {
                CliOutput::ok(resp.message)
            }
        }
        ExportFormat::Dev => {
            let bundle = match build_developer_handoff(&DeveloperHandoffInput {
                project_name,
                spec_markdown: None,
                design_tokens_css: None,
                design_tokens_json: None,
                screen_tsx: None,
                html_source: Some(html_source),
                e2e_spec_ts: None,
            }) {
                Ok(b) => b,
                Err(err) => return cli_err(1, "EXPORT_DEV_FAILED", &err.to_string(), None, json),
            };
            let writes_json_manifest = out_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("json"));

            if writes_json_manifest {
                let raw = serde_json::to_vec_pretty(&bundle).unwrap_or_default();
                if let Err(msg) = write_atomic_file(&out_path, &raw) {
                    return cli_err(1, "EXPORT_WRITE_FAILED", &msg, Some("out"), json);
                }
            } else {
                if let Err(err) = fs::create_dir_all(&out_path) {
                    return cli_err(
                        1,
                        "EXPORT_WRITE_FAILED",
                        &format!(
                            "Failed creating handoff dir '{}': {err}",
                            out_path.display()
                        ),
                        Some("out"),
                        json,
                    );
                }
                for file in &bundle.files {
                    let target = out_path.join(&file.path);
                    if let Err(msg) = write_atomic_file(&target, file.content.as_bytes()) {
                        return cli_err(1, "EXPORT_WRITE_FAILED", &msg, Some("out"), json);
                    }
                }
            }
            persist_cli_export_audit(
                &mut conn,
                &CliExportAuditEntry {
                    export_id: &export_id,
                    project_id: &project_id,
                    audience_mode: "developer",
                    format: "developer_handoff",
                    file_name: &bundle.file_name,
                    sha256: &bundle.sha256,
                    byte_size: bundle.byte_size,
                    external_cdn_refs_count: 0,
                    qc_gate_status: &qc_gate_status,
                    qc_override_acknowledged,
                    plugin_status: "not_required",
                    created_at,
                },
            );
            let file_count = bundle.files.len();
            let data = CliDevExportData {
                export_id,
                project_id,
                audience_mode: "developer".to_string(),
                format: "developer_handoff".to_string(),
                file_name: bundle.file_name,
                file_count,
                sha256: bundle.sha256,
                byte_size: bundle.byte_size,
                files: bundle.files,
                qc_gate_status,
                qc_override_acknowledged,
                output_path: out_path.display().to_string(),
                warnings: Vec::new(),
                created_at,
            };
            if json {
                CliOutput::ok(format_json_success(&data))
            } else {
                CliOutput::ok(format!(
                    "Exported developer handoff ({} files, {} bytes) to {}",
                    data.file_count, data.byte_size, data.output_path
                ))
            }
        }
    }
}

fn load_or_seed_mcp_policies(conn: &mut rusqlite::Connection) -> Vec<ToolPolicyRecord> {
    if let Ok(tx) = conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        for p in default_tool_policies() {
            let enabled = if p.enabled { 1i64 } else { 0i64 };
            let callable = if p.permission_tier == PermissionTier::HumanOnly {
                0i64
            } else if p.mcp_callable {
                1i64
            } else {
                0i64
            };
            let _ = tx.execute(
                "INSERT OR IGNORE INTO mcp_tool_policies (tool_name, permission_tier, enabled, mcp_callable, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    p.tool_name,
                    p.permission_tier.as_str(),
                    enabled,
                    callable,
                    p.updated_at
                ],
            );
        }
        let _ = tx.commit();
    }
    let mut out = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT tool_name, permission_tier, enabled, mcp_callable, updated_at FROM mcp_tool_policies",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            let tool_name: String = r.get(0)?;
            let tier_raw: String = r.get(1)?;
            let enabled: i64 = r.get(2)?;
            let mcp_callable: i64 = r.get(3)?;
            let updated_at: i64 = r.get(4)?;
            let permission_tier =
                PermissionTier::parse(&tier_raw).unwrap_or(PermissionTier::HumanOnly);
            Ok(ToolPolicyRecord {
                tool_name,
                permission_tier,
                enabled: enabled != 0,
                mcp_callable: permission_tier != PermissionTier::HumanOnly && mcp_callable != 0,
                description: String::new(),
                updated_at,
            })
        }) {
            for row in rows.flatten() {
                out.push(row);
            }
        }
    }
    if out.is_empty() {
        default_tool_policies()
    } else {
        out
    }
}

fn exec_mcp(action: McpSubcommand, workspace: Option<&Path>, json: bool) -> CliOutput {
    let (storage, pool, _) = match open_cli_storage(workspace) {
        Ok(v) => v,
        Err(msg) => return cli_err(1, "STORAGE_UNAVAILABLE", &msg, None, json),
    };
    let mut conn = match pool.connection() {
        Ok(c) => c,
        Err(err) => return cli_err(1, "STORAGE_BUSY", &err.to_string(), None, json),
    };

    match action {
        McpSubcommand::Serve => {
            let engine = McpGatewayEngine::new("cowok-design-cli".to_string());
            let policies = load_or_seed_mcp_policies(&mut conn);
            engine.sync_tool_policies(&policies);

            let stdin = io::stdin();
            let mut stdout = io::stdout();
            for line_res in stdin.lock().lines() {
                let Ok(line) = line_res else { break };
                if line.trim().is_empty() {
                    continue;
                }
                match parse_stdio_frame(&line) {
                    Ok(rpc_req) => {
                        let outcome = engine.dispatch(rpc_req, ActorKind::McpClient);
                        let frame = format_stdio_frame(&outcome.response, false);
                        let _ = stdout.write_all(frame.as_bytes());
                        let _ = stdout.flush();
                    }
                    Err(err_resp) => {
                        let frame = format_stdio_frame(&err_resp, false);
                        let _ = stdout.write_all(frame.as_bytes());
                        let _ = stdout.flush();
                    }
                }
            }
            CliOutput::ok(String::new())
        }
        McpSubcommand::Install {
            host,
            rollback_backup_id,
            config_path,
        } => {
            let host_id = match HostClientId::parse(&host) {
                Ok(h) => h,
                Err(err) => {
                    return cli_err(2, "UNSUPPORTED_HOST", &err.to_string(), Some("host"), json)
                }
            };

            if let Some(backup_id) = rollback_backup_id {
                let row: Option<(String, String, String, String)> = conn
                    .query_row(
                        "SELECT config_path, backup_path, before_sha256, after_sha256
                         FROM mcp_host_install_backups
                         WHERE id = ?1 AND host_id = ?2 AND rolled_back = 0",
                        params![backup_id, host_id.as_str()],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()
                    .unwrap_or(None);
                let Some((cfg_path, bkp_path, before_sha, after_sha)) = row else {
                    return cli_err(
                        1,
                        "BACKUP_NOT_FOUND",
                        &format!(
                            "No active backup '{backup_id}' found for host '{}'",
                            host_id.as_str()
                        ),
                        Some("rollback"),
                        json,
                    );
                };
                match rollback_host_install(
                    host_id,
                    Path::new(&cfg_path),
                    Path::new(&bkp_path),
                    &before_sha,
                ) {
                    Ok(rb) => {
                        let _ = conn.execute(
                            "UPDATE mcp_host_install_backups SET rolled_back = 1 WHERE id = ?1",
                            params![backup_id],
                        );
                        let data = McpHostInstallData {
                            action: "rollback".to_string(),
                            host_id: host_id.as_str().to_string(),
                            display_name: host_id.display_name().to_string(),
                            config_path: rb.config_path,
                            backup_id,
                            backup_path: rb.backup_path,
                            before_sha256: before_sha,
                            after_sha256: after_sha,
                            rolled_back: true,
                            timestamp: rb.rolled_back_at,
                        };
                        return if json {
                            CliOutput::ok(format_json_success(&data))
                        } else {
                            CliOutput::ok(format!(
                                "Rolled back {} MCP config at {}",
                                data.display_name, data.config_path
                            ))
                        };
                    }
                    Err(err) => {
                        return cli_err(1, "MCP_ROLLBACK_FAILED", &err.to_string(), None, json)
                    }
                }
            }

            let target_path = if let Some(explicit) = config_path {
                if let Err(err) = reject_symlinks_in_path(&explicit) {
                    return cli_err(
                        2,
                        "SYMLINK_REJECTED",
                        &err.to_string(),
                        Some("configPath"),
                        json,
                    );
                }
                explicit
            } else {
                let home = std::env::var_os("COWOK_MCP_HOST_HOME")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| storage.data_dir.join("mcp-hosts"));
                let p = home.join(host_id.default_relative_path());
                if let Err(err) = reject_symlinks_in_path(&p) {
                    return cli_err(2, "SYMLINK_REJECTED", &err.to_string(), None, json);
                }
                p
            };

            let backup_dir = storage.data_dir.join("mcp-host-backups");
            let server_url = format!("http://127.0.0.1:{DEFAULT_DAEMON_PORT}/api/v1/mcp/rpc");
            let receipt = match install_host_config(&HostInstallOptions {
                host: host_id,
                config_path: target_path,
                backup_dir,
                server_command: CLI_BIN_NAME.to_string(),
                server_args: vec!["mcp".to_string(), "serve".to_string()],
                server_url,
            }) {
                Ok(r) => r,
                Err(err) => return cli_err(1, "MCP_INSTALL_FAILED", &err.to_string(), None, json),
            };

            let _ = retry_on_busy("cli_insert_mcp_backup", || {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT INTO mcp_host_install_backups
                        (id, host_id, config_path, backup_path, before_sha256, after_sha256, rolled_back, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                    params![
                        receipt.backup_id,
                        receipt.host_id,
                        receipt.config_path,
                        receipt.backup_path,
                        receipt.before_sha256,
                        receipt.after_sha256,
                        receipt.installed_at,
                    ],
                )?;
                tx.commit()
            });

            let data = McpHostInstallData {
                action: "install".to_string(),
                host_id: receipt.host_id,
                display_name: receipt.display_name,
                config_path: receipt.config_path,
                backup_id: receipt.backup_id,
                backup_path: receipt.backup_path,
                before_sha256: receipt.before_sha256,
                after_sha256: receipt.after_sha256,
                rolled_back: false,
                timestamp: receipt.installed_at,
            };
            if json {
                CliOutput::ok(format_json_success(&data))
            } else {
                CliOutput::ok(format!(
                    "Installed cowok-design MCP into {} ({})",
                    data.display_name, data.config_path
                ))
            }
        }
    }
}

async fn exec_qc(
    project_id: String,
    target_id: String,
    html_file: Option<PathBuf>,
    force_no_browser: bool,
    workspace: Option<PathBuf>,
    json: bool,
) -> CliOutput {
    let (_, _, registry) = match open_cli_storage(workspace.as_deref()) {
        Ok(v) => v,
        Err(msg) => return cli_err(1, "STORAGE_UNAVAILABLE", &msg, None, json),
    };

    let html = if let Some(path) = html_file {
        if reject_if_symlink(&path).is_err() {
            return cli_err(
                2,
                "SYMLINK_REJECTED",
                "HTML file cannot be a symlink",
                Some("htmlFile"),
                json,
            );
        }
        match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                return cli_err(
                    2,
                    "HTML_FILE_UNREADABLE",
                    &format!("Failed reading '{}': {err}", path.display()),
                    Some("htmlFile"),
                    json,
                );
            }
        }
    } else {
        "<!DOCTYPE html><html><body><main><h1>QC Target</h1></main></body></html>".to_string()
    };

    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), DEFAULT_DAEMON_PORT);
    let req = QcRunRequest {
        project_id,
        target_kind: QcTargetKind::AppScreen,
        target_id,
        html,
        constraints_json: "{}".to_string(),
        viewports: CANONICAL_APP_VIEWPORTS.to_vec(),
        fixture_ref: "cli".to_string(),
        geometry_nodes: Vec::new(),
        flow_nodes: Vec::new(),
        flow_edges: Vec::new(),
        reference_rgba: None,
        candidate_rgba: None,
        frame_width: 0,
        frame_height: 0,
        ssim_threshold: DEFAULT_SSIM_PASS_THRESHOLD,
        force_refresh: true,
        force_no_browser,
    };

    match evaluate_qc_for_project(&state, req).await {
        Ok(data) => {
            let exit_code = if data.p0_count > 0 { 3 } else { 0 };
            if json {
                CliOutput {
                    exit_code,
                    stdout: format_json_success(&data),
                    stderr: String::new(),
                }
            } else {
                CliOutput {
                    exit_code,
                    stdout: format!(
                        "QC verdict={:?} (p0={}, p1={}, rules={})",
                        data.verdict, data.p0_count, data.p1_count, data.rules_evaluated
                    ),
                    stderr: String::new(),
                }
            }
        }
        Err(msg) => cli_err(1, "QC_RUN_FAILED", &msg, None, json),
    }
}

fn exec_de(
    tool: Option<&str>,
    html_file: Option<&Path>,
    workspace: Option<&Path>,
    json: bool,
) -> CliOutput {
    let tools: Vec<BuiltInDeTool> = match tool {
        Some(name) if !name.trim().eq_ignore_ascii_case("all") => {
            let Some(parsed) = BuiltInDeTool::parse_name(name) else {
                return cli_err(
                    2,
                    "UNKNOWN_DE_TOOL",
                    &format!(
                        "Unknown DE tool '{name}'. Supported: scan-ui, intake, tokens, nfa-packs, archify, get_artifact, notes-check"
                    ),
                    Some("tool"),
                    json,
                );
            };
            vec![parsed]
        }
        _ => BuiltInDeTool::all().to_vec(),
    };

    let ws_root = workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let (target_rel, content_override) = if let Some(path) = html_file {
        if reject_if_symlink(path).is_err() {
            return cli_err(
                2,
                "SYMLINK_REJECTED",
                "HTML file cannot be a symlink",
                Some("htmlFile"),
                json,
            );
        }
        match fs::read_to_string(path) {
            Ok(s) => (
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("target.html")
                    .to_string(),
                Some(s),
            ),
            Err(err) => {
                return cli_err(
                    2,
                    "HTML_FILE_UNREADABLE",
                    &format!("Failed reading '{}': {err}", path.display()),
                    Some("htmlFile"),
                    json,
                );
            }
        }
    } else {
        (
            "inline-cli-artifact.html".to_string(),
            Some(
                "<!DOCTYPE html><html><body><main><h1>Design Engineering Audit</h1><p>Clean semantic baseline.</p></main></body></html>".to_string(),
            ),
        )
    };

    let input = DeSuiteInput {
        workspace_root: &ws_root,
        target_artifact_path: &target_rel,
        content_override: content_override.as_deref(),
        tools: &tools,
        token_pairs: &[],
    };

    match run_builtin_de_suite(&input) {
        Ok(report) => {
            if json {
                CliOutput::ok(format_json_success(&report))
            } else {
                CliOutput::ok(format!(
                    "DE suite passed={} ({} tools, {} ms, withinSla={})",
                    report.passed,
                    report.results.len(),
                    report.total_duration_ms,
                    report.within_sla
                ))
            }
        }
        Err(err) => cli_err(1, "DE_RUN_FAILED", &err.to_string(), None, json),
    }
}

fn exec_diff(req: VisualDiffRequest, json: bool) -> CliOutput {
    let reaper = ChildProcessReaper::new();
    match run_visual_diff(&req, Some(&reaper)) {
        Ok(report) => {
            let exit_code = if report.verdict == "FAIL" { 3 } else { 0 };
            if json {
                CliOutput {
                    exit_code,
                    stdout: format_json_success(&report),
                    stderr: String::new(),
                }
            } else {
                CliOutput {
                    exit_code,
                    stdout: format!(
                        "Visual diff verdict={} (cases={}, passed={}, failed={}, unverified={}) -> {}",
                        report.verdict,
                        report.total_cases,
                        report.passed_cases,
                        report.failed_cases,
                        report.unverified_cases,
                        report.report_path
                    ),
                    stderr: String::new(),
                }
            }
        }
        Err(msg) => cli_err(1, "VISUAL_DIFF_FAILED", &msg, None, json),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpStartupData {
    status: String,
    service: String,
    host: String,
    port: u16,
    url: String,
    api_alias_enabled: bool,
    pid: u32,
    schema_version: u32,
    workspace_root: String,
    data_dir: String,
    read_only_fallback: bool,
    embedded_skills_count: usize,
    de_tools_count: usize,
    lock_path: String,
}

async fn exec_up(
    port: Option<u16>,
    workspace: Option<PathBuf>,
    no_open: bool,
    json: bool,
) -> CliOutput {
    let preferred_port = port.or(Some(DEFAULT_DAEMON_PORT));
    let (storage, pool, registry) = match open_cli_storage(workspace.as_deref()) {
        Ok(v) => v,
        Err(msg) => return cli_err(1, "STORAGE_UNAVAILABLE", &msg, None, json),
    };

    let spa = StaticSpaHandler::discover(None, Some(&storage.workspace_root));
    let server = match DaemonServer::bind_standalone_loopback(
        preferred_port,
        &storage,
        Arc::clone(&registry),
        spa.clone(),
    ) {
        Ok(s) => s,
        Err(err) => return cli_err(1, "DAEMON_BIND_FAILED", &err.to_string(), None, json),
    };

    let reaper = ChildProcessReaper::new();
    reaper.set_active_workers(server.active_workers());
    reaper.set_owned_lockfile(
        server.lock_path().clone(),
        server.lock_info().pid,
        server.lock_info().daemon_nonce.clone(),
    );

    let url = format!(
        "http://{}:{}",
        server.lock_info().host,
        server.lock_info().port
    );
    let startup = UpStartupData {
        status: "ok".to_string(),
        service: "cowok-design-harness-daemon".to_string(),
        host: server.lock_info().host.clone(),
        port: server.lock_info().port,
        url: url.clone(),
        api_alias_enabled: true,
        pid: server.lock_info().pid,
        schema_version: server.lock_info().schema_version,
        workspace_root: storage.workspace_root.display().to_string(),
        data_dir: storage.data_dir.display().to_string(),
        read_only_fallback: storage.read_only_fallback,
        embedded_skills_count: spa.embedded_skills_count(),
        de_tools_count: spa.embedded_de_tools_count(),
        lock_path: server.lock_path().display().to_string(),
    };

    if json {
        println!("{}", format_json_success(&startup));
    } else {
        println!(
            "cowok-design standalone workbench listening on {url} (pid={}, schema=V{}, /api/* -> /api/v1/* active)",
            startup.pid, startup.schema_version
        );
    }

    if !no_open && std::env::var_os("CI").is_none() {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }

    drop(pool);
    match server.serve_with_reaper(reaper).await {
        Ok(()) => CliOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(err) => cli_err(1, "DAEMON_SERVE_FAILED", &err.to_string(), None, json),
    }
}
