// @story 12.1
// @implements AC-01 (Embedded React 19 Workbench SPA Resolution & 7 Built-in Craft Skills + 7 DE Tools Registry)
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use plugin_host::BuiltInDeTool;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Compiled-in `packages/builtin-skills/design-craft/plugin.json` manifest so the single binary
/// remains self-contained even when copied outside the repository (e.g. `/opt/homebrew/bin`).
pub const EMBEDDED_DESIGN_CRAFT_PLUGIN_JSON: &str =
    include_str!("../../../packages/builtin-skills/design-craft/plugin.json");

const SKILL_PHASE_0: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/phase-0-intent-framing/SKILL.md"
);
const SKILL_PHASE_1: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/phase-1-structural-sketch/SKILL.md"
);
const SKILL_PHASE_2: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/phase-2-brand-visual-studio/SKILL.md"
);
const SKILL_PHASE_3: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/phase-3-synthesis-polish/SKILL.md"
);
const SKILL_PHASE_4: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/phase-4-verify-handoff/SKILL.md"
);
const SKILL_CRAFT_IMPECCABLE: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/craft/impeccable-polish/SKILL.md"
);
const SKILL_DOMAIN_BUSINESS: &str = include_str!(
    "../../../packages/builtin-skills/design-craft/skills/domain-business/business-requirements-distiller/SKILL.md"
);

const MAX_STATIC_ASSET_BYTES: u64 = 16 * 1024 * 1024;

/// Embedded offline React 19 Workbench HTML shell fallback used when no prebuilt `dist/`
/// directory exists (clean checkouts, `cargo test`, or standalone cold start).
pub const EMBEDDED_WORKBENCH_SHELL_HTML: &str = r#"<!DOCTYPE html>
<html lang="en" data-theme="warm">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; base-uri 'self'; form-action 'self'; object-src 'none';" />
    <title>cowok-design — Standalone Local Workbench</title>
    <style>
      :root { color-scheme: dark; --bg: #0f172a; --card: #1e293b; --fg: #f8fafc; --muted: #94a3b8; --accent: #38bdf8; --border: #334155; }
      body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: var(--bg); color: var(--fg); min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 24px; box-sizing: border-box; }
      main { max-width: 720px; width: 100%; background: var(--card); border: 1px solid var(--border); border-radius: 14px; padding: 28px; box-shadow: 0 20px 48px rgba(0,0,0,0.35); }
      h1 { margin: 0 0 8px; font-size: 22px; }
      p { color: var(--muted); line-height: 1.55; margin: 0 0 16px; }
      code { background: rgba(15, 23, 42, 0.7); padding: 2px 6px; border-radius: 6px; color: var(--accent); font-size: 13px; }
      ul { margin: 12px 0 0; padding-left: 20px; color: var(--muted); }
      li { margin-bottom: 6px; }
    </style>
  </head>
  <body>
    <div id="root" data-cowok-standalone="true" data-api-base="/api">
      <main>
        <h1>cowok-design Standalone Local Workbench</h1>
        <p>Loopback Axum daemon active with standalone <code>/api/* -&gt; /api/v1/*</code> parity alias, 7 built-in craft skill families, and 7 Rust DE tools.</p>
        <ul>
          <li>Health endpoint: <code>GET /api/health</code></li>
          <li>CLI Doctor matrix: <code>GET /api/doctor/cli-matrix</code></li>
          <li>Built-in skills catalog: <code>GET /api/skills/catalog</code></li>
        </ul>
      </main>
    </div>
  </body>
</html>
"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedCraftSkill {
    pub id: String,
    pub family: String,
    pub byte_size: usize,
    pub sha256: String,
    #[serde(skip)]
    pub content: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedBuiltinSkillsRegistry {
    pub plugin_name: String,
    pub plugin_version: String,
    pub manifest_sha256: String,
    pub skills: Vec<EmbeddedCraftSkill>,
    pub de_tools: Vec<String>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

impl EmbeddedBuiltinSkillsRegistry {
    /// Loads the compiled-in 7 Built-in Craft Skills families + 7 Rust DE tools with
    /// deterministic SHA-256 digests and zero filesystem I/O.
    pub fn load() -> Self {
        let raw_entries: [(&str, &str, &'static str); 7] = [
            ("phase-0-intent-framing", "phase-0", SKILL_PHASE_0),
            ("phase-1-structural-sketch", "phase-1", SKILL_PHASE_1),
            ("phase-2-brand-visual-studio", "phase-2", SKILL_PHASE_2),
            ("phase-3-synthesis-polish", "phase-3", SKILL_PHASE_3),
            ("phase-4-verify-handoff", "phase-4", SKILL_PHASE_4),
            ("craft/impeccable-polish", "craft", SKILL_CRAFT_IMPECCABLE),
            (
                "domain-business/business-requirements-distiller",
                "domain-business",
                SKILL_DOMAIN_BUSINESS,
            ),
        ];

        let skills = raw_entries
            .into_iter()
            .map(|(id, family, content)| EmbeddedCraftSkill {
                id: id.to_string(),
                family: family.to_string(),
                byte_size: content.len(),
                sha256: sha256_hex(content.as_bytes()),
                content,
            })
            .collect();

        let de_tools = BuiltInDeTool::all()
            .iter()
            .map(|tool| tool.as_str().to_string())
            .collect();

        let parsed: serde_json::Value =
            serde_json::from_str(EMBEDDED_DESIGN_CRAFT_PLUGIN_JSON).unwrap_or_default();
        let plugin_name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("design-craft")
            .to_string();
        let plugin_version = parsed
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0")
            .to_string();

        Self {
            plugin_name,
            plugin_version,
            manifest_sha256: sha256_hex(EMBEDDED_DESIGN_CRAFT_PLUGIN_JSON.as_bytes()),
            skills,
            de_tools,
        }
    }

    pub fn skills(&self) -> &[EmbeddedCraftSkill] {
        &self.skills
    }

    pub fn de_tools(&self) -> &[String] {
        &self.de_tools
    }

    pub fn plugin_manifest_json(&self) -> &'static str {
        EMBEDDED_DESIGN_CRAFT_PLUGIN_JSON
    }
}

/// Static asset and SPA history-fallback handler for standalone `cowok-design up`.
#[derive(Clone, Debug)]
pub struct StaticSpaHandler {
    dist_dir: Option<PathBuf>,
    skills_count: usize,
    de_tools_count: usize,
}

impl StaticSpaHandler {
    /// Resolves the React 19 `dist/` directory in deterministic priority order:
    /// 1. `explicit_dist_dir` (if provided and valid non-symlink directory with `index.html`)
    /// 2. `COWOK_DIST_DIR` environment variable
    /// 3. `<exe_dir>/dist`
    /// 4. `<workspace_root>/dist`
    /// 5. Embedded standalone HTML shell fallback (`dist_dir = None`)
    pub fn discover(explicit_dist_dir: Option<&Path>, workspace_root: Option<&Path>) -> Self {
        let registry = EmbeddedBuiltinSkillsRegistry::load();
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = explicit_dist_dir {
            candidates.push(p.to_path_buf());
        }
        if let Some(env_dir) = std::env::var_os("COWOK_DIST_DIR").filter(|v| !v.is_empty()) {
            candidates.push(PathBuf::from(env_dir));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("dist"));
            }
        }
        if let Some(ws) = workspace_root {
            candidates.push(ws.join("dist"));
        }

        let resolved = candidates
            .into_iter()
            .find_map(|cand| validate_dist_dir(&cand));

        Self {
            dist_dir: resolved,
            skills_count: registry.skills.len(),
            de_tools_count: registry.de_tools.len(),
        }
    }

    /// Creates a handler using only the built-in embedded standalone HTML shell fallback.
    pub fn embedded_fallback() -> Self {
        let registry = EmbeddedBuiltinSkillsRegistry::load();
        Self {
            dist_dir: None,
            skills_count: registry.skills.len(),
            de_tools_count: registry.de_tools.len(),
        }
    }

    pub fn dist_dir(&self) -> Option<&Path> {
        self.dist_dir.as_deref()
    }

    pub fn embedded_skills_count(&self) -> usize {
        self.skills_count
    }

    pub fn embedded_de_tools_count(&self) -> usize {
        self.de_tools_count
    }

    pub fn handle_request(&self, method: &Method, raw_path: &str) -> Response {
        if *method != Method::GET && *method != Method::HEAD {
            return (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed").into_response();
        }
        if has_unsafe_path_tokens(raw_path) {
            return (StatusCode::BAD_REQUEST, "Invalid static asset path").into_response();
        }

        let rel = raw_path.trim_start_matches('/');
        let Some(ref dist_root) = self.dist_dir else {
            return serve_embedded_shell(method, rel);
        };

        if rel.is_empty() {
            return serve_dist_file(dist_root, Path::new("index.html"), method, false);
        }

        let rel_path = Path::new(rel);
        if rel_path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return (StatusCode::BAD_REQUEST, "Path traversal denied").into_response();
        }

        match inspect_safe_target(dist_root, rel_path) {
            SafeTarget::File(full_path) => {
                let is_immutable = rel.starts_with("assets/");
                serve_checked_file(&full_path, method, is_immutable)
            }
            SafeTarget::SymlinkOrEscape => {
                (StatusCode::FORBIDDEN, "Symlink or path escape denied").into_response()
            }
            SafeTarget::NotFound => {
                // Client-side SPA route fallback for extensionless paths; 404 for missing static assets.
                if rel_path.extension().is_some() {
                    (StatusCode::NOT_FOUND, "Static asset not found").into_response()
                } else {
                    serve_dist_file(dist_root, Path::new("index.html"), method, false)
                }
            }
        }
    }
}

enum SafeTarget {
    File(PathBuf),
    SymlinkOrEscape,
    NotFound,
}

fn validate_dist_dir(candidate: &Path) -> Option<PathBuf> {
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return None;
    }
    let meta = fs::symlink_metadata(candidate).ok()?;
    if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
        return None;
    }
    let canon = candidate.canonicalize().ok()?;
    let idx = canon.join("index.html");
    let idx_meta = fs::symlink_metadata(&idx).ok()?;
    if idx_meta.file_type().is_symlink() || !idx_meta.file_type().is_file() {
        return None;
    }
    Some(canon)
}

fn has_unsafe_path_tokens(raw_path: &str) -> bool {
    let lower = raw_path.to_ascii_lowercase();
    raw_path.contains('\0')
        || raw_path.contains('\\')
        || raw_path.contains("..")
        || lower.contains("%2e%2e")
        || lower.contains("%5c")
        || lower.contains("%00")
}

fn inspect_safe_target(canon_root: &Path, rel: &Path) -> SafeTarget {
    let mut cur = canon_root.to_path_buf();
    for comp in rel.components() {
        let Component::Normal(seg) = comp else {
            return SafeTarget::SymlinkOrEscape;
        };
        cur.push(seg);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return SafeTarget::SymlinkOrEscape;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return SafeTarget::NotFound;
            }
            Err(_) => return SafeTarget::SymlinkOrEscape,
        }
    }
    let Ok(meta) = fs::symlink_metadata(&cur) else {
        return SafeTarget::NotFound;
    };
    if meta.file_type().is_symlink() {
        return SafeTarget::SymlinkOrEscape;
    }
    if !meta.file_type().is_file() {
        return SafeTarget::NotFound;
    }
    let Ok(canon) = cur.canonicalize() else {
        return SafeTarget::SymlinkOrEscape;
    };
    if !canon.starts_with(canon_root) {
        return SafeTarget::SymlinkOrEscape;
    }
    SafeTarget::File(canon)
}

fn serve_dist_file(dist_root: &Path, rel: &Path, method: &Method, immutable: bool) -> Response {
    match inspect_safe_target(dist_root, rel) {
        SafeTarget::File(full) => serve_checked_file(&full, method, immutable),
        SafeTarget::SymlinkOrEscape => {
            (StatusCode::FORBIDDEN, "Symlink or path escape denied").into_response()
        }
        SafeTarget::NotFound => serve_embedded_shell(method, ""),
    }
}

fn serve_checked_file(full_path: &Path, method: &Method, immutable: bool) -> Response {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let Ok(file) = opts.open(full_path) else {
        return (StatusCode::FORBIDDEN, "Unable to open static asset").into_response();
    };
    let Ok(meta) = file.metadata() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Metadata error").into_response();
    };
    if meta.len() > MAX_STATIC_ASSET_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Static asset exceeds limit").into_response();
    }

    let mut bytes = Vec::with_capacity(meta.len() as usize);
    if file
        .take(MAX_STATIC_ASSET_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response();
    }

    let mime = mime_for_path(full_path);
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    let body = if *method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(bytes)
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache)
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn serve_embedded_shell(method: &Method, rel: &str) -> Response {
    if !rel.is_empty() && Path::new(rel).extension().is_some() {
        return (StatusCode::NOT_FOUND, "Static asset not found").into_response();
    }
    let body = if *method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(EMBEDDED_WORKBENCH_SHELL_HTML)
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
