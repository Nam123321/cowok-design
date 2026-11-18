#!/usr/bin/env node
// @story 04.4
// @implements AC-05 ([PLUGIN-G1..G6] Deterministic Packager + Idempotent SQLite Seed/Rollback)
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import zlib from "node:zlib";

const PLUGIN_DIR = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.join(PLUGIN_DIR, "dist");
const ZIP_PATH = path.join(DIST_DIR, "cli-worker-subprocess-adapter.zip");
const SHA_PATH = path.join(DIST_DIR, "cli-worker-subprocess-adapter.sha256");
const MANIFEST_PATH = path.join(PLUGIN_DIR, "manifest.json");
const PLUGIN_FILES = ["manifest.json", "skills/cli-worker/SKILL.md"];

function crc32(bytes) {
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i += 1) {
    crc ^= bytes[i];
    for (let j = 0; j < 8; j += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function buildDeterministicZip(entries) {
  const encoder = new TextEncoder();
  const localParts = [];
  const centralParts = [];
  let offset = 0;

  for (const { name, data } of entries) {
    const nameBytes = encoder.encode(name);
    const compressed = new Uint8Array(zlib.deflateRawSync(data, { level: 9 }));
    const crc = crc32(data);

    const local = new Uint8Array(30 + nameBytes.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint16(8, 8, true);
    lv.setUint16(10, 0, true);
    lv.setUint16(12, 0x0021, true); // fixed 1980-01-01 date
    lv.setUint32(14, crc, true);
    lv.setUint32(18, compressed.length, true);
    lv.setUint32(22, data.length, true);
    lv.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);

    localParts.push(local, compressed);

    const central = new Uint8Array(46 + nameBytes.length);
    const cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 20, true);
    cv.setUint16(6, 20, true);
    cv.setUint16(10, 8, true);
    cv.setUint16(12, 0, true);
    cv.setUint16(14, 0x0021, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, compressed.length, true);
    cv.setUint32(24, data.length, true);
    cv.setUint16(28, nameBytes.length, true);
    cv.setUint32(38, 0o100644 << 16, true);
    cv.setUint32(42, offset, true);
    central.set(nameBytes, 46);
    centralParts.push(central);

    offset += local.length + compressed.length;
  }

  const centralSize = centralParts.reduce((s, p) => s + p.length, 0);
  const eocd = new Uint8Array(22);
  const ev = new DataView(eocd.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, entries.length, true);
  ev.setUint16(10, entries.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, offset, true);

  const totalLen = offset + centralSize + eocd.length;
  const out = new Uint8Array(totalLen);
  let pos = 0;
  for (const part of [...localParts, ...centralParts, eocd]) {
    out.set(part, pos);
    pos += part.length;
  }
  return out;
}

export function packagePlugin() {
  fs.mkdirSync(DIST_DIR, { recursive: true });
  const manifestRaw = fs.readFileSync(MANIFEST_PATH, "utf8");
  const manifest = JSON.parse(manifestRaw);
  const { sha256: _prevSha, checksum: _prevChecksum, ...manifestWithoutHash } = manifest;
  const normalizedManifestRaw = `${JSON.stringify(manifestWithoutHash, null, 2)}\n`;
  const entries = [...PLUGIN_FILES].sort().map((rel) => ({
    name: rel,
    data: rel === "manifest.json"
      ? new TextEncoder().encode(normalizedManifestRaw)
      : new Uint8Array(fs.readFileSync(path.join(PLUGIN_DIR, rel))),
  }));
  const zipBytes = buildDeterministicZip(entries);
  const packSha256 = crypto.createHash("sha256").update(zipBytes).digest("hex");
  const updatedManifestRaw = `${JSON.stringify({ ...manifestWithoutHash, sha256: packSha256 }, null, 2)}\n`;
  if (manifestRaw !== updatedManifestRaw) {
    fs.writeFileSync(MANIFEST_PATH, updatedManifestRaw, "utf8");
  }
  fs.writeFileSync(ZIP_PATH, zipBytes);
  fs.writeFileSync(SHA_PATH, `${packSha256}  cli-worker-subprocess-adapter.zip\n`, "utf8");
  return { zipPath: ZIP_PATH, packSha256 };
}

function runSqliteOp(mode, dbPath, projectId) {
  const { packSha256 } = packagePlugin();
  const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, "utf8"));
  manifest.packSha256 = packSha256;

  const pyScript = mode === "seed"
    ? `import sqlite3, sys, time
db_path, project_id, slug, version, mjson = sys.argv[1:6]
now_ms = int(time.time() * 1000)
conn = sqlite3.connect(db_path)
row_proj = conn.execute("SELECT id FROM projects WHERE id = ?", (project_id,)).fetchone()
if row_proj is None:
    cols = {r[1] for r in conn.execute("PRAGMA table_info(projects)").fetchall()}
    if "tenant_id" in cols:
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, product_brief, status, created_at, updated_at) VALUES (?, 'local-default', ?, 'CLI Worker Default Project', 'Plugin Host Seed Project', 'ACTIVE', ?, ?)",
            (project_id, f"proj-{project_id[:8]}", now_ms, now_ms),
        )
    else:
        conn.execute(
            "INSERT INTO projects (id, slug, name, product_brief, status, created_at, updated_at) VALUES (?, ?, 'CLI Worker Default Project', 'Plugin Host Seed Project', 'ACTIVE', ?, ?)",
            (project_id, f"proj-{project_id[:8]}", now_ms, now_ms),
        )
pid_row = conn.execute(
    "SELECT id FROM skill_plugins WHERE project_id = ? AND plugin_slug = ? AND version = ?",
    (project_id, slug, version),
).fetchone()
plugin_id = f"plugin-{slug}-{project_id}"
if pid_row is None:
    conn.execute(
        "INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at) VALUES (?, ?, ?, ?, 'ACTIVE', ?, ?, ?)",
        (plugin_id, project_id, slug, version, "local://src/plugins/cli-worker-subprocess-adapter", mjson, now_ms),
    )
else:
    conn.execute(
        "UPDATE skill_plugins SET lifecycle_state = 'ACTIVE', manifest_json = ?, updated_at = ? WHERE id = ?",
        (mjson, now_ms, pid_row[0]),
    )
conn.commit()
print("SEEDED_OK", slug, version)`
    : `import sqlite3, sys
db_path, project_id, slug = sys.argv[1:4]
conn = sqlite3.connect(db_path)
cur = conn.execute("DELETE FROM skill_plugins WHERE project_id = ? AND plugin_slug = ?", (project_id, slug))
conn.commit()
print("ROLLBACK_OK", cur.rowcount)`;

  const args = mode === "seed"
    ? ["-c", pyScript, dbPath, projectId, manifest.slug, manifest.version, JSON.stringify(manifest)]
    : ["-c", pyScript, dbPath, projectId, manifest.slug];
  const res = spawnSync("python3", args, { encoding: "utf8" });
  if (res.status !== 0) {
    throw new Error(`SQLite ${mode} failed: ${res.stderr || res.stdout}`);
  }
  return res.stdout.trim();
}

export function parseCliArgs(argv) {
  const args = new Set(argv);
  const dbIdx = argv.indexOf("--db");
  const projIdx = argv.indexOf("--project");
  const wsRoot = process.env.UXUI_OS_WORKSPACE_ROOT || process.cwd();
  const defaultDb = path.resolve(wsRoot, ".cowok-design", "cowok-design.sqlite3");
  const dbPath = dbIdx >= 0 && argv[dbIdx + 1] ? path.resolve(argv[dbIdx + 1]) : defaultDb;
  const projectId = projIdx >= 0 && argv[projIdx + 1] ? argv[projIdx + 1] : "local-default-project";
  return {
    seed: args.has("--seed"),
    rollback: args.has("--rollback") || args.has("--down"),
    dbPath,
    projectId,
  };
}

export async function seedPlugin(dbPath = parseCliArgs([]).dbPath, projectId = "local-default-project") {
  return runSqliteOp("seed", dbPath, projectId);
}

export async function rollbackPlugin(dbPath = parseCliArgs([]).dbPath, projectId = "local-default-project") {
  return runSqliteOp("rollback", dbPath, projectId);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const opts = parseCliArgs(process.argv.slice(2));
  if (opts.rollback) {
    console.log(await rollbackPlugin(opts.dbPath, opts.projectId));
  } else if (opts.seed) {
    console.log(await seedPlugin(opts.dbPath, opts.projectId));
  } else {
    const out = packagePlugin();
    console.log(JSON.stringify(out));
  }
}
