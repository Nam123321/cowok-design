// @story 08.3
// @implements AC-03, AC-05
import { PluginLifecycleError } from "./lifecycle-errors";
import type { BreakingDiffReport } from "./lifecycle-types";

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

/** Deterministic key-order-independent JSON equality check matching Rust Option omitted-key semantics. */
export function stableJsonEqual(a: unknown, b: unknown): boolean {
  const norm = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map((item) => (item === undefined ? null : norm(item)));
    if (v && typeof v === "object") {
      const obj = v as Record<string, unknown>;
      return Object.fromEntries(
        Object.keys(obj).sort().filter((k) => obj[k] !== undefined).map((k) => [k, norm(obj[k])])
      );
    }
    return v ?? null;
  };
  return JSON.stringify(norm(a)) === JSON.stringify(norm(b));
}

const asRecord = (val: unknown): Record<string, unknown> | null =>
  val !== null && typeof val === "object" && !Array.isArray(val) ? (val as Record<string, unknown>) : null;

function canonicalTypeKey(schemaVal: unknown): string {
  const rawType = asRecord(schemaVal)?.type;
  if (rawType === undefined) return "__NONE__";
  if (typeof rawType === "string") return `SET:${JSON.stringify([rawType])}`;
  if (Array.isArray(rawType) && rawType.every((x) => typeof x === "string")) {
    return `SET:${JSON.stringify([...new Set(rawType)].sort())}`;
  }
  return `RAW:${JSON.stringify(rawType)}`;
}

function diffParameterSchemas(current: unknown, candidate: unknown, reasons: string[]): void {
  const curNull = current === null || current === undefined;
  const candNull = candidate === null || candidate === undefined;
  if (curNull || candNull) {
    if (!curNull && candNull) reasons.push("Parameter schema removed completely");
    else if (curNull && !candNull) {
      const candObj = asRecord(candidate);
      if (candObj && Array.isArray(candObj.required) && candObj.required.length > 0) reasons.push("Added new required parameters without existing schema");
      else if (!candObj) reasons.push("Parameter schema structural type changed (expected JSON object)");
    }
    return;
  }
  const curObj = asRecord(current);
  const candObj = asRecord(candidate);
  if (!curObj || !candObj) {
    if (!stableJsonEqual(current, candidate)) reasons.push("Parameter schema structural type changed");
    return;
  }
  const curReq = new Set(Array.isArray(curObj.required) ? curObj.required.filter((v): v is string => typeof v === "string") : []);
  const candReq = new Set(Array.isArray(candObj.required) ? candObj.required.filter((v): v is string => typeof v === "string") : []);
  for (const added of candReq) if (!curReq.has(added)) reasons.push(`Added new required parameter '${added}'`);
  for (const removed of curReq) if (!candReq.has(removed)) reasons.push(`Removed previously required parameter '${removed}'`);

  const curProps = asRecord(curObj.properties);
  const candProps = asRecord(candObj.properties);
  if (curProps && candProps) {
    for (const [propName, oldVal] of Object.entries(curProps)) {
      const nextVal = candProps[propName];
      if (nextVal === undefined) reasons.push(`Removed parameter property '${propName}'`);
      else if (canonicalTypeKey(oldVal) !== canonicalTypeKey(nextVal)) reasons.push(`Parameter '${propName}' type changed`);
    }
  } else if (curProps && !candProps) reasons.push("Removed parameter 'properties' definition");
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
