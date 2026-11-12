import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { ensureRunningDaemon } from "./src/lib/daemon-client";

let tempWsRoot = "";

export async function setup(): Promise<void> {
  const repoRoot = process.cwd();
  const daemonBin = path.resolve(repoRoot, "target", "debug", "harness-daemon");
  execFileSync("cargo", ["build", "--bin", "harness-daemon", "--quiet"], {
    cwd: repoRoot,
    stdio: "inherit",
  });
  const rawTmp = fs.mkdtempSync(path.join(os.tmpdir(), "cwk-vitest-"));
  tempWsRoot = fs.realpathSync(rawTmp);
  process.env.UXUI_OS_DAEMON_BIN = daemonBin;
  process.env.UXUI_OS_WORKSPACE_ROOT = tempWsRoot;
  await ensureRunningDaemon();
}

export async function teardown(): Promise<void> {
  if (!tempWsRoot) return;
  try {
    const lockPath = path.join(tempWsRoot, ".cowok-design", "daemon.lock");
    if (fs.existsSync(lockPath)) {
      const lock = JSON.parse(fs.readFileSync(lockPath, "utf8")) as { pid?: number };
      if (lock.pid && lock.pid > 0) {
        try {
          process.kill(lock.pid, "SIGTERM");
        } catch {
          // already terminated
        }
      }
    }
    fs.rmSync(tempWsRoot, { recursive: true, force: true });
  } catch {
    // ignore cleanup failures
  }
}
