// @story 08.5
// @implements AC-01, AC-05 (archify JSON-IR->SVG & get_artifact BFS bundler with cycle guard + O_NOFOLLOW)
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const GET_ARTIFACT_MAX_DEPTH: usize = 6;
pub const GET_ARTIFACT_MAX_FILES: usize = 64;
pub const GET_ARTIFACT_MAX_FILE_BYTES: usize = 512 * 1024;
pub const ARCHIFY_MAX_EDGES: usize = 512;

const FORBIDDEN_INTERNAL_SEGMENTS: &[&str] = &[
    ".cowok-design",
    ".git",
    ".env",
    ".ssh",
    ".aws",
    ".gnupg",
    "daemon.lock",
];

#[derive(Debug, Error)]
pub enum ArchifyBfsError {
    #[error("ARCHIFY_INVALID_IR: {0}")]
    InvalidIr(String),
    #[error("ARTIFACT_SYMLINK_REJECTED: symlink refused at '{0}'")]
    SymlinkRejected(String),
    #[error("ARTIFACT_PATH_TRAVERSAL: path escapes workspace root '{0}'")]
    PathTraversal(String),
    #[error("ARTIFACT_IO_ERROR: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchifyNode {
    pub id: String,
    pub label: String,
    #[serde(default = "default_node_layer")]
    pub layer: usize,
    #[serde(default)]
    pub kind: Option<String>,
}

fn default_node_layer() -> usize {
    0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchifyEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchifyGraphIr {
    pub title: String,
    pub nodes: Vec<ArchifyNode>,
    #[serde(default)]
    pub edges: Vec<ArchifyEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchifySvgOutput {
    pub svg: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub svg_sha256: String,
}

fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Renders a deterministic, XSS-safe architecture SVG from JSON-IR (`archify`).
pub fn render_archify_svg(ir: &ArchifyGraphIr) -> Result<ArchifySvgOutput, ArchifyBfsError> {
    if ir.nodes.is_empty() {
        return Err(ArchifyBfsError::InvalidIr(
            "nodes array must contain at least one node".to_string(),
        ));
    }
    if ir.nodes.len() > 128 {
        return Err(ArchifyBfsError::InvalidIr(
            "nodes array exceeds deterministic cap of 128".to_string(),
        ));
    }
    if ir.edges.len() > ARCHIFY_MAX_EDGES {
        return Err(ArchifyBfsError::InvalidIr(format!(
            "edges array exceeds deterministic cap of {ARCHIFY_MAX_EDGES}"
        )));
    }

    let mut layers: HashMap<usize, Vec<&ArchifyNode>> = HashMap::new();
    for node in &ir.nodes {
        if node.id.trim().is_empty() {
            return Err(ArchifyBfsError::InvalidIr(
                "node id must not be empty".to_string(),
            ));
        }
        layers.entry(node.layer).or_default().push(node);
    }
    let mut layer_keys: Vec<usize> = layers.keys().copied().collect();
    layer_keys.sort_unstable();

    let node_w: usize = 180;
    let node_h: usize = 56;
    let gap_x: usize = 48;
    let gap_y: usize = 72;
    let pad_x: usize = 36;
    let pad_y: usize = 56;

    let max_cols = layer_keys
        .iter()
        .map(|k| layers.get(k).map_or(1, Vec::len))
        .max()
        .unwrap_or(1);
    let width = pad_x * 2 + max_cols * node_w + max_cols.saturating_sub(1) * gap_x;
    let height =
        pad_y * 2 + layer_keys.len() * node_h + layer_keys.len().saturating_sub(1) * gap_y + 24;

    let mut coords: HashMap<&str, (usize, usize)> = HashMap::new();
    for (row_idx, layer_key) in layer_keys.iter().enumerate() {
        if let Some(row_nodes) = layers.get(layer_key) {
            for (col_idx, node) in row_nodes.iter().enumerate() {
                let x = pad_x + col_idx * (node_w + gap_x);
                let y = pad_y + row_idx * (node_h + gap_y);
                coords.insert(node.id.as_str(), (x, y));
            }
        }
    }

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="{title}">"#,
        title = xml_escape(&ir.title)
    ));
    svg.push_str(
        r#"<defs><marker id="archify-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 1 L 10 5 L 0 9 z" fill="var(--border-strong, #475569)"/></marker></defs>"#,
    );
    svg.push_str(&format!(
        r#"<rect x="0" y="0" width="{width}" height="{height}" fill="var(--bg-surface, #ffffff)" rx="8"/>"#
    ));
    svg.push_str(&format!(
        r#"<text x="{pad_x}" y="30" font-family="var(--font-sans, sans-serif)" font-size="14" font-weight="600" fill="var(--text-primary, #0f172a)">{title}</text>"#,
        title = xml_escape(&ir.title)
    ));

    let mut rendered_edges = 0usize;
    for edge in &ir.edges {
        if let (Some(&(x1, y1)), Some(&(x2, y2))) =
            (coords.get(edge.from.as_str()), coords.get(edge.to.as_str()))
        {
            let sx = x1 + node_w / 2;
            let sy = y1 + node_h;
            let tx = x2 + node_w / 2;
            let ty = y2;
            svg.push_str(&format!(
                r#"<line x1="{sx}" y1="{sy}" x2="{tx}" y2="{ty}" stroke="var(--border-strong, #475569)" stroke-width="1.5" marker-end="url(#archify-arrow)"/>"#
            ));
            if let Some(ref lbl) = edge.label {
                let mx = (sx + tx) / 2;
                let my = (sy + ty) / 2;
                svg.push_str(&format!(
                    r#"<text x="{mx}" y="{my}" text-anchor="middle" font-family="var(--font-mono, monospace)" font-size="10" fill="var(--text-muted, #64748b)">{}</text>"#,
                    xml_escape(lbl)
                ));
            }
            rendered_edges += 1;
        }
    }

    for node in &ir.nodes {
        if let Some(&(x, y)) = coords.get(node.id.as_str()) {
            let cx = x + node_w / 2;
            let cy = y + node_h / 2 + 4;
            svg.push_str(&format!(
                r#"<g data-archify-node="{id}"><rect x="{x}" y="{y}" width="{node_w}" height="{node_h}" rx="6" fill="var(--bg-workspace, #f8fafc)" stroke="var(--border-default, #cbd5e1)" stroke-width="1.5"/><text x="{cx}" y="{cy}" text-anchor="middle" font-family="var(--font-sans, sans-serif)" font-size="12" font-weight="600" fill="var(--text-primary, #0f172a)">{label}</text></g>"#,
                id = xml_escape(&node.id),
                label = xml_escape(&node.label),
            ));
        }
    }
    svg.push_str("</svg>");

    let svg_sha256 = format!("{:x}", Sha256::digest(svg.as_bytes()));
    Ok(ArchifySvgOutput {
        svg,
        node_count: ir.nodes.len(),
        edge_count: rendered_edges,
        svg_sha256,
    })
}

/// Parses JSON-IR string or synthesizes a minimal graph IR from a source artifact.
pub fn archify_from_text(
    content: &str,
    fallback_title: &str,
) -> Result<ArchifySvgOutput, ArchifyBfsError> {
    if let Ok(ir) = serde_json::from_str::<ArchifyGraphIr>(content) {
        return render_archify_svg(&ir);
    }
    let refs = extract_relative_refs(content);
    let mut nodes = vec![ArchifyNode {
        id: "root".to_string(),
        label: fallback_title.to_string(),
        layer: 0,
        kind: Some("entry".to_string()),
    }];
    let mut edges = Vec::new();
    for (idx, dep) in refs.into_iter().take(12).enumerate() {
        let id = format!("dep-{idx}");
        nodes.push(ArchifyNode {
            id: id.clone(),
            label: dep,
            layer: 1,
            kind: Some("dependency".to_string()),
        });
        edges.push(ArchifyEdge {
            from: "root".to_string(),
            to: id,
            label: Some("refs".to_string()),
        });
    }
    render_archify_svg(&ArchifyGraphIr {
        title: fallback_title.to_string(),
        nodes,
        edges,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledArtifactEntry {
    pub relative_path: String,
    pub depth: usize,
    pub byte_len: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactBundleReport {
    pub entry_path: String,
    pub max_depth: usize,
    pub max_files: usize,
    pub files: Vec<BundledArtifactEntry>,
    pub cycles_detected: usize,
    pub depth_capped_refs: usize,
    pub file_cap_reached: bool,
    pub bundle_sha256: String,
}

fn ref_extract_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?:(?:from|import|require\(|@import)\s*["'](\.\.?/[^"'\r\n]{1,180})["']|(?:href|src)\s*=\s*["'](\.\.?/[^"'\r\n]{1,180})["']|\[[^\]]{1,80}\]\((\.\.?/[^)\r\n]{1,180})\))"#,
        )
        .expect("static ref regex")
    })
}

pub fn extract_relative_refs(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for caps in ref_extract_regex().captures_iter(content) {
        let matched = caps
            .get(1)
            .or_else(|| caps.get(2))
            .or_else(|| caps.get(3))
            .map(|m| m.as_str().trim());
        if let Some(raw) = matched {
            let clean = raw
                .split('#')
                .next()
                .unwrap_or(raw)
                .split('?')
                .next()
                .unwrap_or(raw);
            if !clean.is_empty() && seen.insert(clean.to_string()) {
                out.push(clean.to_string());
            }
        }
    }
    out
}

fn is_forbidden_secret_segment(seg: &str) -> bool {
    let lower = seg.trim().to_ascii_lowercase();
    FORBIDDEN_INTERNAL_SEGMENTS
        .iter()
        .any(|&s| lower == s || (s == ".env" && lower.starts_with(".env.")))
}

/// Checks every path prefix under `root` to guarantee no symlink component or internal secret segment exists.
pub fn assert_no_symlink_under_root(
    root: &Path,
    candidate: &Path,
) -> Result<PathBuf, ArchifyBfsError> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| ArchifyBfsError::Io(format!("canonicalize root: {e}")))?;

    // Normalize relative segments without following symlinks
    let mut normalized = canonical_root.clone();
    let rel = if candidate.is_absolute() {
        candidate
            .strip_prefix(&canonical_root)
            .or_else(|_| candidate.strip_prefix(root))
            .map_err(|_| ArchifyBfsError::PathTraversal(candidate.display().to_string()))?
    } else {
        candidate
    };

    for comp in rel.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(seg) => {
                if is_forbidden_secret_segment(&seg.to_string_lossy()) {
                    return Err(ArchifyBfsError::PathTraversal(
                        candidate.display().to_string(),
                    ));
                }
                normalized.push(seg);
                if let Ok(meta) = fs::symlink_metadata(&normalized) {
                    if meta.file_type().is_symlink() {
                        return Err(ArchifyBfsError::SymlinkRejected(
                            normalized.display().to_string(),
                        ));
                    }
                }
            }
            Component::ParentDir => {
                if !normalized.pop() || !normalized.starts_with(&canonical_root) {
                    return Err(ArchifyBfsError::PathTraversal(
                        candidate.display().to_string(),
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ArchifyBfsError::PathTraversal(
                    candidate.display().to_string(),
                ));
            }
        }
    }

    if !normalized.starts_with(&canonical_root) {
        return Err(ArchifyBfsError::PathTraversal(
            candidate.display().to_string(),
        ));
    }
    Ok(normalized)
}

/// Reads a regular file safely with `O_NOFOLLOW` and bounded max size.
pub fn read_file_nofollow_bounded(
    root: &Path,
    path: &Path,
    max_bytes: usize,
) -> Result<(PathBuf, Vec<u8>), ArchifyBfsError> {
    let safe_path = assert_no_symlink_under_root(root, path)?;
    let meta = fs::symlink_metadata(&safe_path)
        .map_err(|e| ArchifyBfsError::Io(format!("{}: {e}", safe_path.display())))?;
    if meta.file_type().is_symlink() {
        return Err(ArchifyBfsError::SymlinkRejected(
            safe_path.display().to_string(),
        ));
    }
    if !meta.file_type().is_file() {
        return Err(ArchifyBfsError::Io(format!(
            "Not a regular file: {}",
            safe_path.display()
        )));
    }

    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }

    let mut file = opts.open(&safe_path).map_err(|e| {
        #[cfg(unix)]
        if e.raw_os_error() == Some(libc::ELOOP) {
            return ArchifyBfsError::SymlinkRejected(safe_path.display().to_string());
        }
        ArchifyBfsError::Io(format!("{}: {e}", safe_path.display()))
    })?;

    let mut buf = Vec::new();
    let mut limited = file.by_ref().take((max_bytes + 1) as u64);
    limited
        .read_to_end(&mut buf)
        .map_err(|e| ArchifyBfsError::Io(format!("{}: {e}", safe_path.display())))?;
    if buf.len() > max_bytes {
        return Err(ArchifyBfsError::Io(format!(
            "File {} exceeds max byte limit {max_bytes}",
            safe_path.display()
        )));
    }
    Ok((safe_path, buf))
}

/// BFS artifact dependency bundler (`get_artifact`) with `visited` cycle guard,
/// `MAX_DEPTH`, `MAX_FILES`, and `O_NOFOLLOW` symlink defense.
pub fn bundle_artifacts_bfs(
    workspace_root: &Path,
    entry_rel_path: &Path,
    max_depth: usize,
    max_files: usize,
) -> Result<ArtifactBundleReport, ArchifyBfsError> {
    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|e| ArchifyBfsError::Io(format!("canonicalize workspace root: {e}")))?;
    let effective_max_depth = max_depth.min(GET_ARTIFACT_MAX_DEPTH);
    let effective_max_files = max_files.clamp(1, GET_ARTIFACT_MAX_FILES);

    let start_safe = assert_no_symlink_under_root(&canonical_root, entry_rel_path)?;
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut files: Vec<BundledArtifactEntry> = Vec::new();
    let mut cycles_detected = 0usize;
    let mut depth_capped_refs = 0usize;
    let mut file_cap_reached = false;
    let mut bundle_hasher = Sha256::new();

    visited.insert(start_safe.clone());
    queue.push_back((start_safe, 0));

    while let Some((current_path, depth)) = queue.pop_front() {
        if files.len() >= effective_max_files {
            file_cap_reached = true;
            break;
        }
        let (safe_path, bytes) = read_file_nofollow_bounded(
            &canonical_root,
            &current_path,
            GET_ARTIFACT_MAX_FILE_BYTES,
        )?;
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let rel_str = safe_path
            .strip_prefix(&canonical_root)
            .unwrap_or(&safe_path)
            .to_string_lossy()
            .replace('\\', "/");

        bundle_hasher.update(format!("{rel_str}:{sha}\n").as_bytes());
        files.push(BundledArtifactEntry {
            relative_path: rel_str,
            depth,
            byte_len: bytes.len(),
            sha256: sha,
        });

        let text = String::from_utf8_lossy(&bytes);
        let parent = safe_path.parent().unwrap_or(&canonical_root);
        for rel_ref in extract_relative_refs(&text) {
            if depth >= effective_max_depth {
                depth_capped_refs += 1;
                continue;
            }
            let joined = parent.join(&rel_ref);
            let next_safe = match assert_no_symlink_under_root(&canonical_root, &joined) {
                Ok(p) => p,
                Err(ArchifyBfsError::SymlinkRejected(s)) => {
                    return Err(ArchifyBfsError::SymlinkRejected(s));
                }
                Err(ArchifyBfsError::PathTraversal(s)) => {
                    return Err(ArchifyBfsError::PathTraversal(s));
                }
                Err(_) => continue,
            };
            let meta = match fs::symlink_metadata(&next_safe) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() {
                return Err(ArchifyBfsError::SymlinkRejected(
                    next_safe.display().to_string(),
                ));
            }
            // M-03: skip directory/special-file imports rather than aborting the whole BFS bundle
            if !meta.file_type().is_file() {
                continue;
            }
            if visited.contains(&next_safe) {
                cycles_detected += 1;
                continue;
            }
            if files.len() + queue.len() >= effective_max_files {
                file_cap_reached = true;
                continue;
            }
            visited.insert(next_safe.clone());
            queue.push_back((next_safe, depth + 1));
        }
    }

    Ok(ArtifactBundleReport {
        entry_path: entry_rel_path.to_string_lossy().replace('\\', "/"),
        max_depth: effective_max_depth,
        max_files: effective_max_files,
        files,
        cycles_detected,
        depth_capped_refs,
        file_cap_reached,
        bundle_sha256: format!("{:x}", bundle_hasher.finalize()),
    })
}
