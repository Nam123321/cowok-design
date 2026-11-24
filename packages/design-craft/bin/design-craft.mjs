#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { BUILDER_TOOL_REGISTRY } from "../engine/builder-tools.mjs";
import {
  generateFeedbackTicketCode,
  scrubFeedbackFields,
  scrubFeedbackPii,
  VALID_FEEDBACK_CATEGORIES,
  VALID_FEEDBACK_CHANNELS,
  validateLoopbackDaemonUrl,
  validateOptionalFeedbackEmail,
  writeFeedbackOutboxFile,
} from "../engine/feedback-scrubber.mjs";
import { QC_TOOL_REGISTRY } from "../engine/qc-tools.mjs";
import { PathJailError, readJailedFile, writeJailedFile } from "../engine/path-jail.mjs";
import { startStdioServer } from "../mcp/server.mjs";

const BIN_DIR = path.dirname(fileURLToPath(import.meta.url));
const BUNDLE_ROOT = path.resolve(BIN_DIR, "..");
const VALID_TARGETS = ["cowok", "claude", "codex", "cursor", "antigravity", "openclaw"];
const AGENT_NAME_RE = /^[a-z0-9-]+$/;
const GITHUB_REPO_URL_RE = /^https:\/\/github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)$/;

export { scrubFeedbackPii };

export function isValidGitHubRepoUrl(url) {
  if (typeof url !== "string") return false;
  const match = GITHUB_REPO_URL_RE.exec(url);
  if (!match) return false;
  const [, owner, repo] = match;
  return owner !== "." && owner !== ".." && repo !== "." && repo !== "..";
}

export function validateOptionalPeer(peer) {
  if (!peer || typeof peer !== "object") {
    throw new Error("optionalPeers entry 'rea-upstream' is required");
  }
  if (!isValidGitHubRepoUrl(peer.repoUrl)) {
    throw new Error(`Invalid upstream GitHub repoUrl '${String(peer.repoUrl)}'`);
  }
  if (peer.installHint !== undefined && peer.installHint !== null) {
    const expected = `git clone ${peer.repoUrl}`;
    if (peer.installHint !== expected) {
      throw new Error(`Invalid installHint '${String(peer.installHint)}': expected '${expected}'`);
    }
  }
  return peer;
}

function loadManifest() {
  return JSON.parse(fs.readFileSync(path.join(BUNDLE_ROOT, "plugin.json"), "utf8"));
}

function extractBoundedFrontmatter(raw, label) {
  const clean = String(raw).replace(/^\uFEFF/, "");
  const match = clean.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  if (!match) {
    throw new Error(`Agent contract '${label}' missing bounded YAML frontmatter`);
  }
  return match[1];
}

function hasEmptyWriteScope(frontmatter) {
  const inlineMatch = frontmatter.match(/^write_scope:\s*(.*)$/m);
  if (!inlineMatch) return true;
  const rest = inlineMatch[1].trim();
  if (rest === "[]") return true;
  if (rest.length > 0) return false;
  const lines = frontmatter.split(/\r?\n/);
  const idx = lines.findIndex((line) => /^write_scope:\s*$/.test(line));
  if (idx === -1) return true;
  for (let i = idx + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^\s*-\s+/.test(line)) return false;
    if (/^\S/.test(line)) break;
  }
  return true;
}

export function loadAgentFiles(agentNames, bundleRoot = BUNDLE_ROOT) {
  if (!Array.isArray(agentNames) || agentNames.length === 0) {
    throw new Error("manifest.agents must be a non-empty array");
  }
  return agentNames.map((name) => {
    if (typeof name !== "string" || !AGENT_NAME_RE.test(name)) {
      throw new Error(`Invalid agent name '${String(name)}': must match /^[a-z0-9-]+$/`);
    }
    const filePath = path.join(bundleRoot, "agents", `${name}.md`);
    const content = fs.readFileSync(filePath, "utf8");
    const frontmatter = extractBoundedFrontmatter(content, `agents/${name}.md`);
    const writeAllowedFalse = /^product_write_allowed:\s*false\s*$/m.test(frontmatter);
    const readOnlyTrue = /^read_only:\s*true\s*$/m.test(frontmatter);
    const emptyScope = hasEmptyWriteScope(frontmatter);
    if (name === "design-auditor" && (!writeAllowedFalse || !readOnlyTrue || !emptyScope)) {
      throw new Error("design-auditor frontmatter must enforce product_write_allowed: false, read_only: true, write_scope: []");
    }
    const isReadOnly = writeAllowedFalse && readOnlyTrue;
    const roleMatch = frontmatter.match(/^role:\s*(.+)$/m);
    const role = roleMatch ? roleMatch[1].trim() : name;
    return { name, role, isReadOnly, content };
  });
}

function escapeTomlBasicString(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t");
}

export function renderCodexAgentToml(agent) {
  const escapedRole = escapeTomlBasicString(agent.role);
  const escapedBody = String(agent.content)
    .trim()
    .replace(/\\/g, "\\\\")
    .replace(/"{3,}/g, (m) => m.replace(/"/g, '\\"'))
    .replace(/"+$/, (m) => m.replace(/"/g, '\\"'));
  return [
    `name = "${agent.name}"`,
    `role = "${escapedRole}"`,
    `read_only = ${agent.isReadOnly ? "true" : "false"}`,
    `product_write_allowed = ${agent.isReadOnly ? "false" : "true"}`,
    `instructions = """`,
    escapedBody,
    `"""`,
    "",
  ].join("\n");
}

function writeOut(baseDir, relPath, content, written) {
  const posixRel = relPath.split(path.sep).join("/");
  writeJailedFile(baseDir, posixRel, content);
  written.push(posixRel);
}

function exportTargetArtifacts(target, targetDir, manifest, agents, flowYaml) {
  fs.mkdirSync(targetDir, { recursive: true });
  const written = [];
  const prefixByTarget = {
    claude: ".claude",
    codex: ".codex",
    cursor: ".cursor",
    antigravity: ".agents",
    cowok: ".cowok",
    openclaw: ".",
  };
  if (!Object.hasOwn(prefixByTarget, target)) {
    throw new Error(`Unsupported target '${target}'`);
  }
  const basePrefix = prefixByTarget[target];
  for (const agent of agents) {
    if (target === "codex") {
      writeOut(targetDir, path.posix.join(basePrefix, "agents", `${agent.name}.toml`), renderCodexAgentToml(agent), written);
    } else {
      const rel =
        target === "openclaw"
          ? path.posix.join("agents", `${agent.name}.md`)
          : path.posix.join(basePrefix, "agents", `${agent.name}.md`);
      writeOut(targetDir, rel, agent.content, written);
    }
  }
  const flowRel =
    target === "openclaw"
      ? "flows/cognitive-progressive.yaml"
      : path.posix.join(basePrefix, "flows", "cognitive-progressive.yaml");
  writeOut(targetDir, flowRel, flowYaml, written);
  if (target === "openclaw") {
    const pack = {
      schemaVersion: "1.0",
      plugin: manifest.name,
      version: manifest.version,
      mcpServer: manifest.mcpServer,
      flow: "flows/cognitive-progressive.yaml",
      agents: agents.map((a) => ({
        name: a.name,
        role: a.role,
        readOnly: a.isReadOnly,
        productWriteAllowed: !a.isReadOnly,
        file: `agents/${a.name}.md`,
      })),
      optionalPeers: manifest.optionalPeers,
    };
    writeOut(targetDir, "agent-pack.json", JSON.stringify(pack, null, 2) + "\n", written);
  } else if (target === "cursor") {
    const mdc = [
      "---",
      "description: design-craft Cognitive Progressive 5-Phase Design Rules",
      "alwaysApply: false",
      "---",
      "Follow `.cursor/flows/cognitive-progressive.yaml` and keep `design-auditor` strictly read-only.",
      "",
    ].join("\n");
    writeOut(targetDir, ".cursor/rules/design-craft.mdc", mdc, written);
  } else if (target === "cowok") {
    writeOut(targetDir, ".cowok/plugins/design-craft/plugin.json", JSON.stringify(manifest, null, 2) + "\n", written);
  } else {
    const metaRel = path.posix.join(basePrefix, "design-craft.json");
    writeOut(
      targetDir,
      metaRel,
      JSON.stringify(
        {
          plugin: manifest.name,
          version: manifest.version,
          target,
          agents: manifest.agents,
          mcpServer: manifest.mcpServer,
        },
        null,
        2
      ) + "\n",
      written
    );
  }
  return written;
}

function parseFlags(args) {
  const flags = { positional: [] };
  for (let i = 0; i < args.length; i += 1) {
    const token = args[i];
    if (token.startsWith("--")) {
      const key = token.slice(2);
      const next = args[i + 1];
      if (!next || next.startsWith("--")) {
        flags[key] = true;
      } else {
        flags[key] = next;
        i += 1;
      }
    } else {
      flags.positional.push(token);
    }
  }
  return flags;
}

export function runInstallCommand(options) {
  const target = String(options.target ?? "").toLowerCase();
  if (!VALID_TARGETS.includes(target)) {
    throw new Error(`Unsupported target '${target}'. Valid targets: ${VALID_TARGETS.join(", ")}`);
  }
  const targetDir = path.resolve(options.dir ?? process.cwd());
  const skipRea = Boolean(options.skipRea);
  const manifest = loadManifest();
  const reaPeer = validateOptionalPeer((manifest.optionalPeers ?? []).find((p) => p.id === "rea-upstream"));
  const agents = loadAgentFiles(manifest.agents);
  const flowYaml = fs.readFileSync(path.join(BUNDLE_ROOT, "flows/cognitive-progressive.yaml"), "utf8");
  const writtenFiles = exportTargetArtifacts(target, targetDir, manifest, agents, flowYaml);
  const reaRecommendation = {
    recommended: !skipRea,
    skipped: skipRea,
    peerId: reaPeer.id,
    upstreamRepoUrl: reaPeer.repoUrl,
    fallback: reaPeer.fallback ?? "catalogs/reference-archetypes.json",
    message: skipRea
      ? `Skipped optional upstream REA; Phase 2 will use offline fallback ${reaPeer.fallback ?? "catalogs/reference-archetypes.json"}.`
      : `Optional peer available: install upstream REA (${reaPeer.repoUrl}) to extract live UI behavior via Design-Only contract, or continue offline with ${reaPeer.fallback}.`,
  };
  return {
    ok: true,
    plugin: manifest.name,
    version: manifest.version,
    target,
    targetDir,
    writtenFiles,
    reaRecommendation,
  };
}

export async function runFeedbackCommand(options = {}) {
  const category = String(options.category ?? "bug_report").trim();
  if (!VALID_FEEDBACK_CATEGORIES.includes(category)) {
    throw new Error(
      `Unsupported feedback category '${category}'. Valid categories: ${VALID_FEEDBACK_CATEGORIES.join(", ")}`
    );
  }
  const channel = String(options.channel ?? "cli").trim();
  if (!VALID_FEEDBACK_CHANNELS.includes(channel)) {
    throw new Error(
      `Unsupported feedback channel '${channel}'. Valid channels: ${VALID_FEEDBACK_CHANNELS.join(", ")}`
    );
  }
  const rawTitle = String(options.title ?? "").trim();
  const rawDesc = String(options.description ?? "").trim();
  if (rawTitle.length < 3 || rawTitle.length > 160) {
    throw new Error("Feedback --title must be between 3 and 160 characters");
  }
  if (rawDesc.length < 5 || rawDesc.length > 4000) {
    throw new Error("Feedback --description must be between 5 and 4000 characters");
  }

  const emailCheck = validateOptionalFeedbackEmail(options.email);
  if (!emailCheck.ok) {
    throw new Error(emailCheck.error);
  }
  const email = emailCheck.value;

  const scrubbed = scrubFeedbackFields({
    title: rawTitle,
    description: rawDesc,
    locale: options.locale ?? "en-US",
    recentErrorCodes: Array.isArray(options.recentErrorCodes) ? options.recentErrorCodes : [],
    activeSkills: Array.isArray(options.activeSkills) ? options.activeSkills : ["design-craft"],
  });
  const redactionCount = scrubbed.redactionCount;
  const env = options.env ?? process.env;
  const airgapped = ["1", "true", "yes"].includes(String(env?.COWOK_AIRGAPPED ?? "").trim().toLowerCase());
  const rootDir = path.resolve(options.dir ?? process.cwd());

  const payload = {
    category,
    title: scrubbed.title,
    description: scrubbed.description,
    contactEmail: email ?? "",
    channel,
    locale: scrubbed.locale,
    includeDiagnostics: options.includeDiagnostics !== false,
    recentErrorCodes: scrubbed.recentErrorCodes,
    activeSkills: scrubbed.activeSkills,
  };

  const explicitDaemonRaw =
    typeof options.daemonUrl === "string" && options.daemonUrl.trim()
      ? options.daemonUrl.trim()
      : String(env?.COWOK_DAEMON_URL ?? "").trim();
  let daemonBase = explicitDaemonRaw ? validateLoopbackDaemonUrl(explicitDaemonRaw) : "";

  if (!daemonBase && !airgapped && fs.existsSync(rootDir)) {
    try {
      const lockRaw = readJailedFile(rootDir, ".cowok-design/daemon.lock");
      const lock = JSON.parse(lockRaw);
      if (lock.host === "127.0.0.1" && Number.isInteger(lock.port) && lock.port >= 1 && lock.port <= 65535) {
        daemonBase = `http://127.0.0.1:${lock.port}`;
      }
    } catch (err) {
      if (err instanceof PathJailError) {
        throw err;
      }
      // Fall back to local outbox when lockfile is absent or unreadable.
    }
  }

  if (daemonBase && !airgapped && typeof (options.fetchImpl ?? globalThis.fetch) === "function") {
    const doFetch = options.fetchImpl ?? globalThis.fetch;
    let res;
    try {
      const headers = { "Content-Type": "application/json", Accept: "application/json" };
      const sessTok = options.sessionToken ?? env?.COWOK_SESSION_TOKEN;
      if (sessTok) {
        headers["X-Cowok-Session-Token"] = String(sessTok);
      }
      res = await doFetch(`${daemonBase}/api/v1/support/tickets`, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
      });
    } catch {
      res = null;
    }

    if (res) {
      if (res.ok) {
        const body = await res.json();
        return {
          ok: true,
          mode: "daemon",
          scrubbedRedactions: redactionCount,
          ticket: body?.data ?? body,
        };
      }
      if (res.status >= 400 && res.status < 500) {
        let detail = `HTTP ${res.status}`;
        try {
          const errBody = await res.json();
          detail = errBody?.error?.message ?? errBody?.message ?? detail;
        } catch {
          // Use status fallback if body is non-JSON.
        }
        throw new Error(`Daemon rejected feedback (${res.status}): ${detail}`);
      }
      // 5xx falls through to local scrubbed outbox.
    }
  }

  const ticketCode = generateFeedbackTicketCode();
  const offlineTicket = {
    ticketCode,
    category,
    channel,
    title: scrubbed.title,
    description: scrubbed.description,
    contactEmail: email,
    status: "submitted",
    syncStatus: airgapped ? "local_only" : "queued",
    scrubbedRedactions: redactionCount,
    diagnostics: {
      appVersion: "1.0.0",
      channel,
      os: os.platform(),
      locale: scrubbed.locale,
      recentErrorCodes: scrubbed.recentErrorCodes,
      activeSkills: scrubbed.activeSkills,
      redactionCount,
    },
    createdAt: Date.now(),
  };

  if (!options.noWrite && fs.existsSync(rootDir)) {
    writeFeedbackOutboxFile(rootDir, ticketCode, offlineTicket);
  }

  return {
    ok: true,
    mode: "local_outbox",
    scrubbedRedactions: redactionCount,
    ticket: offlineTicket,
  };
}

async function main() {
  const [, , command, ...rest] = process.argv;
  const flags = parseFlags(rest);
  if (command === "mcp") {
    startStdioServer();
    return;
  }
  if (command === "list-tools") {
    const tools = [...Object.keys(BUILDER_TOOL_REGISTRY), ...Object.keys(QC_TOOL_REGISTRY), "cowok_submit_feedback"];
    process.stdout.write(`${JSON.stringify({ tools }, null, 2)}\n`);
    return;
  }
  if (command === "feedback") {
    const receipt = await runFeedbackCommand({
      category: flags.category,
      title: flags.title,
      description: flags.description,
      email: flags.email,
      channel: flags.channel,
      locale: flags.locale,
      dir: flags.dir,
      daemonUrl: flags["daemon-url"],
      includeDiagnostics: flags["no-diagnostics"] ? false : true,
    });
    if (flags.json) {
      process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
    } else {
      const code = receipt.ticket?.ticketCode ?? "CWK-LOCAL";
      process.stdout.write(`Submitted feedback ${code} via ${receipt.mode} (redacted ${receipt.scrubbedRedactions} secret/path items)\n`);
    }
    return;
  }
  if (command === "install") {
    const receipt = runInstallCommand({
      target: flags.target,
      dir: flags.dir,
      skipRea: Boolean(flags["skip-rea"]),
    });
    if (flags.json) {
      process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
    } else {
      process.stdout.write(`Installed design-craft (${receipt.version}) for ${receipt.target} (${receipt.writtenFiles.length} files)\n`);
      process.stdout.write(`${receipt.reaRecommendation.message}\n`);
    }
    return;
  }
  if (command === "run") {
    const toolName = flags.positional[0];
    const allTools = { ...BUILDER_TOOL_REGISTRY, ...QC_TOOL_REGISTRY };
    if (typeof toolName !== "string" || !Object.hasOwn(allTools, toolName)) {
      process.stderr.write(`Unknown tool '${toolName ?? ""}'\n`);
      process.exit(1);
    }
    const input = flags.input ? JSON.parse(String(flags.input)) : {};
    const result = allTools[toolName](input, { rootDir: flags.root ?? process.cwd(), env: process.env });
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stderr.write("Usage: design-craft <install|run|mcp|list-tools|feedback> [options]\n");
  process.exit(1);
}

function isDirectInvocation() {
  if (!process.argv[1]) return false;
  try {
    const argvUrl = pathToFileURL(fs.realpathSync(path.resolve(process.argv[1]))).href;
    const selfUrl = pathToFileURL(fs.realpathSync(fileURLToPath(import.meta.url))).href;
    return argvUrl === selfUrl;
  } catch {
    return false;
  }
}

if (isDirectInvocation()) {
  void main().catch((err) => {
    process.stderr.write(`${err instanceof Error ? err.message : String(err)}\n`);
    process.exit(1);
  });
}
