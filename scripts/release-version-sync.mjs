#!/usr/bin/env node
/**
 * Release Version Sync & Parity Guard (`cowok-design` Phase 1 — v1.0.0 GA)
 *
 * Keeps all 6 release manifests strictly aligned on a single SemVer 2.0.0 version:
 *   1. Cargo.toml (`[workspace.package].version`)
 *   2. package.json (`version`)
 *   3. packaging/tauri/tauri.conf.json (`version`)
 *   4. packaging/tauri/Cargo.toml (`[package].version`)
 *   5. packaging/npm/package.json (`version` + `cowokChecksums.*.asset`)
 *   6. packaging/homebrew/cowok-design.rb (`version "..."`)
 *
 * Usage:
 *   node scripts/release-version-sync.mjs --check [--tag v1.0.0-rc.1]
 *   node scripts/release-version-sync.mjs --set 1.0.0-rc.1
 */
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "..");

const SEMVER_RE =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

function readText(relPath) {
  return fs.readFileSync(path.join(REPO_ROOT, relPath), "utf8");
}

function writeText(relPath, content) {
  fs.writeFileSync(path.join(REPO_ROOT, relPath), content, "utf8");
}

function readTomlSectionVersion(relPath, sectionHeader) {
  const text = readText(relPath);
  const idx = text.indexOf(sectionHeader);
  if (idx < 0) throw new Error(`Missing ${sectionHeader} in ${relPath}`);
  const after = text.slice(idx);
  const nextSection = after.slice(sectionHeader.length).search(/\n\[/);
  const block = nextSection >= 0 ? after.slice(0, sectionHeader.length + nextSection) : after;
  const match = block.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error(`Missing version in ${sectionHeader} of ${relPath}`);
  return match[1];
}

function writeTomlSectionVersion(relPath, sectionHeader, nextVersion) {
  const text = readText(relPath);
  const idx = text.indexOf(sectionHeader);
  if (idx < 0) throw new Error(`Missing ${sectionHeader} in ${relPath}`);
  const before = text.slice(0, idx);
  const after = text.slice(idx);
  const nextSection = after.slice(sectionHeader.length).search(/\n\[/);
  const block = nextSection >= 0 ? after.slice(0, sectionHeader.length + nextSection) : after;
  const rest = nextSection >= 0 ? after.slice(sectionHeader.length + nextSection) : "";
  const updatedBlock = block.replace(/^(\s*version\s*=\s*)"([^"]+)"/m, `$1"${nextVersion}"`);
  writeText(relPath, `${before}${updatedBlock}${rest}`);
}

function readHomebrewVersion(relPath) {
  const text = readText(relPath);
  const match = text.match(/^\s*version\s+"([^"]+)"/m);
  if (!match) throw new Error(`Missing version in ${relPath}`);
  return match[1];
}

function writeHomebrewVersion(relPath, nextVersion) {
  const text = readText(relPath);
  const updated = text.replace(/^(\s*version\s+)"([^"]+)"/m, `$1"${nextVersion}"`);
  writeText(relPath, updated);
}

function inspectAllVersions() {
  const rootPkg = JSON.parse(readText("package.json"));
  const tauriConf = JSON.parse(readText("packaging/tauri/tauri.conf.json"));
  const npmPkg = JSON.parse(readText("packaging/npm/package.json"));

  const versions = {
    "Cargo.toml": readTomlSectionVersion("Cargo.toml", "[workspace.package]"),
    "package.json": String(rootPkg.version ?? ""),
    "packaging/tauri/tauri.conf.json": String(tauriConf.version ?? ""),
    "packaging/tauri/Cargo.toml": readTomlSectionVersion("packaging/tauri/Cargo.toml", "[package]"),
    "packaging/npm/package.json": String(npmPkg.version ?? ""),
    "packaging/homebrew/cowok-design.rb": readHomebrewVersion("packaging/homebrew/cowok-design.rb"),
  };

  const npmAssetMismatches = [];
  const expectedAssetPrefix = `cowok-design-v${versions["packaging/npm/package.json"]}-`;
  for (const [key, entry] of Object.entries(npmPkg.cowokChecksums ?? {})) {
    const asset = String(entry?.asset ?? "");
    if (!asset.startsWith(expectedAssetPrefix)) {
      npmAssetMismatches.push(`${key}: '${asset}' does not start with '${expectedAssetPrefix}'`);
    }
  }

  return { versions, npmAssetMismatches };
}

function setAllVersions(nextVersion) {
  if (!SEMVER_RE.test(nextVersion)) {
    throw new Error(`Invalid SemVer 2.0.0 version: '${nextVersion}'`);
  }

  writeTomlSectionVersion("Cargo.toml", "[workspace.package]", nextVersion);
  writeTomlSectionVersion("packaging/tauri/Cargo.toml", "[package]", nextVersion);
  writeHomebrewVersion("packaging/homebrew/cowok-design.rb", nextVersion);

  for (const relPath of ["package.json", "packaging/tauri/tauri.conf.json"]) {
    const data = JSON.parse(readText(relPath));
    data.version = nextVersion;
    writeText(relPath, `${JSON.stringify(data, null, 2)}\n`);
  }

  const npmPkg = JSON.parse(readText("packaging/npm/package.json"));
  npmPkg.version = nextVersion;
  for (const entry of Object.values(npmPkg.cowokChecksums ?? {})) {
    if (entry && typeof entry === "object" && typeof entry.triple === "string") {
      entry.asset = `cowok-design-v${nextVersion}-${entry.triple}`;
    }
  }
  writeText("packaging/npm/package.json", `${JSON.stringify(npmPkg, null, 2)}\n`);
}

function main() {
  const args = process.argv.slice(2);
  const setIdx = args.indexOf("--set");
  const tagIdx = args.indexOf("--tag");
  const checkMode = args.includes("--check") || setIdx < 0;

  if (setIdx >= 0) {
    const targetVer = args[setIdx + 1] ?? "";
    setAllVersions(targetVer);
    console.log(`✅ Synchronized all 6 release manifests to version ${targetVer}`);
  }

  const { versions, npmAssetMismatches } = inspectAllVersions();
  const unique = new Set(Object.values(versions));
  const errors = [];

  for (const [file, ver] of Object.entries(versions)) {
    if (!SEMVER_RE.test(ver)) {
      errors.push(`${file}: invalid SemVer '${ver}'`);
    }
  }
  if (unique.size !== 1) {
    errors.push(`Version mismatch across manifests: ${JSON.stringify(versions)}`);
  }
  for (const mismatch of npmAssetMismatches) {
    errors.push(`packaging/npm/package.json cowokChecksums.${mismatch}`);
  }

  if (tagIdx >= 0) {
    const rawTag = String(args[tagIdx + 1] ?? "").trim();
    const expected = rawTag.replace(/^refs\/tags\//, "").replace(/^v/, "");
    const actual = [...unique][0];
    if (!expected || actual !== expected) {
      errors.push(`Git tag '${rawTag}' expects version '${expected}', but manifests are at '${actual}'`);
    }
  }

  if (checkMode) {
    if (errors.length > 0) {
      console.error(JSON.stringify({ ok: false, versions, errors }, null, 2));
      process.exit(1);
    }
    console.log(JSON.stringify({ ok: true, version: [...unique][0], versions }, null, 2));
  }
}

main();
