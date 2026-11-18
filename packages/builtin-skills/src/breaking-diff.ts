// @story 08.3
// @implements AC-03, AC-05
import { diffParameterSchemas, stableJsonEqual } from "./breaking-schema-diff";
import { PluginLifecycleError } from "./lifecycle-errors";
import type { BreakingDiffReport } from "./lifecycle-types";

export { stableJsonEqual };

export interface DiffableManifest {
  version: string;
  entry_point: string;
  capabilities_required?: string[];
  risk_tier?: number;
  parameters?: unknown;
}

export interface SemVer {
  major: bigint;
  minor: bigint;
  patch: bigint;
}

const U64_MAX = 18446744073709551615n;

/** Strict 'MAJOR.MINOR.PATCH' u64 parser rejecting leading zeros, u64 overflow, and non-digits. */
export function parseSemVerStrict(raw: string): SemVer {
  const parts = raw.split(".");
  if (parts.length !== 3 || parts.some((p) => !/^\d+$/.test(p) || (p.length > 1 && p.startsWith("0")))) {
    throw new PluginLifecycleError("InvalidSemVer", `Invalid SemVer '${raw}': expected strict 'MAJOR.MINOR.PATCH' numeric digits`);
  }
  const [major, minor, patch] = parts.map((p) => BigInt(p));
  if (major > U64_MAX || minor > U64_MAX || patch > U64_MAX) {
    throw new PluginLifecycleError("InvalidSemVer", `Invalid SemVer '${raw}': version component exceeds u64 range`);
  }
  return { major, minor, patch };
}

export function compareSemVer(a: SemVer, b: SemVer): number {
  if (a.major !== b.major) return a.major < b.major ? -1 : 1;
  if (a.minor !== b.minor) return a.minor < b.minor ? -1 : 1;
  if (a.patch !== b.patch) return a.patch < b.patch ? -1 : 1;
  return 0;
}

/** Authoritative structural manifest & SemVer breaking-change detector matching Rust plugin-host (EC-E08-03). */
export function detectBreakingChange(current: DiffableManifest, candidate: DiffableManifest, forceBreakingHint = false): BreakingDiffReport {
  const curVer = parseSemVerStrict(current.version);
  const candVer = parseSemVerStrict(candidate.version);
  const reasons: string[] = [];
  if (compareSemVer(candVer, curVer) < 0) reasons.push(`Version downgrade from ${current.version} to ${candidate.version}`);
  else if (candVer.major > curVer.major) reasons.push(`Major SemVer bump from ${current.version} to ${candidate.version}`);
  else if (curVer.major === 0n && candVer.minor > curVer.minor) reasons.push(`Pre-1.0 minor bump from ${current.version} to ${candidate.version}`);
  if (candidate.entry_point !== current.entry_point) reasons.push(`Entry point changed from '${current.entry_point}' to '${candidate.entry_point}'`);

  const curCaps = new Set(current.capabilities_required ?? []);
  const candCaps = new Set(candidate.capabilities_required ?? []);
  for (const added of candCaps) if (!curCaps.has(added)) reasons.push(`Privilege escalation: added capability '${added}'`);
  for (const removed of curCaps) if (!candCaps.has(removed)) reasons.push(`Contract change: removed capability '${removed}'`);
  if ((candidate.risk_tier ?? 1) > (current.risk_tier ?? 1)) reasons.push(`Privilege escalation: risk_tier increased from ${current.risk_tier ?? 1} to ${candidate.risk_tier ?? 1}`);
  diffParameterSchemas(current.parameters, candidate.parameters, reasons);
  if (forceBreakingHint) reasons.push("Caller flagged conservative breaking-change hint");
  return { is_breaking: reasons.length > 0, reasons };
}
