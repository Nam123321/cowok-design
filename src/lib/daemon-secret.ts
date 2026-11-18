// @story 04.1, 08.4
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const WORKBENCH_HUMAN_SECRET_SYMBOL = Symbol.for("cowok.workbenchHumanSecret");
const WORKBENCH_SECRET_BYTES = 32;
const WORKBENCH_SECRET_FILE_MODE = 0o600;
const WORKBENCH_DIR_MODE = 0o700;

function resolveWorkbenchSecretPath(workspaceRoot: string): string {
  const cowokDir = path.join(workspaceRoot, ".cowok-design");
  const stateRoot =
    process.env.COWOK_WORKBENCH_SECRET_DIR?.trim() ||
    process.env.XDG_STATE_HOME?.trim() ||
    (os.homedir() ? path.join(os.homedir(), ".local", "state") : path.join(cowokDir, "state-fallback"));
  const wsHash = crypto.createHash("sha256").update(cowokDir, "utf8").digest("hex");
  return path.join(stateRoot, "cowok-design", "workbench-secrets", `${wsHash}.secret`);
}

function readVerifiedSecretFile(secretPath: string): string | null {
  const noFollow = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(secretPath, noFollow);
  try {
    const stat = fs.fstatSync(fd);
    if (!stat.isFile() || stat.nlink !== 1) return null;
    const persisted = fs.readFileSync(fd, "utf8").trim();
    return persisted.length >= WORKBENCH_SECRET_BYTES ? persisted : null;
  } finally {
    fs.closeSync(fd);
  }
}

function writeSecretFileExclusive(secretPath: string, generated: string): string {
  fs.mkdirSync(path.dirname(secretPath), { recursive: true, mode: WORKBENCH_DIR_MODE });
  try {
    const flags = fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL | (fs.constants.O_NOFOLLOW ?? 0);
    const fd = fs.openSync(secretPath, flags, WORKBENCH_SECRET_FILE_MODE);
    try {
      fs.writeFileSync(fd, generated, "utf8");
      fs.fchmodSync(fd, WORKBENCH_SECRET_FILE_MODE);
    } finally {
      fs.closeSync(fd);
    }
    return generated;
  } catch {
    return readVerifiedSecretFile(secretPath) ?? generated;
  }
}

export function getOrInitWorkbenchHumanSecret(workspaceRoot: string): string {
  const existing = process.env.COWOK_WORKBENCH_HUMAN_SECRET?.trim();
  if (existing) return existing;
  const secretPath = resolveWorkbenchSecretPath(workspaceRoot);
  try {
    const persisted = readVerifiedSecretFile(secretPath);
    if (persisted) {
      process.env.COWOK_WORKBENCH_HUMAN_SECRET = persisted;
      return persisted;
    }
  } catch {
    // Fall through to initialization below
  }
  const g = globalThis as Record<PropertyKey, unknown>;
  let generated =
    typeof g[WORKBENCH_HUMAN_SECRET_SYMBOL] === "string" &&
    (g[WORKBENCH_HUMAN_SECRET_SYMBOL] as string).length >= WORKBENCH_SECRET_BYTES
      ? (g[WORKBENCH_HUMAN_SECRET_SYMBOL] as string)
      : crypto.randomBytes(WORKBENCH_SECRET_BYTES).toString("hex");
  try {
    generated = writeSecretFileExclusive(secretPath, generated);
  } catch (err) {
    const lockPath = path.join(workspaceRoot, ".cowok-design", "daemon.lock");
    if (fs.existsSync(lockPath)) {
      const reason = err instanceof Error ? err.message : String(err);
      throw new Error(`HUMAN_AUTH_MISCONFIGURED: Cannot persist workbench secret while daemon.lock is active (${reason})`);
    }
  }
  g[WORKBENCH_HUMAN_SECRET_SYMBOL] = generated;
  process.env.COWOK_WORKBENCH_HUMAN_SECRET = generated;
  return generated;
}
