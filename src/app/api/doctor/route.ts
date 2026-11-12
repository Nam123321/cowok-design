// @story 04.2
// @implements AC-02 (CLI Doctor probe)
import { execFile } from "node:child_process";
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
  latencyMs: number;
}

const SUPPORTED_CLIS = [
  { id: "claude", name: "Claude Code", command: "claude", versionFlag: "--version" },
  { id: "codex", name: "OpenAI Codex CLI", command: "codex", versionFlag: "--version" },
  { id: "agy", name: "Google Antigravity CLI", command: "agy", versionFlag: "--version" },
  { id: "gemini", name: "Gemini CLI", command: "gemini", versionFlag: "--version" },
  { id: "cursor", name: "Cursor Agent CLI", command: "cursor", versionFlag: "--version" },
  { id: "opencode", name: "OpenCode CLI", command: "opencode", versionFlag: "--version" },
  { id: "omp", name: "Oh My Prompt CLI", command: "omp", versionFlag: "--version" },
  { id: "ollama", name: "Ollama Local LLM", command: "ollama", versionFlag: "--version" },
];

async function probeCli(target: typeof SUPPORTED_CLIS[0]): Promise<CliDiagnosticItem> {
  const start = performance.now();
  try {
    const { stdout, stderr } = await execFileAsync("which", [target.command], { timeout: 1500 });
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
      const verRes = await execFileAsync(target.command, [target.versionFlag], { timeout: 1500 });
      versionStr = (verRes.stdout || verRes.stderr).trim().split("\n")[0];
    } catch {
      versionStr = "detected";
    }

    return {
      id: target.id,
      name: target.name,
      command: target.command,
      installed: true,
      version: versionStr,
      authenticated: true, // Installed binary is ready for local invocations
      status: "ready",
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
    const probePromises = SUPPORTED_CLIS.map((cli) => probeCli(cli));
    const results = await Promise.all(probePromises);
    return new Response(JSON.stringify({ clis: results, scannedAt: new Date().toISOString() }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  },
  { authMode: "strict-loopback" }
);
