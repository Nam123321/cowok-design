// @story 08.4
import { z } from "zod";

export type AssertExactKeys<A, B> = [
  Exclude<keyof A, keyof B>,
  Exclude<keyof B, keyof A>,
] extends [never, never]
  ? true
  : never;

const MAX_URL_LENGTH = 1024;
const BLOCKED_HOSTS = new Set(["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]", "::", "[::]", "169.254.169.254", "metadata.google.internal"]);
const IPV4_PART_COUNT = 4;
const IPV4_OCTET_MAX = 255;
const OCTET_10 = 10;
const OCTET_100 = 100;
const OCTET_100_MIN_SECOND = 64;
const OCTET_100_MAX_SECOND = 127;
const OCTET_127 = 127;
const OCTET_169 = 169;
const OCTET_169_SECOND = 254;
const OCTET_172 = 172;
const OCTET_172_MIN_SECOND = 16;
const OCTET_172_MAX_SECOND = 31;
const OCTET_192 = 192;
const OCTET_192_SECOND = 168;
const OCTET_MULTICAST_MIN = 224;
const DANGEROUS_URI_SCHEME_RE = /^\s*(javascript|vbscript|data|file):/i;

function isReservedOctetPair(a: number, b: number): boolean {
  if (a === 0 || a === OCTET_10 || a === OCTET_127 || a >= OCTET_MULTICAST_MIN) return true;
  if (a === OCTET_100) return b >= OCTET_100_MIN_SECOND && b <= OCTET_100_MAX_SECOND;
  if (a === OCTET_169) return b === OCTET_169_SECOND;
  if (a === OCTET_172) return b >= OCTET_172_MIN_SECOND && b <= OCTET_172_MAX_SECOND;
  return a === OCTET_192 && b === OCTET_192_SECOND;
}

function isPrivateOrReservedIpv4(host: string): boolean {
  const parts = host.split(".");
  if (parts.length !== IPV4_PART_COUNT || parts.some((p) => !/^\d{1,3}$/.test(p))) {
    return /^(0x[0-9a-f]+|0[0-7]+|\d+)$/i.test(host);
  }
  const nums = parts.map((p) => Number(p));
  if (nums.some((n) => n < 0 || n > IPV4_OCTET_MAX)) return true;
  return isReservedOctetPair(nums[0], nums[1]);
}

function isBlockedIpv6Host(host: string): boolean {
  if (
    host === "::" ||
    host === "::1" ||
    host === "0:0:0:0:0:0:0:0" ||
    host === "0:0:0:0:0:0:0:1" ||
    host.startsWith("::ffff:")
  ) {
    return true;
  }
  return (
    /^fe[89ab][0-9a-f]?:/i.test(host) ||
    host.startsWith("fc") ||
    host.startsWith("fd") ||
    host.startsWith("ff") ||
    host.startsWith("2001:0:") ||
    host.startsWith("2001:0000:") ||
    host.startsWith("2002:") ||
    host.startsWith("64:ff9b:") ||
    host.startsWith("100::")
  );
}

function isBlockedHostname(rawHost: string): boolean {
  const host = rawHost.replace(/^\[|\]$/g, "").toLowerCase();
  if (!host || BLOCKED_HOSTS.has(host) || host.endsWith(".localhost") || host.endsWith(".local") || host.endsWith(".internal")) {
    return true;
  }
  if (isPrivateOrReservedIpv4(host)) return true;
  if (host.includes(":")) return isBlockedIpv6Host(host);
  return false;
}

export function utf8BytesMax(maxBytes: number, fieldName: string) {
  return z.string().min(1).refine(
    (v) => new TextEncoder().encode(v).length <= maxBytes,
    { message: `${fieldName} exceeds maximum ${maxBytes} UTF-8 bytes` }
  );
}

export function safeRelativePath(maxLen: number) {
  return z
    .string()
    .min(1)
    .max(maxLen)
    .refine(
      (p) => !p.startsWith("/") && !p.includes("\\") && !p.includes("\0") && !p.split("/").some((s) => !s || s === "." || s === ".."),
      { message: "Path must be a clean relative path without traversal segments" }
    );
}

/** Validates that a remote URL uses https:// with no credentials and targets a public host. */
export function isPublicHttpsUrl(raw: string): boolean {
  if (/[\s\x00-\x1f\x7f-\x9f]/.test(raw)) return false;
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== "https:" || parsed.username || parsed.password) return false;
    return !isBlockedHostname(parsed.hostname);
  } catch {
    return false;
  }
}

export function safeRemoteHttpUrl(maxLen = MAX_URL_LENGTH) {
  return z
    .string()
    .url()
    .max(maxLen)
    .refine(isPublicHttpsUrl, {
      message: "URL must use https:// without credentials and must not target private, loopback, or metadata addresses",
    });
}

/** Validates non-github sourceRepoUrl values, rejecting dangerous URI schemes and private/metadata URLs. */
export function safeNonGithubSourceUrl(raw: string): boolean {
  const trimmed = raw.trim();
  if (!trimmed || /[\x00-\x1f\x7f-\x9f]/.test(trimmed) || DANGEROUS_URI_SCHEME_RE.test(trimmed)) {
    return false;
  }
  if (/^(builtin|local):\/\//i.test(trimmed)) {
    const rest = trimmed.replace(/^(builtin|local):\/\//i, "");
    return Boolean(rest) && !rest.includes("..") && !rest.includes("\\");
  }
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed)) {
    return isPublicHttpsUrl(trimmed);
  }
  return !trimmed.includes("..") && !trimmed.includes("\\");
}
