import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { deflateRawSync } from "node:zlib";

const PLUGIN_DIR = dirname(fileURLToPath(import.meta.url));
const FIXED_DOS_TIME = 0x0000;
const FIXED_DOS_DATE = 0x0021; // 1980-01-01

function buildCrc32Table() {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i += 1) {
    let c = i;
    for (let k = 0; k < 8; k += 1) {
      c = (c & 1) !== 0 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[i] = c >>> 0;
  }
  return table;
}

const CRC_TABLE = buildCrc32Table();

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i += 1) {
    crc = CRC_TABLE[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function buildZipEntryRecord(entry, offset) {
  const nameBuf = Buffer.from(entry.name, "utf8");
  const rawBuf = Buffer.isBuffer(entry.data) ? entry.data : Buffer.from(entry.data, "utf8");
  const compressed = deflateRawSync(rawBuf, { level: 9 });
  const crc = crc32(rawBuf);

  const local = Buffer.alloc(30 + nameBuf.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(8, 8);
  local.writeUInt16LE(FIXED_DOS_TIME, 10);
  local.writeUInt16LE(FIXED_DOS_DATE, 12);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(compressed.length, 18);
  local.writeUInt32LE(rawBuf.length, 22);
  local.writeUInt16LE(nameBuf.length, 26);
  nameBuf.copy(local, 30);

  const central = Buffer.alloc(46 + nameBuf.length);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(0x031e, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(8, 10);
  central.writeUInt16LE(FIXED_DOS_TIME, 12);
  central.writeUInt16LE(FIXED_DOS_DATE, 14);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(compressed.length, 20);
  central.writeUInt32LE(rawBuf.length, 24);
  central.writeUInt16LE(nameBuf.length, 28);
  central.writeUInt32LE((0o100644 << 16) >>> 0, 38);
  central.writeUInt32LE(offset, 42);
  nameBuf.copy(central, 46);
  return { local, compressed, central };
}

function createDeterministicZipBuffer(entries) {
  const sorted = [...entries].sort((a, b) => a.name.localeCompare(b.name));
  const localChunks = [];
  const centralChunks = [];
  let offset = 0;

  for (const entry of sorted) {
    const rec = buildZipEntryRecord(entry, offset);
    localChunks.push(rec.local, rec.compressed);
    centralChunks.push(rec.central);
    offset += rec.local.length + rec.compressed.length;
  }

  const centralDir = Buffer.concat(centralChunks);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(sorted.length, 8);
  eocd.writeUInt16LE(sorted.length, 10);
  eocd.writeUInt32LE(centralDir.length, 12);
  eocd.writeUInt32LE(offset, 16);
  return Buffer.concat([...localChunks, centralDir, eocd]);
}

export function buildDeterministicZip(pluginDir = PLUGIN_DIR) {
  const manifestPath = join(pluginDir, "manifest.json");
  const skillPath = join(pluginDir, "skills", "cli-doctor", "SKILL.md");
  const manifestRaw = readFileSync(manifestPath, "utf8");
  const manifest = JSON.parse(manifestRaw);
  const { sha256: _prevSha, checksum: _prevChecksum, ...manifestWithoutHash } = manifest;
  const normalizedManifestRaw = `${JSON.stringify(manifestWithoutHash, null, 2)}\n`;
  const skillRaw = readFileSync(skillPath, "utf8");

  const zipBuf = createDeterministicZipBuffer([
    { name: "manifest.json", data: normalizedManifestRaw },
    { name: "skills/cli-doctor/SKILL.md", data: skillRaw },
  ]);
  const packSha256 = createHash("sha256").update(zipBuf).digest("hex");
  const updatedManifestRaw = `${JSON.stringify({ ...manifestWithoutHash, sha256: packSha256 }, null, 2)}\n`;
  if (manifestRaw !== updatedManifestRaw) {
    writeFileSync(manifestPath, updatedManifestRaw, "utf8");
  }
  const distDir = join(pluginDir, "dist");
  mkdirSync(distDir, { recursive: true });

  const zipPath = join(distDir, `${manifest.slug}.zip`);
  writeFileSync(zipPath, zipBuf);
  const checksumPath = join(distDir, "pack-checksum.json");
  const checksumDoc = {
    id: manifest.id,
    slug: manifest.slug,
    version: manifest.version,
    pack_sha256: packSha256,
    archive: `${manifest.slug}.zip`,
    entries: ["manifest.json", "skills/cli-doctor/SKILL.md"],
  };
  writeFileSync(checksumPath, `${JSON.stringify(checksumDoc, null, 2)}\n`, "utf8");
  return { manifest: { ...manifestWithoutHash, sha256: packSha256 }, packSha256, zipPath, checksumPath };
}

function resolveDefaultDbPath() {
  return resolve(PLUGIN_DIR, "..", "..", "..", ".cowok-design", "cowok-design.sqlite3");
}

function ensureTargetProject(db) {
  const existing = db.prepare("SELECT id FROM projects ORDER BY updated_at DESC LIMIT 1").get();
  if (existing && typeof existing.id === "string") return existing.id;
  const now = Date.now();
  const projectId = "proj-cowok-default";
  db.prepare(
    `INSERT INTO projects (id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
     VALUES (?, ?, ?, '.', '', 'ACTIVE', ?, ?)
     ON CONFLICT(slug) DO UPDATE SET updated_at = excluded.updated_at`
  ).run(projectId, "cowok-default", "Cowok Default Workspace", now, now);
  const row = db.prepare("SELECT id FROM projects WHERE slug = 'cowok-default'").get();
  return row.id;
}

export async function seedPlugin(dbPath = resolveDefaultDbPath(), pluginDir = PLUGIN_DIR) {
  if (!existsSync(dbPath)) throw new Error(`SQLite database not found at ${dbPath}`);
  const { DatabaseSync } = await import(/* @vite-ignore */ "node:sqlite");
  const { manifest, packSha256, zipPath } = buildDeterministicZip(pluginDir);
  const db = new DatabaseSync(dbPath);
  try {
    const projectId = ensureTargetProject(db);
    const now = Date.now();
    const enrichedManifest = JSON.stringify({ ...manifest, pack_sha256: packSha256 });
    const rowId = `plugin-${manifest.slug}-${manifest.version}`;
    db.prepare(
      `INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
       VALUES (?, ?, ?, ?, 'ENABLED', 'local://src/plugins/cli-doctor-extension', ?, ?)
       ON CONFLICT(project_id, plugin_slug, version)
       DO UPDATE SET lifecycle_state = 'ENABLED', manifest_json = excluded.manifest_json, updated_at = excluded.updated_at`
    ).run(rowId, projectId, manifest.slug, manifest.version, enrichedManifest, now);
    return { action: "seeded", projectId, slug: manifest.slug, version: manifest.version, packSha256, zipPath };
  } finally {
    db.close();
  }
}

export async function rollbackPlugin(dbPath = resolveDefaultDbPath()) {
  if (!existsSync(dbPath)) throw new Error(`SQLite database not found at ${dbPath}`);
  const { DatabaseSync } = await import(/* @vite-ignore */ "node:sqlite");
  const db = new DatabaseSync(dbPath);
  try {
    const res = db.prepare("DELETE FROM skill_plugins WHERE plugin_slug = ?").run("cli-doctor-extension");
    return { action: "rollback", slug: "cli-doctor-extension", deletedRows: Number(res.changes ?? 0) };
  } finally {
    db.close();
  }
}

export function parseCliArgs(argv) {
  const args = new Set(argv);
  const dbIdx = argv.indexOf("--db");
  const dbPath = dbIdx !== -1 && argv[dbIdx + 1] ? resolve(argv[dbIdx + 1]) : resolveDefaultDbPath();
  return { seed: args.has("--seed"), rollback: args.has("--rollback") || args.has("--down"), dbPath };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const opts = parseCliArgs(process.argv.slice(2));
  if (opts.rollback) {
    console.log(JSON.stringify(await rollbackPlugin(opts.dbPath)));
  } else if (opts.seed) {
    console.log(JSON.stringify(await seedPlugin(opts.dbPath)));
  } else {
    console.log(JSON.stringify(buildDeterministicZip()));
  }
}
