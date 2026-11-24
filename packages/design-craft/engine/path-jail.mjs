import fs from "node:fs";
import path from "node:path";

const SHELL_META_PATTERN = /[;|&`$()<>\r\n]/;
const CONTROL_CHAR_PATTERN = /[\x00-\x1f\x7f]/;
const NETWORK_URL_PATTERN = /^(https?|wss?|ftp):\/\//i;

export class PathJailError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "PathJailError";
    this.code = code;
  }
}

export function assertNoShellInjection(raw, fieldName = "input") {
  if (typeof raw !== "string") {
    throw new PathJailError("INVALID_TYPE", `${fieldName} must be a string`);
  }
  if (raw.includes("\0") || CONTROL_CHAR_PATTERN.test(raw)) {
    throw new PathJailError("CONTROL_CHAR", `${fieldName} contains NUL or control characters`);
  }
  if (SHELL_META_PATTERN.test(raw)) {
    throw new PathJailError("SHELL_METACHAR", `${fieldName} contains forbidden shell metacharacters`);
  }
  return raw;
}

export function assertNoNetworkEgress(candidate, fieldName = "target") {
  if (typeof candidate === "string" && NETWORK_URL_PATTERN.test(candidate.trim())) {
    throw new PathJailError(
      "NETWORK_EGRESS_BLOCKED",
      `${fieldName} cannot trigger live network egress in portable offline engine`
    );
  }
}

function isWithinRoot(realRoot, realCandidate) {
  const rel = path.relative(realRoot, realCandidate);
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

function checkSegmentSymlinks(realRoot, relPath) {
  const parts = relPath.split("/").filter(Boolean);
  let cursor = realRoot;
  for (const part of parts) {
    if (part === "." || part === "..") {
      throw new PathJailError("TRAVERSAL_BLOCKED", `Path traversal segment '${part}' is forbidden`);
    }
    cursor = path.join(cursor, part);
    let stat;
    try {
      stat = fs.lstatSync(cursor);
    } catch (err) {
      if (err && err.code === "ENOENT") {
        break;
      }
      throw err;
    }
    if (stat.isSymbolicLink()) {
      let resolved;
      try {
        resolved = fs.realpathSync(cursor);
      } catch {
        throw new PathJailError(
          "SYMLINK_BLOCKED",
          `Dangling symlink component rejected by O_NOFOLLOW policy: '${part}'`
        );
      }
      if (!isWithinRoot(realRoot, resolved)) {
        throw new PathJailError("SYMLINK_ESCAPE", `Symlink '${relPath}' escapes jail root`);
      }
      throw new PathJailError("SYMLINK_BLOCKED", `Symlink component rejected by O_NOFOLLOW policy: '${part}'`);
    }
  }
}

function resolveDeepestRealPath(targetPath) {
  let cursor = targetPath;
  const suffix = [];
  for (;;) {
    try {
      fs.lstatSync(cursor);
      break;
    } catch (err) {
      if (!err || err.code !== "ENOENT") {
        throw err;
      }
      const parent = path.dirname(cursor);
      if (parent === cursor) break;
      suffix.unshift(path.basename(cursor));
      cursor = parent;
    }
  }
  const realBase = fs.realpathSync(cursor);
  return suffix.length > 0 ? path.join(realBase, ...suffix) : realBase;
}

export function assertSafePath(rootDir, targetPath) {
  if (!rootDir || typeof rootDir !== "string") {
    throw new PathJailError("INVALID_ROOT", "Jail root directory is required");
  }
  assertNoShellInjection(targetPath, "path");
  assertNoNetworkEgress(targetPath, "path");
  const trimmed = targetPath.trim();
  if (!trimmed) {
    throw new PathJailError("EMPTY_PATH", "Target path must not be empty");
  }
  if (trimmed.includes("\\")) {
    throw new PathJailError("BACKSLASH_BLOCKED", `Backslash separator rejected in '${targetPath}'`);
  }
  if (/^[A-Za-z]:/.test(trimmed)) {
    throw new PathJailError("DRIVE_PATH_BLOCKED", `Drive-letter path rejected in '${targetPath}'`);
  }
  if (trimmed.startsWith("//")) {
    throw new PathJailError("UNC_PATH_BLOCKED", `UNC path rejected in '${targetPath}'`);
  }
  const realRoot = fs.realpathSync(path.resolve(rootDir));
  const rawSegments = trimmed.split("/");
  if (rawSegments.includes("..")) {
    throw new PathJailError("TRAVERSAL_BLOCKED", `Path traversal '..' rejected in '${targetPath}'`);
  }
  const resolvedCandidate = path.isAbsolute(trimmed)
    ? path.resolve(trimmed)
    : path.resolve(realRoot, trimmed);
  if (!isWithinRoot(realRoot, resolvedCandidate)) {
    throw new PathJailError("ABSOLUTE_ESCAPE", `Path '${targetPath}' escapes jail root`);
  }
  const relativeFromRoot = path.relative(realRoot, resolvedCandidate).split(path.sep).join("/");
  if (relativeFromRoot) {
    checkSegmentSymlinks(realRoot, relativeFromRoot);
  }
  const realTarget = resolveDeepestRealPath(resolvedCandidate);
  if (!isWithinRoot(realRoot, realTarget)) {
    throw new PathJailError("REALPATH_ESCAPE", `Resolved path for '${targetPath}' escapes jail root`);
  }
  return realTarget;
}

export function readJailedFile(rootDir, targetPath) {
  const safePath = assertSafePath(rootDir, targetPath);
  const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(safePath, flags);
  try {
    return fs.readFileSync(fd, "utf8");
  } finally {
    fs.closeSync(fd);
  }
}

export function writeJailedFile(rootDir, targetPath, content) {
  const safePath = assertSafePath(rootDir, targetPath);
  fs.mkdirSync(path.dirname(safePath), { recursive: true });
  const flags =
    fs.constants.O_WRONLY |
    fs.constants.O_CREAT |
    fs.constants.O_TRUNC |
    (fs.constants.O_NOFOLLOW ?? 0);
  const fd = fs.openSync(safePath, flags, 0o644);
  try {
    fs.writeSync(fd, content, 0, "utf8");
  } finally {
    fs.closeSync(fd);
  }
  return safePath;
}
