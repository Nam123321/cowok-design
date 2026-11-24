import crypto from "node:crypto";
import fs from "node:fs";
import { writeJailedFile } from "./path-jail.mjs";

export const VALID_FEEDBACK_CATEGORIES = [
  "bug_report",
  "feature_request",
  "skill_template_request",
  "question",
];

export const VALID_FEEDBACK_CHANNELS = [
  "cli",
  "desktop",
  "claude_code",
  "codex",
  "antigravity",
  "cursor",
  "cowok_ai",
  "docker",
];

const MAX_RECENT_ERROR_CODES = 20;
const MAX_ACTIVE_SKILLS = 30;
const MAX_DIAG_TOKEN_LEN = 64;
const MIN_LOCALE_LEN = 2;
const MAX_LOCALE_LEN = 16;
const MAX_EMAIL_LEN = 254;
const OUTBOX_FILE_MODE = 0o600;
const WHITESPACE_OR_CONTROL_RE = /[\s\x00-\x1f\x7f]/;

export function scrubFeedbackPii(input) {
  let text = String(input ?? "");
  let redactionCount = 0;
  const replaceWithCount = (re, replacement) => {
    text = text.replace(re, (...args) => {
      redactionCount += 1;
      return typeof replacement === "function" ? replacement(...args) : replacement;
    });
  };
  replaceWithCount(/(?<![A-Za-z0-9])(Set-Cookie|Cookie):[^\r\n]+/gi, (_, h) => `${h}: [REDACTED_SECRET]`);
  replaceWithCount(/(?<![A-Za-z0-9])Bearer[ \t]+[A-Za-z0-9_.\-+/=]{8,}/gi, "Bearer [REDACTED_SECRET]");
  replaceWithCount(/(?<![A-Za-z0-9])eyJ[A-Za-z0-9_-]{6,}\.eyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}(?![A-Za-z0-9_-])/g, "[REDACTED_SECRET]");
  replaceWithCount(
    /(?<![A-Za-z0-9])(?:sk-[A-Za-z0-9_-]{8,}|AIza[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9_-]{12,}|xox[baprs]-[A-Za-z0-9_-]{10,}|AKIA[A-Z0-9]{16})(?![A-Za-z0-9_-])/g,
    "[REDACTED_SECRET]"
  );
  replaceWithCount(/\/users\/[A-Za-z0-9_.-]+/gi, "<HOME>");
  replaceWithCount(/\/home\/[A-Za-z0-9_.-]+/gi, "<HOME>");
  replaceWithCount(/(?<![A-Za-z0-9])[A-Za-z]:[\\/]users[\\/][A-Za-z0-9_.-]+/gi, "<HOME>");
  return { scrubbed: text, redactionCount };
}

export function validateOptionalFeedbackEmail(rawEmail) {
  if (rawEmail === undefined || rawEmail === null) {
    return { ok: true, value: undefined };
  }
  if (typeof rawEmail !== "string") {
    return { ok: false, error: "INVALID_EMAIL: contactEmail must be a string" };
  }
  const trimmed = rawEmail.trim();
  if (trimmed.length === 0) {
    return { ok: true, value: undefined };
  }
  if (trimmed.length > MAX_EMAIL_LEN || WHITESPACE_OR_CONTROL_RE.test(trimmed)) {
    return { ok: false, error: "INVALID_EMAIL: contactEmail must be a valid email address" };
  }
  const parts = trimmed.split("@");
  if (
    parts.length !== 2 ||
    !parts[0] ||
    parts[0].includes("..") ||
    !parts[1] ||
    !parts[1].includes(".") ||
    parts[1].startsWith(".") ||
    parts[1].endsWith(".") ||
    parts[1].includes("..") ||
    parts[1].split(".").some((seg) => seg.length === 0)
  ) {
    return { ok: false, error: "INVALID_EMAIL: contactEmail domain is invalid" };
  }
  return { ok: true, value: trimmed };
}

export function validateLoopbackDaemonUrl(rawUrl) {
  const trimmed = String(rawUrl ?? "").trim();
  if (!trimmed) return "";
  let parsed;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw new Error("INVALID_DAEMON_URL: daemonUrl must be a valid loopback http://(127.0.0.1|localhost):<port> URL");
  }
  if (parsed.protocol !== "http:") {
    throw new Error("INVALID_DAEMON_URL: daemonUrl must use http:// on loopback only");
  }
  if (parsed.hostname !== "127.0.0.1" && parsed.hostname !== "localhost") {
    throw new Error("INVALID_DAEMON_URL: daemonUrl host must be strictly 127.0.0.1 or localhost");
  }
  if (parsed.username || parsed.password) {
    throw new Error("INVALID_DAEMON_URL: daemonUrl must not include credentials");
  }
  if ((parsed.pathname && parsed.pathname !== "/") || parsed.search || parsed.hash) {
    throw new Error("INVALID_DAEMON_URL: daemonUrl must not include path, query, or fragment");
  }
  const portNum = Number.parseInt(parsed.port, 10);
  if (!parsed.port || !Number.isInteger(portNum) || portNum < 1 || portNum > 65535) {
    throw new Error("INVALID_DAEMON_URL: daemonUrl must specify an explicit valid port (1..65535)");
  }
  return `http://${parsed.hostname}:${portNum}`;
}

export function scrubFeedbackFields(fields = {}) {
  const cleanTitle = scrubFeedbackPii(fields.title ?? "");
  const cleanDesc = scrubFeedbackPii(fields.description ?? "");
  const cleanLocaleRaw = scrubFeedbackPii(String(fields.locale ?? "en-US").trim());
  let redactionCount =
    cleanTitle.redactionCount + cleanDesc.redactionCount + cleanLocaleRaw.redactionCount;

  const trimmedLocale = cleanLocaleRaw.scrubbed.trim().slice(0, MAX_LOCALE_LEN);
  const locale =
    trimmedLocale.length >= MIN_LOCALE_LEN && !WHITESPACE_OR_CONTROL_RE.test(trimmedLocale)
      ? trimmedLocale
      : "en-US";

  const rawErrors = Array.isArray(fields.recentErrorCodes)
    ? fields.recentErrorCodes.slice(0, MAX_RECENT_ERROR_CODES)
    : [];
  const rawSkills = Array.isArray(fields.activeSkills)
    ? fields.activeSkills.slice(0, MAX_ACTIVE_SKILLS)
    : [];

  const recentErrorCodes = [];
  for (const item of rawErrors) {
    const trimmed = String(item ?? "").trim();
    if (!trimmed) continue;
    const scrubbed = scrubFeedbackPii(trimmed);
    redactionCount += scrubbed.redactionCount;
    recentErrorCodes.push(scrubbed.scrubbed.slice(0, MAX_DIAG_TOKEN_LEN));
  }

  const activeSkills = [];
  for (const item of rawSkills) {
    const trimmed = String(item ?? "").trim();
    if (!trimmed) continue;
    const scrubbed = scrubFeedbackPii(trimmed);
    redactionCount += scrubbed.redactionCount;
    activeSkills.push(scrubbed.scrubbed.slice(0, MAX_DIAG_TOKEN_LEN));
  }

  return {
    title: cleanTitle.scrubbed,
    description: cleanDesc.scrubbed,
    locale,
    recentErrorCodes,
    activeSkills,
    redactionCount,
  };
}

export function generateFeedbackTicketCode() {
  return `CWK-${crypto.randomBytes(4).toString("hex").toUpperCase()}`;
}

export function writeFeedbackOutboxFile(rootDir, ticketCode, record) {
  const safePath = writeJailedFile(
    rootDir,
    `.cowok-design/support-outbox/${ticketCode}.json`,
    `${JSON.stringify(record, null, 2)}\n`
  );
  const stat = fs.lstatSync(safePath);
  if (!stat.isSymbolicLink()) {
    try {
      fs.chmodSync(safePath, OUTBOX_FILE_MODE);
    } catch {
      // Best-effort POSIX permission tightening on non-POSIX filesystems.
    }
  }
  return safePath;
}
