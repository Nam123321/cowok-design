// @story 04.2
// @implements AC-02 (Real CLI Doctor & Auth/Session Verification Probe)
import { execFile } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { guardedRoute } from "../../../lib/api-guard";

const execFileAsync = promisify(execFile);

export interface CliDiagnosticItem {
  id: string;
  name: string;
  command: string;
  installed: boolean;
  version?: string;
  authenticated: boolean;
  status: "ready" | "not_installed" | "auth_needed" | "error";
  authReason?: string;
  latencyMs: number;
}

interface CliTargetSpec {
  id: string;
  name: string;
  command: string;
  versionFlag: string;
  envKeys: string[];
  configPaths: string[];
}

const HOME = os.homedir();

const SUPPORTED_CLIS: CliTargetSpec[] = [
  {
    id: "claude",
    name: "Claude Code",
    command: "claude",
    versionFlag: "--version",
    envKeys: ["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"],
    configPaths: [path.join(HOME, ".claude", ".credentials.json"), path.join(HOME, ".claude.json")],
  },
  {
    id: "codex",
    name: "OpenAI Codex CLI",
    command: "codex",
    versionFlag: "--version",
    envKeys: ["OPENAI_API_KEY", "CODEX_API_KEY"],
    configPaths: [path.join(HOME, ".codex", "auth.json")],
  },
  {
    id: "agy",
    name: "Google Antigravity CLI",
    command: "agy",
    versionFlag: "--version",
    envKeys: ["GEMINI_API_KEY", "GOOGLE_API_KEY", "ANTIGRAVITY_API_KEY"],
    configPaths: [path.join(HOME, ".gemini", "oauth_creds.json")],
  },
  {
    id: "gemini",
    name: "Gemini CLI",
    command: "gemini",
    versionFlag: "--version",
    envKeys: ["GEMINI_API_KEY", "GOOGLE_API_KEY"],
    configPaths: [path.join(HOME, ".gemini", "oauth_creds.json"), path.join(HOME, ".config", "gemini")],
  },
  {
    id: "cursor",
    name: "Cursor Agent CLI",
    command: "cursor",
    versionFlag: "--version",
    envKeys: ["CURSOR_API_KEY", "OPENAI_API_KEY"],
    configPaths: [path.join(HOME, ".cursor", "cli-config.json")],
  },
  {
    id: "opencode",
    name: "OpenCode CLI",
    command: "opencode",
    versionFlag: "--version",
    envKeys: ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OPENCODE_API_KEY"],
    configPaths: [path.join(HOME, ".local", "share", "opencode", "auth.json")],
  },
  {
    id: "omp",
    name: "Oh My Prompt CLI",
    command: "omp",
    versionFlag: "--version",
    envKeys: ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY"],
    configPaths: [path.join(HOME, ".omp", "config.json")],
  },
  {
    id: "ollama",
    name: "Ollama Local LLM",
    command: "ollama",
    versionFlag: "--version",
    envKeys: ["OLLAMA_API_KEY"],
    configPaths: [],
  },
];

async function checkCliAuthentication(
  target: CliTargetSpec
): Promise<{ authenticated: boolean; reason?: string }> {
  for (const key of target.envKeys) {
    if (process.env[key]?.trim()) {
      return { authenticated: true };
    }
  }
  for (const cfgPath of target.configPaths) {
    try {
      if (fs.existsSync(cfgPath)) {
        const stat = fs.statSync(cfgPath);
        if (stat.isDirectory() || stat.size > 2) {
          return { authenticated: true };
        }
      }
    } catch {
      // Ignore unreadable config file and continue checking
    }
  }
  if (target.id === "ollama") {
    const base = (process.env.OLLAMA_HOST || "http://127.0.0.1:11434").replace(/\/$/, "");
    try {
      const resp = await fetch(`${base}/api/tags`, { signal: AbortSignal.timeout(750) });
      if (resp.ok) {
        return { authenticated: true };
      }
    } catch {
      return { authenticated: false, reason: "Ollama daemon is not running on 127.0.0.1:11434" };
    }
  }
  const expectedEnv = target.envKeys[0] || "API_KEY";
  return {
    authenticated: false,
    reason: `Missing ${expectedEnv} or CLI login session`,
  };
}

async function probeCli(target: CliTargetSpec): Promise<CliDiagnosticItem> {
  const start = performance.now();
  try {
    const { stdout } = await execFileAsync("which", [target.command], { timeout: 1200 });
    const binaryPath = stdout.trim();
    if (!binaryPath) {
      return {
        id: target.id,
        name: target.name,
        command: target.command,
        installed: false,
        authenticated: false,
        status: "not_installed",
        latencyMs: Math.round(performance.now() - start),
      };
    }

    let versionStr: string | undefined;
    try {
      const verRes = await execFileAsync(target.command, [target.versionFlag], { timeout: 1200 });
      versionStr = (verRes.stdout || verRes.stderr).trim().split("\n")[0];
    } catch {
      versionStr = "detected";
    }

    const auth = await checkCliAuthentication(target);
    return {
      id: target.id,
      name: target.name,
      command: target.command,
      installed: true,
      version: versionStr,
      authenticated: auth.authenticated,
      status: auth.authenticated ? "ready" : "auth_needed",
      authReason: auth.reason,
      latencyMs: Math.round(performance.now() - start),
    };
  } catch {
    return {
      id: target.id,
      name: target.name,
      command: target.command,
      installed: false,
      authenticated: false,
      status: "not_installed",
      latencyMs: Math.round(performance.now() - start),
    };
  }
}

export const GET = guardedRoute(
  async () => {
    const results = await Promise.all(SUPPORTED_CLIS.map((cli) => probeCli(cli)));
    return new Response(
      JSON.stringify({ clis: results, scannedAt: new Date().toISOString() }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }
    );
  },
  { authMode: "strict-loopback" }
);
