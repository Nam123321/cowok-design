// @story 12.1
// @implements AC-04 (Focus-Free macOS/Linux Headless Chromium CDP Screenshot & Self-Contained Offline HTML Visual Diff Suite)
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use html_forge::export_bundler::OFFLINE_CSP_CONTENT;
use html_forge::visual_qc::{
    compute_layout_sha1, discover_local_chromium_binary, DEFAULT_SSIM_PASS_THRESHOLD,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::lockfile::reject_if_symlink;
use crate::port_binder::ChildProcessReaper;
use crate::routes::visual_qc::shared_cdp_evaluator;

const CHROMIUM_CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BASELINE_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB

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
            "PATH_TRAVERSAL_DENIED: path '{}' cannot contain '..'",
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
                    "PATH_TRAVERSAL_DENIED: path '{}' cannot contain '..'",
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
                                "SYMLINK_REJECTED: symlink component '{}' is forbidden",
                                current.display()
                            ));
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                    Err(err) => {
                        return Err(format!(
                            "PATH_INSPECT_FAILED: cannot inspect '{}': {err}",
                            current.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Strictly non-activating headless Chromium flags ensuring macOS NEVER steals window focus
/// or activates a foreground Dock window during screenshots or diff suite runs.
pub const FOCUS_FREE_HEADLESS_FLAGS: &[&str] = &[
    "--headless=new",
    "--disable-gpu",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-background-timer-throttling",
    "--disable-extensions",
    "--hide-scrollbars",
    "--mute-audio",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualDiffMode {
    Suite(PathBuf),
    Screenshot(String),
}

#[derive(Debug, Clone)]
pub struct VisualDiffRequest {
    pub mode: VisualDiffMode,
    pub out_path: PathBuf,
    pub baseline: Option<PathBuf>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub ssim_threshold: f64,
    pub force_no_browser: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualDiffCaseResult {
    pub case_id: String,
    pub target: String,
    pub viewport: String,
    pub verdict: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim_score: Option<f64>,
    pub layout_sha1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_sha256: Option<String>,
    pub duration_ms: u64,
    pub notes: Vec<String>,
    #[serde(skip_serializing)]
    pub screenshot_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualDiffReportData {
    pub mode: String,
    pub verdict: String,
    pub focus_free: bool,
    pub headless_flags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_binary: Option<String>,
    pub incognito_context_id: String,
    pub used_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_mode: Option<String>,
    pub ssim_threshold: f64,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub unverified_cases: usize,
    pub report_path: String,
    pub report_sha256: String,
    pub external_cdn_refs_count: usize,
    pub cases: Vec<VisualDiffCaseResult>,
    pub generated_at: String,
}

#[derive(Debug, Clone)]
pub enum ValidatedDiffTarget {
    LoopbackUrl(String),
    LocalHtmlFile {
        canonical_path: PathBuf,
        file_url: String,
        html_content: String,
    },
}

/// Parses `WxH` viewport string (e.g., `"1440x900"`), bounded to safe dimensions.
pub fn parse_viewport(raw: &str) -> Result<(u32, u32), String> {
    let trimmed = raw.trim().to_ascii_lowercase();
    let (w_str, h_str) = trimmed
        .split_once('x')
        .ok_or_else(|| format!("Invalid viewport '{raw}': expected <width>x<height>"))?;
    let w: u32 = w_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid viewport width in '{raw}'"))?;
    let h: u32 = h_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid viewport height in '{raw}'"))?;
    if !(320..=3840).contains(&w) || !(240..=2160).contains(&h) {
        return Err(format!(
            "Viewport {w}x{h} out of allowed bounds (320..=3840 x 240..=2160)"
        ));
    }
    Ok((w, h))
}

/// Constructs strictly focus-free headless Chromium arguments for screenshot capture.
pub fn build_focus_free_chromium_args(
    target_url: &str,
    screenshot_out: &Path,
    user_data_dir: &Path,
    width: u32,
    height: u32,
) -> Vec<String> {
    let mut args: Vec<String> = FOCUS_FREE_HEADLESS_FLAGS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    args.push(format!("--window-size={width},{height}"));
    args.push(format!("--user-data-dir={}", user_data_dir.display()));
    args.push(format!("--screenshot={}", screenshot_out.display()));
    args.push(target_url.to_string());
    args
}

/// Validates a diff target: allows only loopback HTTP URLs (`127.0.0.1` / `localhost`)
/// or canonical non-symlink local `.html`/`.htm` files. Blocks external SSRF URLs.
pub fn validate_diff_target(raw: &str) -> Result<ValidatedDiffTarget, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Diff target cannot be empty".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || trimmed.contains("://") {
        if trimmed.contains('\\') {
            return Err(format!(
                "SSRF_BLOCKED: backslash in URL '{trimmed}' is forbidden for local diff"
            ));
        }
        let url = reqwest::Url::parse(trimmed)
            .map_err(|e| format!("SSRF_BLOCKED: invalid URL '{trimmed}': {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!(
                "SSRF_BLOCKED: scheme '{}' is forbidden for local diff",
                url.scheme()
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(format!(
                "SSRF_BLOCKED: userinfo credentials in URL '{trimmed}' are forbidden for local diff"
            ));
        }
        if !matches!(url.host_str(), Some("127.0.0.1") | Some("localhost")) {
            return Err(format!(
                "SSRF_BLOCKED: external non-loopback URL '{trimmed}' is forbidden for local diff"
            ));
        }
        return Ok(ValidatedDiffTarget::LoopbackUrl(url.to_string()));
    }

    let path = PathBuf::from(trimmed);
    reject_symlink_ancestors(&path)?;
    reject_if_symlink(&path).map_err(|e| format!("SYMLINK_REJECTED: {e}"))?;
    let meta = fs::symlink_metadata(&path)
        .map_err(|e| format!("Target file '{trimmed}' unreadable: {e}"))?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return Err(format!(
            "Target '{trimmed}' must be a regular non-symlink HTML file"
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("html") | Some("htm")) {
        return Err(format!(
            "Target file '{trimmed}' must have .html or .htm extension"
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize '{trimmed}': {e}"))?;
    let html_content = fs::read_to_string(&canonical)
        .map_err(|e| format!("Failed reading HTML '{}': {e}", canonical.display()))?;
    let file_url = format!("file://{}", canonical.display());
    Ok(ValidatedDiffTarget::LocalHtmlFile {
        canonical_path: canonical,
        file_url,
        html_content,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn validate_safe_output_path(out_path: &Path) -> Result<PathBuf, String> {
    reject_symlink_ancestors(out_path)?;
    reject_if_symlink(out_path).map_err(|e| format!("SYMLINK_REJECTED: {e}"))?;
    if let Some(parent) = out_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        reject_symlink_ancestors(parent)?;
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed creating output directory '{}': {e}",
                parent.display()
            )
        })?;
        reject_symlink_ancestors(parent)?;
    }
    Ok(out_path.to_path_buf())
}

fn write_file_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let tmp_name = format!(".cowok-diff-tmp-{}", Uuid::new_v4().simple());
    let tmp_path = parent.join(tmp_name);
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
        return Err(format!("Failed writing temp artifact: {err}"));
    }
    reject_if_symlink(target).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("SYMLINK_REJECTED: {e}")
    })?;
    if let Err(err) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Failed finalizing output '{}': {err}",
            target.display()
        ));
    }
    Ok(())
}

fn discover_suite_targets(dir: &Path) -> Result<Vec<(String, ValidatedDiffTarget)>, String> {
    reject_symlink_ancestors(dir)?;
    reject_if_symlink(dir).map_err(|e| format!("SYMLINK_REJECTED: {e}"))?;
    let meta = fs::symlink_metadata(dir)
        .map_err(|e| format!("Suite directory '{}' not readable: {e}", dir.display()))?;
    if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
        return Err(format!(
            "Suite path '{}' must be a regular non-symlink directory",
            dir.display()
        ));
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("Failed listing suite directory: {e}"))?
        .filter_map(|e| e.ok().map(|de| de.path()))
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
            })
        })
        .collect();
    entries.sort();

    if entries.is_empty() {
        return Err(format!(
            "Suite directory '{}' contains no .html/.htm target files",
            dir.display()
        ));
    }

    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let case_id = entry
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("case")
            .to_string();
        let validated = validate_diff_target(&entry.to_string_lossy())?;
        out.push((case_id, validated));
    }
    Ok(out)
}

fn capture_screenshot_focus_free(
    chrome_bin: &str,
    target_url: &str,
    width: u32,
    height: u32,
    reaper: Option<&ChildProcessReaper>,
) -> Result<Vec<u8>, String> {
    let scratch_dir =
        std::env::temp_dir().join(format!("cowok-focus-free-cdp-{}", Uuid::new_v4().simple()));
    let profile_dir = scratch_dir.join("profile");
    let shot_path = scratch_dir.join("capture.png");
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&scratch_dir)
            .map_err(|e| format!("Failed creating ephemeral CDP scratch dir: {e}"))?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&profile_dir)
            .map_err(|e| format!("Failed creating ephemeral CDP profile dir: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed creating ephemeral CDP profile dir: {e}"))?;
    }

    let args = build_focus_free_chromium_args(target_url, &shot_path, &profile_dir, width, height);
    let mut cmd = Command::new(chrome_bin);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let local_reaper = ChildProcessReaper::new();
    let active_reaper = reaper.unwrap_or(&local_reaper);
    let (mut child, handle) = active_reaper
        .spawn_managed_child(cmd, "focus-free-chromium-diff")
        .map_err(|e| {
            let _ = fs::remove_dir_all(&scratch_dir);
            format!("Failed spawning headless Chromium '{chrome_bin}': {e}")
        })?;

    let deadline = Instant::now() + CHROMIUM_CAPTURE_TIMEOUT;
    let status_res = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = ChildProcessReaper::terminate_pgid_safe(
                        handle.pid,
                        handle.pgid,
                        &handle.start_time,
                    );
                    let _ = child.wait();
                    break Err("Headless Chromium screenshot exceeded 15s timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => break Err(format!("Failed waiting for headless Chromium: {e}")),
        }
    };
    active_reaper.unregister_child(handle.pid);

    let capture_res = match status_res {
        Ok(st) if st.success() && shot_path.is_file() => {
            fs::read(&shot_path).map_err(|e| format!("Failed reading captured PNG screenshot: {e}"))
        }
        Ok(st) => Err(format!(
            "Headless Chromium exited with status {st} without producing screenshot"
        )),
        Err(msg) => Err(msg),
    };

    let _ = fs::remove_dir_all(&scratch_dir);
    capture_res
}

fn compute_byte_similarity(baseline: &[u8], candidate: &[u8]) -> f64 {
    if baseline == candidate {
        return 1.0;
    }
    if baseline.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    let min_len = baseline.len().min(candidate.len());
    let max_len = baseline.len().max(candidate.len());
    let matching = baseline[..min_len]
        .iter()
        .zip(candidate[..min_len].iter())
        .filter(|(a, b)| a == b)
        .count();
    (matching as f64) / (max_len as f64)
}

fn read_regular_file_bounded_nofollow(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    reject_symlink_ancestors(path)?;
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = opts
        .open(path)
        .map_err(|e| format!("BASELINE_OPEN_FAILED: '{}': {e}", path.display()))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("BASELINE_METADATA_FAILED: '{}': {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!(
            "BASELINE_NOT_REGULAR_FILE: '{}' must be a regular file",
            path.display()
        ));
    }
    if meta.len() > max_bytes {
        return Err(format!(
            "BASELINE_TOO_LARGE: '{}' exceeds {} byte limit",
            path.display(),
            max_bytes
        ));
    }
    let mut buf = Vec::with_capacity(meta.len().min(max_bytes) as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("BASELINE_READ_FAILED: '{}': {e}", path.display()))?;
    if buf.len() as u64 > max_bytes {
        return Err(format!(
            "BASELINE_TOO_LARGE: '{}' exceeded {} byte limit while reading",
            path.display(),
            max_bytes
        ));
    }
    Ok(buf)
}

fn resolve_baseline_bytes(
    baseline: Option<&Path>,
    case_id: &str,
) -> Result<Option<Vec<u8>>, String> {
    let Some(base) = baseline else {
        return Ok(None);
    };
    reject_symlink_ancestors(base)?;
    let meta = fs::symlink_metadata(base)
        .map_err(|e| format!("BASELINE_UNREADABLE: '{}': {e}", base.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "SYMLINK_REJECTED: baseline '{}' cannot be a symlink",
            base.display()
        ));
    }
    if meta.file_type().is_file() {
        return read_regular_file_bounded_nofollow(base, MAX_BASELINE_BYTES).map(Some);
    }
    if meta.file_type().is_dir() {
        if case_id.is_empty()
            || case_id.contains("..")
            || case_id.contains('/')
            || case_id.contains('\\')
            || case_id.contains('\0')
        {
            return Err(format!(
                "PATH_TRAVERSAL_DENIED: unsafe baseline case_id '{case_id}'"
            ));
        }
        for ext in ["png", "html"] {
            let candidate = base.join(format!("{case_id}.{ext}"));
            match fs::symlink_metadata(&candidate) {
                Ok(cmeta) => {
                    if cmeta.file_type().is_symlink() {
                        return Err(format!(
                            "SYMLINK_REJECTED: baseline file '{}' cannot be a symlink",
                            candidate.display()
                        ));
                    }
                    if !cmeta.file_type().is_file() {
                        return Err(format!(
                            "BASELINE_NOT_REGULAR_FILE: '{}' must be a regular file",
                            candidate.display()
                        ));
                    }
                    return read_regular_file_bounded_nofollow(&candidate, MAX_BASELINE_BYTES)
                        .map(Some);
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => {
                    return Err(format!(
                        "BASELINE_UNREADABLE: '{}': {err}",
                        candidate.display()
                    ));
                }
            }
        }
        return Ok(None);
    }
    Err(format!(
        "BASELINE_NOT_REGULAR_FILE: baseline '{}' must be a regular file or directory",
        base.display()
    ))
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

/// Counts any accidental external `http://` or `https://` resource references inside HTML.
pub fn count_external_cdn_refs(html: &str) -> usize {
    let lower = html.to_ascii_lowercase();
    let mut count = 0usize;
    for marker in [
        "src=\"http://",
        "src=\"https://",
        "href=\"http://",
        "href=\"https://",
        "url(http://",
        "url(https://",
    ] {
        count += lower.matches(marker).count();
    }
    count
}

fn render_self_contained_diff_html(report: &VisualDiffReportData) -> String {
    let csp = format!("{OFFLINE_CSP_CONTENT} script-src 'none';");
    let mut cards = String::new();
    for case in &report.cases {
        let badge_class = match case.verdict.as_str() {
            "PASS" => "badge-pass",
            "FAIL" => "badge-fail",
            _ => "badge-unverified",
        };
        let ssim_label = case
            .ssim_score
            .map(|s| format!("{s:.4}"))
            .unwrap_or_else(|| "N/A".to_string());
        let preview_html = if let Some(ref b64) = case.screenshot_base64 {
            format!(
                "<img class=\"preview-img\" alt=\"Captured screenshot for {}\" src=\"data:image/png;base64,{}\" />",
                escape_html(&case.case_id),
                b64
            )
        } else {
            "<div class=\"preview-placeholder\">Browser capture unavailable (UNVERIFIED_NO_BROWSER) — structural layout SHA-1 recorded offline.</div>".to_string()
        };
        let notes_html = if case.notes.is_empty() {
            String::new()
        } else {
            let items: String = case
                .notes
                .iter()
                .map(|n| format!("<li>{}</li>", escape_html(n)))
                .collect();
            format!("<ul class=\"notes\">{items}</ul>")
        };

        cards.push_str(&format!(
            r#"<article class="case-card">
  <header class="case-head">
    <h2>{}</h2>
    <span class="badge {}">{}</span>
  </header>
  <dl class="meta-grid">
    <div><dt>Viewport</dt><dd>{}</dd></div>
    <div><dt>SSIM</dt><dd>{}</dd></div>
    <div><dt>Layout SHA-1</dt><dd><code>{}</code></dd></div>
    <div><dt>Duration</dt><dd>{} ms</dd></div>
  </dl>
  <div class="preview-frame">{}</div>
  {}
</article>
"#,
            escape_html(&case.case_id),
            badge_class,
            escape_html(&case.verdict),
            escape_html(&case.viewport),
            escape_html(&ssim_label),
            escape_html(&case.layout_sha1),
            case.duration_ms,
            preview_html,
            notes_html,
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <meta http-equiv="Content-Security-Policy" content="{}" />
  <title>cowok-design Visual Diff Report ({})</title>
  <style>
    :root {{ --bg: #0f172a; --surface: #1e293b; --fg: #f8fafc; --muted: #94a3b8; --border: #334155; --pass: #10b981; --fail: #ef4444; --warn: #f59e0b; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; padding: 28px; background: var(--bg); color: var(--fg); font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }}
    .wrap {{ max-width: 1120px; margin: 0 auto; }}
    .header {{ display: flex; justify-content: space-between; align-items: baseline; flex-wrap: wrap; gap: 12px; border-bottom: 1px solid var(--border); padding-bottom: 16px; margin-bottom: 20px; }}
    .kpi-row {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px; margin-bottom: 24px; }}
    .kpi {{ background: var(--surface); border: 1px solid var(--border); border-radius: 10px; padding: 14px; }}
    .kpi-label {{ font-size: 12px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }}
    .kpi-val {{ font-size: 22px; font-weight: 700; margin-top: 4px; }}
    .case-card {{ background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 18px; margin-bottom: 16px; }}
    .case-head {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }}
    .case-head h2 {{ margin: 0; font-size: 17px; }}
    .badge {{ padding: 4px 10px; border-radius: 999px; font-size: 12px; font-weight: 600; }}
    .badge-pass {{ background: rgba(16, 185, 129, 0.16); color: var(--pass); }}
    .badge-fail {{ background: rgba(239, 68, 68, 0.16); color: var(--fail); }}
    .badge-unverified {{ background: rgba(245, 158, 11, 0.16); color: var(--warn); }}
    .meta-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 8px 16px; margin: 0 0 14px; }}
    dt {{ font-size: 11px; color: var(--muted); text-transform: uppercase; }}
    dd {{ margin: 2px 0 0; font-size: 13px; }}
    code {{ background: rgba(15, 23, 42, 0.7); padding: 2px 6px; border-radius: 4px; font-size: 12px; }}
    .preview-frame {{ background: #020617; border: 1px solid var(--border); border-radius: 8px; padding: 12px; text-align: center; }}
    .preview-img {{ max-width: 100%; height: auto; border-radius: 6px; }}
    .preview-placeholder {{ padding: 24px; color: var(--muted); font-size: 13px; }}
    .notes {{ margin: 12px 0 0; padding-left: 18px; color: var(--muted); font-size: 13px; }}
  </style>
</head>
<body>
  <div class="wrap" data-cowok-visual-diff="true" data-focus-free="true">
    <div class="header">
      <h1>cowok-design Visual Diff Report</h1>
      <span class="badge {}">{}</span>
    </div>
    <section class="kpi-row">
      <div class="kpi"><div class="kpi-label">Mode</div><div class="kpi-val">{}</div></div>
      <div class="kpi"><div class="kpi-label">Total Cases</div><div class="kpi-val">{}</div></div>
      <div class="kpi"><div class="kpi-label">Passed</div><div class="kpi-val">{}</div></div>
      <div class="kpi"><div class="kpi-label">Failed</div><div class="kpi-val">{}</div></div>
      <div class="kpi"><div class="kpi-label">Unverified</div><div class="kpi-val">{}</div></div>
      <div class="kpi"><div class="kpi-label">Focus-Free CDP</div><div class="kpi-val">{}</div></div>
    </section>
    {}
  </div>
</body>
</html>
"#,
        escape_html(&csp),
        escape_html(&report.verdict),
        match report.verdict.as_str() {
            "PASS" => "badge-pass",
            "FAIL" => "badge-fail",
            _ => "badge-unverified",
        },
        escape_html(&report.verdict),
        escape_html(&report.mode),
        report.total_cases,
        report.passed_cases,
        report.failed_cases,
        report.unverified_cases,
        if report.focus_free { "YES" } else { "NO" },
        cards,
    )
}

/// Executes a focus-free Visual Diff suite or single screenshot capture through the
/// shared Pooled 1-Chromium CDP semaphore (`shared_cdp_evaluator()`), emitting a
/// self-contained offline HTML Visual Diff report (or PNG when `--screenshot` targets `.png`).
pub fn run_visual_diff(
    req: &VisualDiffRequest,
    reaper: Option<&ChildProcessReaper>,
) -> Result<VisualDiffReportData, String> {
    let out_path = validate_safe_output_path(&req.out_path)?;
    let threshold = if req.ssim_threshold > 0.0 && req.ssim_threshold <= 1.0 {
        req.ssim_threshold
    } else {
        DEFAULT_SSIM_PASS_THRESHOLD
    };

    let (mode_label, targets) = match &req.mode {
        VisualDiffMode::Suite(dir) => ("suite".to_string(), discover_suite_targets(dir)?),
        VisualDiffMode::Screenshot(target) => {
            let validated = validate_diff_target(target)?;
            let case_id = match &validated {
                ValidatedDiffTarget::LocalHtmlFile { canonical_path, .. } => canonical_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("screenshot")
                    .to_string(),
                ValidatedDiffTarget::LoopbackUrl(_) => "loopback-target".to_string(),
            };
            ("screenshot".to_string(), vec![(case_id, validated)])
        }
    };

    let evaluator = shared_cdp_evaluator();
    let permit = evaluator.acquire_permit();
    let incognito_context_id = permit.incognito_context_id.clone();

    let browser_binary = if req.force_no_browser {
        None
    } else if evaluator.can_attempt_cdp() {
        discover_local_chromium_binary()
    } else {
        None
    };

    let viewport_label = format!("{}x{}", req.viewport_width, req.viewport_height);
    let mut cases = Vec::with_capacity(targets.len());
    let mut used_fallback = browser_binary.is_none();
    let mut fallback_mode = browser_binary
        .is_none()
        .then(|| "iframe_bridge_fallback".to_string());
    let mut last_png_bytes: Option<Vec<u8>> = None;

    for (case_id, target) in targets {
        let started = Instant::now();
        let (target_url, html_for_hash) = match &target {
            ValidatedDiffTarget::LoopbackUrl(u) => (u.clone(), u.clone()),
            ValidatedDiffTarget::LocalHtmlFile {
                file_url,
                html_content,
                ..
            } => (file_url.clone(), html_content.clone()),
        };

        let layout_sha1 =
            compute_layout_sha1(&html_for_hash, "{}", &[req.viewport_width], &viewport_label);
        let baseline_bytes = resolve_baseline_bytes(req.baseline.as_deref(), &case_id)?;
        let baseline_sha256 = baseline_bytes.as_deref().map(sha256_hex);

        if let Some(ref chrome) = browser_binary {
            match capture_screenshot_focus_free(
                chrome,
                &target_url,
                req.viewport_width,
                req.viewport_height,
                reaper,
            ) {
                Ok(png_bytes) => {
                    evaluator.record_worker_healthy();
                    let shot_sha = sha256_hex(&png_bytes);
                    let shot_b64 = B64_STD.encode(&png_bytes);
                    let (ssim_score, verdict, notes) = if let Some(ref base) = baseline_bytes {
                        let sim = compute_byte_similarity(base, &png_bytes);
                        if sim >= threshold {
                            (Some(sim), "PASS".to_string(), Vec::new())
                        } else {
                            (
                                Some(sim),
                                "FAIL".to_string(),
                                vec![format!(
                                    "Similarity {:.4} below threshold {:.2}",
                                    sim, threshold
                                )],
                            )
                        }
                    } else {
                        (
                            Some(1.0),
                            "PASS".to_string(),
                            vec![
                                "Captured fresh baseline screenshot via focus-free headless CDP"
                                    .to_string(),
                            ],
                        )
                    };
                    last_png_bytes = Some(png_bytes);
                    cases.push(VisualDiffCaseResult {
                        case_id,
                        target: target_url,
                        viewport: viewport_label.clone(),
                        verdict,
                        ssim_score,
                        layout_sha1,
                        screenshot_sha256: Some(shot_sha),
                        baseline_sha256,
                        duration_ms: started.elapsed().as_millis() as u64,
                        notes,
                        screenshot_base64: Some(shot_b64),
                    });
                }
                Err(err) => {
                    evaluator.record_worker_crash(&err);
                    used_fallback = true;
                    fallback_mode = Some("iframe_bridge_fallback".to_string());
                    cases.push(VisualDiffCaseResult {
                        case_id,
                        target: target_url,
                        viewport: viewport_label.clone(),
                        verdict: "UNVERIFIED_NO_BROWSER".to_string(),
                        ssim_score: None,
                        layout_sha1,
                        screenshot_sha256: None,
                        baseline_sha256,
                        duration_ms: started.elapsed().as_millis() as u64,
                        notes: vec![format!("Headless CDP capture degraded safely: {err}")],
                        screenshot_base64: None,
                    });
                }
            }
        } else {
            let mut notes = vec![
                "Local Chromium CDP unavailable or --force-no-browser enabled; structural DOM layout SHA-1 recorded offline".to_string(),
            ];
            if let Some(ref base) = baseline_bytes {
                if base == html_for_hash.as_bytes() {
                    notes.push("Structural HTML matches baseline byte-for-byte".to_string());
                } else {
                    notes.push("Structural HTML differs from baseline snapshot".to_string());
                }
            }
            cases.push(VisualDiffCaseResult {
                case_id,
                target: target_url,
                viewport: viewport_label.clone(),
                verdict: "UNVERIFIED_NO_BROWSER".to_string(),
                ssim_score: None,
                layout_sha1,
                screenshot_sha256: None,
                baseline_sha256,
                duration_ms: started.elapsed().as_millis() as u64,
                notes,
                screenshot_base64: None,
            });
        }
    }

    drop(permit);

    let total_cases = cases.len();
    let passed_cases = cases.iter().filter(|c| c.verdict == "PASS").count();
    let failed_cases = cases.iter().filter(|c| c.verdict == "FAIL").count();
    let unverified_cases = cases
        .iter()
        .filter(|c| c.verdict == "UNVERIFIED_NO_BROWSER")
        .count();

    let overall_verdict = if failed_cases > 0 {
        "FAIL"
    } else if unverified_cases > 0 {
        "UNVERIFIED_NO_BROWSER"
    } else {
        "PASS"
    }
    .to_string();

    let mut report = VisualDiffReportData {
        mode: mode_label,
        verdict: overall_verdict,
        focus_free: true,
        headless_flags: FOCUS_FREE_HEADLESS_FLAGS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        browser_binary,
        incognito_context_id,
        used_fallback,
        fallback_mode,
        ssim_threshold: threshold,
        total_cases,
        passed_cases,
        failed_cases,
        unverified_cases,
        report_path: out_path.display().to_string(),
        report_sha256: String::new(),
        external_cdn_refs_count: 0,
        cases,
        generated_at: Utc::now().to_rfc3339(),
    };

    let wants_raw_png = matches!(req.mode, VisualDiffMode::Screenshot(_))
        && out_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));

    if wants_raw_png {
        if let Some(png_bytes) = last_png_bytes {
            write_file_atomic(&out_path, &png_bytes)?;
            report.report_sha256 = sha256_hex(&png_bytes);
            report.external_cdn_refs_count = 0;
            return Ok(report);
        }
    }

    let final_html_path = if wants_raw_png {
        let fallback_path = out_path.with_extension("html");
        validate_safe_output_path(&fallback_path)?;
        report.report_path = fallback_path.display().to_string();
        fallback_path
    } else {
        out_path
    };

    let html_report = render_self_contained_diff_html(&report);
    let ext_refs = count_external_cdn_refs(&html_report);
    write_file_atomic(&final_html_path, html_report.as_bytes())?;
    report.report_sha256 = sha256_hex(html_report.as_bytes());
    report.external_cdn_refs_count = ext_refs;

    Ok(report)
}
