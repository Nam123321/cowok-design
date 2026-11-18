// @story 08.2
// @implements AC-01, AC-02, AC-06, AC-07
import { DiscoveryStub, DiscoveryStubSchema } from "./types";
import { scanSkillMarkdownSafety } from "./repackager-assets";

export const MAX_DISCOVERY_STUB_BYTES = 1536;
const MAX_SKILL_SEGMENT_BYTES = 64;
const MAX_STUB_DESC_BYTES = 320;
const MAX_UPSTREAM_URL_BYTES = 256;
const MAX_STUB_CAPS = 16;
const MAX_CAP_BYTES = 48;
const IPV4_PARTS = 4;

const BLOCKED_STUB_HOSTS = new Set([
  "localhost",
  "0.0.0.0",
  "::",
  "::1",
  "169.254.169.254",
  "metadata.google.internal",
]);

function isDisallowedIpv4(host: string): boolean {
  const parts = host.split(".");
  if (parts.length !== IPV4_PARTS || parts.some((p) => !/^\d{1,3}$/.test(p))) {
    return /^(0x[0-9a-f]+|0[0-7]+|\d+)$/i.test(host);
  }
  const [a, b] = parts.map((p) => Number(p));
  if ([a, b].some((n) => n < 0 || n > 255)) return true;
  if (a === 0 || a === 10 || a === 127 || a >= 224) return true;
  if (a === 100 && b >= 64 && b <= 127) return true;
  if (a === 169 && b === 254) return true;
  return (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
}

function isDisallowedUpstreamHost(rawHost: string): boolean {
  const host = rawHost.replace(/^\[|\]$/g, "").toLowerCase();
  if (!host || BLOCKED_STUB_HOSTS.has(host) || host.endsWith(".localhost") || host.endsWith(".local") || host.endsWith(".internal")) {
    return true;
  }
  if (isDisallowedIpv4(host)) return true;
  return host.includes(":") && (host.startsWith("fe80:") || host.startsWith("fc") || host.startsWith("fd") || host.startsWith("::ffff:"));
}

function validateSafeSkillSegment(segment: string): void {
  if (!segment || segment === "." || segment === ".." || Buffer.byteLength(segment, "utf8") > MAX_SKILL_SEGMENT_BYTES || !/^[A-Za-z0-9._-]+$/.test(segment)) {
    throw new Error(`Invalid skill_name '${segment}': unsafe characters, traversal segment, or length > 64B`);
  }
}

function validateUpstreamUrl(url: string): void {
  if (Buffer.byteLength(url, "utf8") > MAX_UPSTREAM_URL_BYTES || /[\s\x00-\x1f\x7f-\x9f]/.test(url)) {
    throw new Error("Invalid or oversized upstream_url for DiscoveryStub");
  }
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error("Invalid or oversized upstream_url for DiscoveryStub");
  }
  if (parsed.protocol !== "https:" || parsed.username || parsed.password || isDisallowedUpstreamHost(parsed.hostname)) {
    throw new Error("Invalid or oversized upstream_url for DiscoveryStub: must be public https:// URL");
  }
}

/** Builds and size-verifies a <=1536B agentskills.io DiscoveryStub payload. */
export function buildDiscoveryStub(params: {
  skillName: string;
  description: string;
  capabilities: string[];
  upstreamUrl?: string;
  packageSha256: string;
}): { stub: DiscoveryStub; stubJson: string } {
  const { skillName, description, capabilities, upstreamUrl, packageSha256 } = params;
  validateSafeSkillSegment(skillName);
  scanSkillMarkdownSafety("description", description);
  if (description.replace(/\r\n?/g, "\n").split("\n").some((l) => l.trim() === "---")) {
    throw new Error("YAML delimiter '---' line forbidden inside frontmatter field");
  }
  if (Buffer.byteLength(description, "utf8") > MAX_STUB_DESC_BYTES) throw new Error("DiscoveryStub overflow: description > 320B");
  if (capabilities.length > MAX_STUB_CAPS || capabilities.some((c) => Buffer.byteLength(c, "utf8") > MAX_CAP_BYTES)) {
    throw new Error("DiscoveryStub overflow: capabilities limit exceeded");
  }
  for (const cap of capabilities) scanSkillMarkdownSafety("capability", cap);
  if (upstreamUrl !== undefined) validateUpstreamUrl(upstreamUrl);
  const stub: DiscoveryStub = {
    name: skillName,
    version: "1.0.0",
    description,
    ...(upstreamUrl !== undefined ? { upstream_url: upstreamUrl } : {}),
    capabilities,
    risk_tier: 2,
    package_sha256: packageSha256,
    stub_size_bytes: 0,
  };
  for (let i = 0; i < 8; i++) {
    const finalJson = JSON.stringify(stub, null, 2);
    const bytes = Buffer.byteLength(finalJson, "utf8");
    if (bytes === stub.stub_size_bytes) {
      if (bytes > MAX_DISCOVERY_STUB_BYTES) {
        throw new Error(`DiscoveryStubFailClosed: ${bytes}B > ${MAX_DISCOVERY_STUB_BYTES}B`);
      }
      return { stub: DiscoveryStubSchema.parse(stub), stubJson: finalJson };
    }
    stub.stub_size_bytes = bytes;
  }
  throw new Error("DiscoveryStubFailClosed: stub_size_bytes failed to converge to serialized byte length");
}