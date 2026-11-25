#!/usr/bin/env node
// @story 12.2
// @implements AC-01, AC-02, AC-04 (Tauri v2 Bundle Policy, Least-Privilege Capability ACL, Canonical Manifest & Ed25519+SHA256 Sidecar/Updater Verification Harness)
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../../..");

const REQUIRED_TARGETS = ["dmg", "msi", "appimage"];
const REQUIRED_EXTERNAL_BIN = "binaries/cowok-design";
const PLACEHOLDER_UPDATER_PUBKEY = "__COWOK_RELEASE_ED25519_PUBKEY_INJECTED_AT_BUILD__";
const REQUIRED_HUMAN_ONLY_TOOLS = [
  "ephemeral_tool_forge",
  "learn_card_promote",
  "staging_plugin_promote",
  "workspace_destructive_reset",
];
const REQUIRED_HOSTS = {
  cowok: "cowok_ai",
  claude: "claude_code",
  codex: "codex",
  antigravity: "antigravity",
  cursor: "cursor",
};
const FORBIDDEN_CAPABILITY_IDS = new Set([
  "shell:allow-open",
  "shell:default",
  "shell:allow-spawn",
  "shell:allow-stdin-write",
  "dialog:allow-open",
]);
const MAX_UPDATER_BYTES = 64 * 1024 * 1024;
const DESKTOP_BUNDLE_BUDGET_BYTES = 25 * 1024 * 1024;
const READ_CHUNK_BYTES = 64 * 1024;

function fail(code, message, exitCode = 1) {
  console.error(JSON.stringify({ ok: false, code, message }, null, 2));
  process.exit(exitCode);
}

function readJsonStrict(filePath) {
  if (!fs.existsSync(filePath)) {
    fail("MISSING_FILE", `Required file not found: ${filePath}`);
  }
  const stat = fs.lstatSync(filePath);
  if (stat.isSymbolicLink()) {
    fail("SYMLINK_REJECTED", `Refusing symlink at ${filePath}`);
  }
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (err) {
    fail("INVALID_JSON", `Failed to parse ${filePath}: ${err.message}`);
  }
}

function canonicalizeJson(value) {
  if (Array.isArray(value)) {
    return value.map(canonicalizeJson);
  }
  if (value !== null && typeof value === "object") {
    const out = {};
    for (const key of Object.keys(value).sort()) {
      out[key] = canonicalizeJson(value[key]);
    }
    return out;
  }
  return value;
}

function parseCspDirectives(cspRaw) {
  const map = new Map();
  for (const rawPart of String(cspRaw ?? "").split(";")) {
    const trimmed = rawPart.trim();
    if (!trimmed) continue;
    const tokens = trimmed.split(/\s+/);
    const directive = tokens[0].toLowerCase();
    map.set(directive, tokens.slice(1));
  }
  return map;
}

function validateStrictCsp(cspRaw) {
  const csp = String(cspRaw ?? "").trim();
  if (!csp) {
    fail("WEAK_CSP", "tauri.conf.json app.security.csp must not be empty");
  }
  if (csp.toLowerCase().includes("'unsafe-eval'")) {
    fail("WEAK_CSP", "tauri.conf.json app.security.csp forbids 'unsafe-eval'");
  }
  const directives = parseCspDirectives(csp);
  const defaultSrc = directives.get("default-src") ?? [];
  const scriptSrc = directives.get("script-src") ?? [];
  const objectSrc = directives.get("object-src") ?? [];
  const frameSrc = directives.get("frame-src") ?? [];

  const checkRestrictedSrc = (name, tokens) => {
    if (!tokens.includes("'self'")) {
      fail("WEAK_CSP", `CSP ${name} must include 'self'`);
    }
    for (const tok of tokens) {
      const lower = tok.toLowerCase();
      if (
        lower === "*" ||
        lower.includes("127.0.0.1") ||
        lower.startsWith("http:") ||
        lower.startsWith("https:") ||
        lower === "'unsafe-eval'" ||
        lower === "'unsafe-inline'"
      ) {
        fail("WEAK_CSP", `CSP ${name} forbids overbroad token '${tok}'`);
      }
    }
  };

  checkRestrictedSrc("default-src", defaultSrc);
  checkRestrictedSrc("script-src", scriptSrc);

  if (objectSrc.length !== 1 || objectSrc[0] !== "'none'") {
    fail("WEAK_CSP", "CSP object-src must be strictly 'none'");
  }
  if (frameSrc.length !== 1 || frameSrc[0] !== "'none'") {
    fail("WEAK_CSP", "CSP frame-src must be strictly 'none'");
  }
}

function checkManifestMode({ sidecarBinaryPath, releaseMode }) {
  const tauriConfPath = path.join(REPO_ROOT, "packaging/tauri/tauri.conf.json");
  const capabilitiesPath = path.join(REPO_ROOT, "packaging/tauri/capabilities/default.json");
  const canonicalManifestPath = path.join(REPO_ROOT, "packaging/plugins/canonical-manifest.json");
  const tauriBuildRsPath = path.join(REPO_ROOT, "packaging/tauri/build.rs");
  const rootCargoPath = path.join(REPO_ROOT, "Cargo.toml");

  if (!fs.existsSync(tauriBuildRsPath)) {
    fail("MISSING_TAURI_BUILD_RS", "packaging/tauri/build.rs is required for Tauri v2 builds");
  }
  const buildRsStat = fs.lstatSync(tauriBuildRsPath);
  if (buildRsStat.isSymbolicLink() || !buildRsStat.isFile()) {
    fail("INVALID_TAURI_BUILD_RS", "packaging/tauri/build.rs must be a regular non-symlink file");
  }

  const tauriConf = readJsonStrict(tauriConfPath);
  const capabilities = readJsonStrict(capabilitiesPath);
  const canonical = readJsonStrict(canonicalManifestPath);

  const checks = ["tauri.build_rs_present"];

  // 1. Validate bundle targets (.dmg/.msi/.AppImage) and externalBin sidecar
  const targets = Array.isArray(tauriConf?.bundle?.targets) ? tauriConf.bundle.targets : [];
  for (const requiredTarget of REQUIRED_TARGETS) {
    if (!targets.includes(requiredTarget)) {
      fail("MISSING_BUNDLE_TARGET", `tauri.conf.json bundle.targets missing '${requiredTarget}'`);
    }
  }
  checks.push("tauri.bundle.targets=dmg,msi,appimage");

  const externalBins = Array.isArray(tauriConf?.bundle?.externalBin)
    ? tauriConf.bundle.externalBin
    : [];
  if (!externalBins.includes(REQUIRED_EXTERNAL_BIN)) {
    fail(
      "MISSING_EXTERNAL_BIN",
      `tauri.conf.json bundle.externalBin must include '${REQUIRED_EXTERNAL_BIN}'`,
    );
  }
  checks.push("tauri.bundle.externalBin=binaries/cowok-design");

  // 2. Validate strict CSP, dragDropEnabled, withGlobalTauri=false, and updater config
  if (tauriConf?.app?.withGlobalTauri !== false) {
    fail("GLOBAL_TAURI_EXPOSED", "tauri.conf.json app.withGlobalTauri must be false");
  }
  const mainWindow = Array.isArray(tauriConf?.app?.windows) ? tauriConf.app.windows[0] : null;
  if (!mainWindow || mainWindow.dragDropEnabled !== true) {
    fail("DRAG_DROP_DISABLED", "tauri.conf.json primary window must enable dragDropEnabled");
  }
  validateStrictCsp(tauriConf?.app?.security?.csp);
  const updater = tauriConf?.plugins?.updater;
  if (!updater || updater.active !== true) {
    fail("UPDATER_INACTIVE", "tauri.conf.json plugins.updater.active must be true");
  }
  const pubkey = String(updater.pubkey ?? "").trim();
  if (!pubkey || pubkey.includes("PRIVATE KEY")) {
    fail(
      "INVALID_UPDATER_PUBKEY",
      "tauri.conf.json plugins.updater.pubkey must be non-empty and never a private key",
    );
  }
  if ((sidecarBinaryPath || releaseMode) && pubkey === PLACEHOLDER_UPDATER_PUBKEY) {
    fail(
      "PLACEHOLDER_UPDATER_PUBKEY",
      "Release/sidecar verification forbids placeholder plugins.updater.pubkey",
    );
  }
  const endpoints = Array.isArray(updater.endpoints) ? updater.endpoints : [];
  if (
    endpoints.length === 0 ||
    !endpoints.every((u) => typeof u === "string" && u.startsWith("https://"))
  ) {
    fail("INSECURE_UPDATER_ENDPOINT", "tauri.conf.json plugins.updater.endpoints must use HTTPS");
  }
  checks.push("tauri.security.csp+updater+dragDrop");

  // 3. Validate least-privilege capabilities/default.json (ZERO raw fs, dialog:allow-open, or shell wildcards)
  const permissions = Array.isArray(capabilities?.permissions) ? capabilities.permissions : [];
  if (permissions.length === 0) {
    fail("EMPTY_CAPABILITIES", "packaging/tauri/capabilities/default.json must declare permissions");
  }
  for (const perm of permissions) {
    if (typeof perm === "string") {
      if (perm === "shell:allow-execute") {
        fail(
          "UNSCOPED_SHELL_EXECUTE",
          "Unscoped string 'shell:allow-execute' is forbidden; require strict sidecar allowlist object",
        );
      }
      if (FORBIDDEN_CAPABILITY_IDS.has(perm)) {
        fail("FORBIDDEN_CAPABILITY", `Forbidden capability permission '${perm}'`);
      }
    }
    const id = typeof perm === "string" ? perm : String(perm?.identifier ?? "");
    if (id.startsWith("fs:") || id.includes("fs:allow-") || id.includes("fs:scope")) {
      fail(
        "FORBIDDEN_FS_PERMISSION",
        `Raw filesystem capability '${id}' is strictly forbidden; use HMAC capability tokens in Rust main`,
      );
    }
    if (FORBIDDEN_CAPABILITY_IDS.has(id)) {
      fail("FORBIDDEN_CAPABILITY", `Forbidden capability permission '${id}'`);
    }
    if (typeof perm === "object" && perm !== null && perm.identifier === "shell:allow-execute") {
      const allowList = Array.isArray(perm.allow) ? perm.allow : [];
      if (
        allowList.length !== 1 ||
        allowList[0]?.name !== REQUIRED_EXTERNAL_BIN ||
        allowList[0]?.sidecar !== true
      ) {
        fail(
          "OVERBROAD_SHELL_EXECUTE",
          "shell:allow-execute must strictly allow only the binaries/cowok-design sidecar",
        );
      }
      const allowArgs = allowList[0]?.args;
      if (
        !Array.isArray(allowArgs) ||
        allowArgs.length === 0 ||
        !allowArgs.every((arg) => typeof arg === "string" && arg.length > 0)
      ) {
        fail(
          "OVERBROAD_SHELL_ARGS",
          "shell:allow-execute allow[0].args must be a non-empty array of literal strings (args: true is forbidden)",
        );
      }
    }
  }
  checks.push("capabilities.least_privilege_no_raw_fs_no_dialog_open");

  // 4. Verify packaging/tauri is NOT in root Cargo.toml workspace members
  const rootCargoRaw = fs.readFileSync(rootCargoPath, "utf8");
  if (rootCargoRaw.includes("packaging/tauri")) {
    fail(
      "TAURI_IN_WORKSPACE_MEMBERS",
      "packaging/tauri must stay isolated outside root Cargo.toml [workspace].members",
    );
  }
  checks.push("workspace.tauri_isolated");

  // 5. Validate canonical-manifest.json (7 skills, 7 DE tools, 5 hosts, 4 HUMAN_ONLY locks, feedback tool)
  if (!Array.isArray(canonical.skillIds) || canonical.skillIds.length !== 7) {
    fail("INVALID_SKILL_COUNT", "canonical-manifest.json must declare all 7 built-in craft skills");
  }
  if (!Array.isArray(canonical.deTools) || canonical.deTools.length !== 7) {
    fail("INVALID_DE_TOOL_COUNT", "canonical-manifest.json must declare all 7 Rust DE tools");
  }
  if (canonical.feedbackToolName !== "cowok_submit_feedback") {
    fail("MISSING_FEEDBACK_TOOL", "canonical-manifest.json must expose cowok_submit_feedback");
  }
  const humanOnlySet = new Set(Array.isArray(canonical.humanOnlyTools) ? canonical.humanOnlyTools : []);
  for (const tool of REQUIRED_HUMAN_ONLY_TOOLS) {
    if (!humanOnlySet.has(tool)) {
      fail("MISSING_HUMAN_ONLY_TOOL", `canonical-manifest.json missing HUMAN_ONLY tool '${tool}'`);
    }
  }
  const mcpTools = Array.isArray(canonical.mcpTools) ? canonical.mcpTools : [];
  for (const tool of REQUIRED_HUMAN_ONLY_TOOLS) {
    const entry = mcpTools.find((t) => t.toolName === tool);
    if (
      !entry ||
      entry.permissionTier !== "HUMAN_ONLY" ||
      entry.mcpCallable !== false ||
      entry.lockedHumanOnly !== true
    ) {
      fail("HUMAN_ONLY_LOCK_VIOLATION", `HUMAN_ONLY invariant violated for '${tool}'`);
    }
  }
  const feedbackEntry = mcpTools.find((t) => t.toolName === "cowok_submit_feedback");
  if (!feedbackEntry || feedbackEntry.mcpCallable !== true) {
    fail("FEEDBACK_TOOL_UNREACHABLE", "cowok_submit_feedback must be mcpCallable=true");
  }
  for (const [hostKey, expectedChannel] of Object.entries(REQUIRED_HOSTS)) {
    if (canonical?.hosts?.[hostKey]?.channel !== expectedChannel) {
      fail("HOST_CHANNEL_MISMATCH", `Host '${hostKey}' must declare channel '${expectedChannel}'`);
    }
  }
  checks.push("canonical_manifest.5_hosts+7_skills+7_de_tools+4_human_only");

  // 6. Optional binary size budget check (~25MB desktop target)
  let sidecarSizeBytes = null;
  if (sidecarBinaryPath) {
    const stat = fs.lstatSync(sidecarBinaryPath);
    if (stat.isSymbolicLink() || !stat.isFile()) {
      fail(
        "INVALID_SIDECAR_BINARY",
        `Sidecar binary must be a regular non-symlink file: ${sidecarBinaryPath}`,
      );
    }
    if (stat.nlink !== 1) {
      fail("HARDLINK_REJECTED", `Sidecar binary has nlink=${stat.nlink}; expected 1`);
    }
    sidecarSizeBytes = stat.size;
    if (sidecarSizeBytes === 0 || sidecarSizeBytes > DESKTOP_BUNDLE_BUDGET_BYTES) {
      fail(
        "SIDECAR_SIZE_BUDGET_EXCEEDED",
        `Sidecar binary size ${sidecarSizeBytes} exceeds ${DESKTOP_BUNDLE_BUDGET_BYTES} byte budget`,
      );
    }
    checks.push(`sidecar.size_bytes=${sidecarSizeBytes}`);
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        mode: "check-manifest",
        checks,
        sidecarSizeBytes,
      },
      null,
      2,
    ),
  );
}

function buildEd25519SpkiDer(raw32PubkeyBytes) {
  // RFC 8410 ASN.1 SubjectPublicKeyInfo header for Ed25519 (OID 1.3.101.112)
  const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
  return Buffer.concat([spkiPrefix, raw32PubkeyBytes]);
}

function validateAndWalkRelativeArtifactPath(rootDir, rawRelPath) {
  if (typeof rawRelPath !== "string" || rawRelPath.length === 0) {
    fail("PATH_TRAVERSAL_DENIED", "Artifact relativePath must be a non-empty string");
  }
  if (
    rawRelPath.includes("\\") ||
    rawRelPath.includes("\0") ||
    rawRelPath.includes("..") ||
    rawRelPath.startsWith("/") ||
    path.isAbsolute(rawRelPath) ||
    path.win32.isAbsolute(rawRelPath)
  ) {
    fail("PATH_TRAVERSAL_DENIED", `Unsafe relativePath '${rawRelPath}'`);
  }
  const segments = rawRelPath.split("/");
  if (
    segments.length === 0 ||
    segments.some((seg) => seg.length === 0 || seg === "." || seg === "..")
  ) {
    fail("PATH_TRAVERSAL_DENIED", `Unsafe relativePath segments in '${rawRelPath}'`);
  }

  let cursor = rootDir;
  for (let i = 0; i < segments.length; i += 1) {
    cursor = path.join(cursor, segments[i]);
    let st;
    try {
      st = fs.lstatSync(cursor);
    } catch (err) {
      fail("MISSING_ARTIFACT_FILE", `Artifact path does not exist: ${rawRelPath} (${err.code ?? err.message})`);
    }
    if (st.isSymbolicLink()) {
      fail("SYMLINK_REJECTED", `Symbolic link rejected in artifact path '${rawRelPath}'`);
    }
    const isLast = i === segments.length - 1;
    if (!isLast && !st.isDirectory()) {
      fail("INVALID_ARTIFACT_FILE", `Ancestor segment is not a directory in '${rawRelPath}'`);
    }
  }
  return cursor;
}

function readAndHashBoundedFd(fd, maxBytes) {
  const hash = crypto.createHash("sha256");
  const chunk = Buffer.allocUnsafe(READ_CHUNK_BYTES);
  let totalRead = 0;
  while (true) {
    const remainingBudget = maxBytes + 1 - totalRead;
    if (remainingBudget <= 0) {
      fail("FILE_TOO_LARGE", `Artifact exceeds ${maxBytes} byte limit`);
    }
    const toRead = Math.min(chunk.length, remainingBudget);
    const n = fs.readSync(fd, chunk, 0, toRead, null);
    if (n === 0) {
      break;
    }
    totalRead += n;
    if (totalRead > maxBytes) {
      fail("FILE_TOO_LARGE", `Artifact exceeds ${maxBytes} byte limit`);
    }
    hash.update(chunk.subarray(0, n));
  }
  return { sha256: hash.digest("hex"), bytesRead: totalRead };
}

function verifySignedMode(manifestPath, pubkeyB64, artifactRootDir, minSeq) {
  const envelope = readJsonStrict(manifestPath);
  const trimmedPubkey = pubkeyB64.trim();
  const rawPub = Buffer.from(trimmedPubkey, "base64");
  if (rawPub.length !== 32) {
    fail("INVALID_PUBKEY", "Ed25519 public key must decode to 32 bytes");
  }
  if (envelope.schemaVersion !== 1) {
    fail("INVALID_SCHEMA_VERSION", `Expected schemaVersion=1, got '${envelope.schemaVersion}'`);
  }
  if (typeof envelope.keyId !== "string" || envelope.keyId.trim().length === 0) {
    fail("INVALID_KEY_ID", "Signed envelope keyId must be non-empty");
  }
  if (envelope.manifestKind !== "desktop_sidecar_integrity") {
    fail("INVALID_MANIFEST_KIND", `Unexpected manifestKind '${envelope.manifestKind}'`);
  }
  if (
    typeof envelope.manifestSeq !== "number" ||
    !Number.isInteger(envelope.manifestSeq) ||
    envelope.manifestSeq < 1
  ) {
    fail("INVALID_MANIFEST_SEQ", `manifestSeq must be an integer >= 1, got '${envelope.manifestSeq}'`);
  }
  if (minSeq !== null && envelope.manifestSeq <= minSeq) {
    fail(
      "ROLLBACK_ATTACK_DETECTED",
      `Rollback detected: manifestSeq=${envelope.manifestSeq} <= minSeq=${minSeq}`,
    );
  }
  if (
    typeof envelope.issuedAt !== "number" ||
    !Number.isFinite(envelope.issuedAt) ||
    typeof envelope.expiresAt !== "number" ||
    !Number.isFinite(envelope.expiresAt) ||
    envelope.issuedAt > envelope.expiresAt
  ) {
    fail("INVALID_TIMESTAMPS", "Signed envelope issuedAt/expiresAt must be finite numbers with issuedAt <= expiresAt");
  }
  const nowSecs = Math.floor(Date.now() / 1000);
  if (envelope.expiresAt <= nowSecs) {
    fail(
      "EXPIRED_MANIFEST",
      `Signed manifest expired (expiresAt=${envelope.expiresAt}, now=${nowSecs})`,
    );
  }
  if (String(envelope.publicKeyBase64 ?? "").trim() !== trimmedPubkey) {
    fail("PUBKEY_MISMATCH", "Signed envelope publicKeyBase64 does not match trusted --pubkey");
  }
  if (!envelope.payload || typeof envelope.payload !== "object" || Array.isArray(envelope.payload)) {
    fail("INVALID_PAYLOAD", "Signed envelope payload must be an object");
  }
  if (envelope.payload.channel !== "desktop") {
    fail("INVALID_CHANNEL", `Expected payload.channel='desktop', got '${envelope.payload.channel}'`);
  }
  if (envelope.payload.releaseSeq !== envelope.manifestSeq) {
    fail(
      "RELEASE_SEQ_MISMATCH",
      `payload.releaseSeq (${envelope.payload.releaseSeq}) must match envelope.manifestSeq (${envelope.manifestSeq})`,
    );
  }
  if (!Array.isArray(envelope.payload.artifacts) || envelope.payload.artifacts.length === 0) {
    fail("EMPTY_ARTIFACTS", "Signed manifest payload.artifacts must be a non-empty array");
  }

  const canonicalPayload = JSON.stringify(canonicalizeJson(envelope.payload));
  if (canonicalPayload !== envelope.payloadCanonicalJson) {
    fail("PAYLOAD_CANONICAL_MISMATCH", "Signed payloadCanonicalJson does not match envelope payload");
  }
  const signingInput = `v${envelope.schemaVersion}:${envelope.keyId}:${envelope.manifestKind}:${envelope.manifestSeq}:${envelope.issuedAt}:${envelope.expiresAt}:${envelope.payloadCanonicalJson}`;
  const sigBytes = Buffer.from(String(envelope.signatureBase64 ?? "").trim(), "base64");
  if (sigBytes.length !== 64) {
    fail("TAMPERED_SIGNATURE", "Ed25519 signature must decode to 64 bytes");
  }
  const keyObj = crypto.createPublicKey({
    key: buildEd25519SpkiDer(rawPub),
    format: "der",
    type: "spki",
  });
  const verifiedSig = crypto.verify(null, Buffer.from(signingInput, "utf8"), keyObj, sigBytes);
  if (!verifiedSig) {
    fail("TAMPERED_SIGNATURE", "Ed25519 signature verification failed");
  }

  const rootDir = path.resolve(artifactRootDir || path.dirname(manifestPath));
  let rootStat;
  try {
    rootStat = fs.lstatSync(rootDir);
  } catch (err) {
    fail("INVALID_ARTIFACT_ROOT", `Cannot inspect artifact root: ${err.message}`);
  }
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    fail("SYMLINK_REJECTED", "Artifact root must be a non-symlink directory");
  }

  const verifiedArtifacts = [];
  for (const entry of envelope.payload.artifacts) {
    if (
      typeof entry.byteSize !== "number" ||
      !Number.isInteger(entry.byteSize) ||
      entry.byteSize <= 0 ||
      entry.byteSize > MAX_UPDATER_BYTES
    ) {
      fail("INVALID_ARTIFACT_SIZE", `Invalid declared byteSize for ${entry.artifactId}`);
    }
    const expectedSha = String(entry.sha256 ?? "").toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(expectedSha)) {
      fail("INVALID_SHA256_FORMAT", `Malformed sha256 digest for ${entry.artifactId}`);
    }

    const targetPath = validateAndWalkRelativeArtifactPath(rootDir, entry.relativePath);
    let fd;
    try {
      fd = fs.openSync(targetPath, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0));
    } catch (err) {
      if (err && err.code === "ELOOP") {
        fail("SYMLINK_REJECTED", `Symbolic link rejected via O_NOFOLLOW for ${entry.artifactId}`);
      }
      fail("ARTIFACT_OPEN_FAILED", `Failed opening artifact ${entry.artifactId}: ${err.message}`);
    }
    try {
      const st = fs.fstatSync(fd);
      if (!st.isFile() || st.size === 0 || st.size > MAX_UPDATER_BYTES) {
        fail("INVALID_ARTIFACT_FILE", `Invalid artifact file size/type for ${entry.artifactId}`);
      }
      if (process.platform !== "win32" && st.nlink !== 1) {
        fail("HARDLINK_REJECTED", `Hardlink detected (nlink=${st.nlink}) for ${entry.artifactId}`);
      }
      const { sha256: actualSha, bytesRead } = readAndHashBoundedFd(fd, MAX_UPDATER_BYTES);
      if (bytesRead === 0 || bytesRead !== entry.byteSize) {
        fail(
          "BYTE_SIZE_MISMATCH",
          `Artifact ${entry.artifactId} read ${bytesRead} bytes, expected ${entry.byteSize}`,
        );
      }
      if (
        actualSha.length !== expectedSha.length ||
        !crypto.timingSafeEqual(Buffer.from(actualSha), Buffer.from(expectedSha))
      ) {
        fail("SHA256_MISMATCH", `SHA-256 mismatch for ${entry.artifactId}`);
      }
      verifiedArtifacts.push({
        artifactId: entry.artifactId,
        byteSize: bytesRead,
        sha256: actualSha,
        nlink: st.nlink,
      });
    } finally {
      fs.closeSync(fd);
    }
  }
  console.log(
    JSON.stringify(
      {
        ok: true,
        mode: "verify-signed",
        manifestSeq: envelope.manifestSeq,
        verifiedArtifacts,
      },
      null,
      2,
    ),
  );
}

function parseCliArgs(argv) {
  let checkManifest = false;
  let releaseMode = false;
  let sidecarBin = null;
  let verifySignedManifest = null;
  let pubkeyB64 = null;
  let rootDir = null;
  let minSeq = null;

  const requireValue = (flag, idx) => {
    const val = argv[idx + 1];
    if (val === undefined || val.length === 0) {
      fail("MISSING_FLAG_VALUE", `Flag '${flag}' requires a value`, 2);
    }
    return val;
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case "--check-manifest":
        checkManifest = true;
        break;
      case "--release":
        releaseMode = true;
        break;
      case "--sidecar-bin":
        sidecarBin = requireValue(arg, i);
        i += 1;
        break;
      case "--verify-signed":
        verifySignedManifest = requireValue(arg, i);
        i += 1;
        break;
      case "--pubkey":
        pubkeyB64 = requireValue(arg, i);
        i += 1;
        break;
      case "--root":
        rootDir = requireValue(arg, i);
        i += 1;
        break;
      case "--min-seq": {
        const raw = requireValue(arg, i);
        i += 1;
        if (!/^\d+$/.test(raw)) {
          fail("INVALID_MIN_SEQ", `--min-seq must be a non-negative integer, got '${raw}'`, 2);
        }
        minSeq = Number.parseInt(raw, 10);
        if (!Number.isSafeInteger(minSeq) || minSeq < 0) {
          fail("INVALID_MIN_SEQ", `--min-seq out of range: '${raw}'`, 2);
        }
        break;
      }
      default:
        fail("UNKNOWN_CLI_FLAG", `Unknown CLI argument: '${arg}'`, 2);
    }
  }

  if (verifySignedManifest !== null) {
    if (checkManifest || releaseMode || sidecarBin !== null) {
      fail(
        "CONFLICTING_FLAGS",
        "--verify-signed cannot be combined with --check-manifest, --release, or --sidecar-bin",
        2,
      );
    }
    if (!pubkeyB64) {
      fail(
        "MISSING_ARGS",
        "Usage: verify-desktop-bundle.mjs --verify-signed <manifest.json> --pubkey <b64> [--root <dir>] [--min-seq <n>]",
        2,
      );
    }
    return {
      mode: "verify-signed",
      verifySignedManifest,
      pubkeyB64,
      rootDir,
      minSeq,
    };
  }

  if (pubkeyB64 !== null || rootDir !== null || minSeq !== null) {
    fail(
      "CONFLICTING_FLAGS",
      "--pubkey, --root, and --min-seq are only valid with --verify-signed",
      2,
    );
  }

  return {
    mode: "check-manifest",
    sidecarBinaryPath: sidecarBin,
    releaseMode,
  };
}

const parsed = parseCliArgs(process.argv.slice(2));
if (parsed.mode === "verify-signed") {
  verifySignedMode(parsed.verifySignedManifest, parsed.pubkeyB64, parsed.rootDir, parsed.minSeq);
} else {
  checkManifestMode({
    sidecarBinaryPath: parsed.sidecarBinaryPath,
    releaseMode: parsed.releaseMode,
  });
}
