#!/usr/bin/env node
// @story 12.1
// @implements AC-01 (Zero-Dep Node 20+ npx Wrapper with Constant-Time SHA-256 Verification & Signal Forwarding)

import { spawn } from "node:child_process";
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import fs from "node:fs";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PKG_PATH = path.resolve(__dirname, "..", "package.json");

export const EMPTY_FILE_SHA256 =
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const MAX_DOWNLOAD_BYTES = 256 * 1024 * 1024; // 256 MiB

export function loadManifest() {
  const raw = fs.readFileSync(PKG_PATH, "utf8");
  const pkg = JSON.parse(raw);
  return {
    name: pkg.name,
    version: pkg.version,
    releaseBaseUrl:
      process.env.COWOK_RELEASE_BASE_URL ||
      pkg.cowokReleaseBaseUrl ||
      "https://github.com/cowok-ai/cowok-design/releases/download",
    checksums: pkg.cowokChecksums || {},
  };
}

export function resolvePlatformTarget(
  platform = process.platform,
  arch = process.arch,
  manifest = loadManifest()
) {
  const key = `${platform}-${arch}`;
  const entry = manifest.checksums[key];
  if (!entry) {
    throw new Error(
      `E_UNSUPPORTED_PLATFORM: no prebuilt cowok-design binary for '${key}'`
    );
  }
  return {
    key,
    triple: entry.triple,
    asset: entry.asset,
    sha256: entry.sha256,
    url: `${manifest.releaseBaseUrl}/v${manifest.version}/${entry.asset}`,
  };
}

export function isValidSha256Hex(hex) {
  if (typeof hex !== "string") {
    return false;
  }
  const normalized = hex.trim().toLowerCase();
  return /^[0-9a-f]{64}$/.test(normalized) && normalized !== EMPTY_FILE_SHA256;
}

export function computeFileSha256(filePath) {
  const hash = createHash("sha256");
  const data = fs.readFileSync(filePath);
  hash.update(data);
  return hash.digest("hex");
}

export function verifyFileSha256(filePath, expectedSha256, options = {}) {
  const { deleteOnMismatch = false } = options;
  const cleanup = () => {
    if (deleteOnMismatch && fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
  };
  if (!isValidSha256Hex(expectedSha256)) {
    cleanup();
    return {
      ok: false,
      code: "E_CHECKSUM_MISMATCH",
      expected: String(expectedSha256 || ""),
      actual: null,
      reason:
        "Expected SHA-256 must be a valid non-empty 64-character hexadecimal string (empty-file SHA-256 and release placeholders are rejected)",
    };
  }

  let stat;
  try {
    stat = fs.statSync(filePath);
  } catch (err) {
    return {
      ok: false,
      code: "E_BINARY_NOT_FOUND",
      expected: expectedSha256.trim().toLowerCase(),
      actual: null,
      reason: `Cannot stat binary '${path.basename(filePath)}': ${err.message}`,
    };
  }

  if (!stat.isFile() || stat.size === 0) {
    cleanup();
    return {
      ok: false,
      code: "E_CHECKSUM_MISMATCH",
      expected: expectedSha256.trim().toLowerCase(),
      actual: stat.size === 0 ? EMPTY_FILE_SHA256 : null,
      reason: `Refusing empty (0-byte) or non-regular binary '${path.basename(filePath)}'`,
    };
  }

  const actualHex = computeFileSha256(filePath).toLowerCase();
  if (actualHex === EMPTY_FILE_SHA256) {
    cleanup();
    return {
      ok: false,
      code: "E_CHECKSUM_MISMATCH",
      expected: expectedSha256.trim().toLowerCase(),
      actual: actualHex,
      reason: `Empty-file SHA-256 rejected for ${path.basename(filePath)}`,
    };
  }

  const expectedBuf = Buffer.from(expectedSha256.trim().toLowerCase(), "hex");
  const actualBuf = Buffer.from(actualHex, "hex");
  const matched =
    expectedBuf.length === actualBuf.length &&
    timingSafeEqual(expectedBuf, actualBuf);

  if (!matched) {
    cleanup();
    return {
      ok: false,
      code: "E_CHECKSUM_MISMATCH",
      expected: expectedSha256.trim().toLowerCase(),
      actual: actualHex,
      reason: `SHA-256 mismatch for ${path.basename(filePath)}`,
    };
  }

  return {
    ok: true,
    code: "OK",
    expected: expectedSha256.trim().toLowerCase(),
    actual: actualHex,
  };
}

function resolveCacheDir(version) {
  const base =
    process.env.XDG_CACHE_HOME ||
    (process.platform === "darwin"
      ? path.join(os.homedir(), "Library", "Caches")
      : path.join(os.homedir(), ".cache"));
  return path.join(base, "cowok-design", "bin", `v${version}`);
}

function downloadToTemp(currentUrl, destTmpPath, expectedSha256, redirectsLeft = 3) {
  if (!isValidSha256Hex(expectedSha256)) {
    const err = new Error(
      `E_CHECKSUM_MISMATCH: refusing to download without a valid non-empty release SHA-256 (got '${expectedSha256}')`
    );
    err.exitCode = 3;
    return Promise.reject(err);
  }

  let parsedUrl;
  try {
    parsedUrl = new URL(currentUrl);
  } catch {
    return Promise.reject(
      new Error(`E_DOWNLOAD_FAILED: invalid download URL '${currentUrl}'`)
    );
  }
  if (parsedUrl.protocol !== "https:") {
    return Promise.reject(
      new Error(
        `E_DOWNLOAD_FAILED: refusing non-HTTPS download URL '${parsedUrl.toString()}'`
      )
    );
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    let out = null;
    const cleanupFile = () => {
      try {
        if (fs.existsSync(destTmpPath)) {
          fs.unlinkSync(destTmpPath);
        }
      } catch {
        // ignore cleanup failure
      }
    };
    const fail = (err, reqObj) => {
      if (settled) {
        return;
      }
      settled = true;
      try {
        if (out) {
          out.destroy();
        }
      } catch {
        // ignore
      }
      try {
        if (reqObj) {
          reqObj.destroy();
        }
      } catch {
        // ignore
      }
      cleanupFile();
      reject(err);
    };

    const req = https.get(parsedUrl, (res) => {
      if (
        res.statusCode &&
        res.statusCode >= 300 &&
        res.statusCode < 400 &&
        res.headers.location
      ) {
        res.resume();
        if (redirectsLeft <= 0) {
          fail(new Error("E_DOWNLOAD_FAILED: too many HTTP redirects"), req);
          return;
        }
        let nextUrl;
        try {
          nextUrl = new URL(res.headers.location, parsedUrl);
        } catch {
          fail(
            new Error(
              `E_DOWNLOAD_FAILED: invalid redirect location '${res.headers.location}'`
            ),
            req
          );
          return;
        }
        if (nextUrl.protocol !== "https:") {
          fail(
            new Error(
              `E_DOWNLOAD_FAILED: refusing non-HTTPS redirect '${nextUrl.toString()}'`
            ),
            req
          );
          return;
        }
        settled = true;
        resolve(
          downloadToTemp(
            nextUrl.toString(),
            destTmpPath,
            expectedSha256,
            redirectsLeft - 1
          )
        );
        return;
      }

      if (res.statusCode !== 200) {
        res.resume();
        fail(
          new Error(
            `E_DOWNLOAD_FAILED: HTTP ${res.statusCode} from ${parsedUrl.toString()}`
          ),
          req
        );
        return;
      }

      const contentLength = Number(res.headers["content-length"]);
      if (Number.isFinite(contentLength) && contentLength > MAX_DOWNLOAD_BYTES) {
        res.resume();
        fail(
          new Error(
            `E_DOWNLOAD_TOO_LARGE: remote asset exceeds ${MAX_DOWNLOAD_BYTES} bytes`
          ),
          req
        );
        return;
      }

      try {
        out = fs.createWriteStream(destTmpPath, { flags: "wx", mode: 0o700 });
      } catch (err) {
        res.resume();
        fail(err, req);
        return;
      }

      let receivedBytes = 0;
      res.on("data", (chunk) => {
        receivedBytes += chunk.length;
        if (receivedBytes > MAX_DOWNLOAD_BYTES) {
          res.destroy(
            new Error(
              `E_DOWNLOAD_TOO_LARGE: download exceeded ${MAX_DOWNLOAD_BYTES} bytes`
            )
          );
        }
      });
      res.on("error", (err) => fail(err, req));
      out.on("error", (err) => fail(err, req));
      out.on("finish", () => {
        if (settled) {
          return;
        }
        out.close((closeErr) => {
          if (closeErr) {
            fail(closeErr, req);
            return;
          }
          settled = true;
          resolve(destTmpPath);
        });
      });
      res.pipe(out);
    });

    req.on("error", (err) => fail(err, req));
  });
}

async function ensureVerifiedBinary(manifest) {
  const overrideBin = process.env.COWOK_DESIGN_BIN;
  if (overrideBin && overrideBin.trim()) {
    const binPath = path.resolve(overrideBin.trim());
    if (!fs.existsSync(binPath)) {
      throw new Error(
        `E_BINARY_NOT_FOUND: COWOK_DESIGN_BIN '${binPath}' does not exist`
      );
    }
    const expectedOverrideSha = process.env.COWOK_DESIGN_EXPECTED_SHA256;
    if (expectedOverrideSha !== undefined) {
      const check = verifyFileSha256(binPath, expectedOverrideSha, {
        deleteOnMismatch: false,
      });
      if (!check.ok) {
        const err = new Error(
          `${check.code}: ${check.reason} (expected=${check.expected}, actual=${check.actual})`
        );
        err.exitCode = 3;
        throw err;
      }
      return binPath;
    }
    if (process.env.COWOK_ALLOW_UNVERIFIED_DEV_BIN === "1") {
      return binPath;
    }
    const err = new Error(
      "E_CHECKSUM_REQUIRED: COWOK_DESIGN_BIN requires COWOK_DESIGN_EXPECTED_SHA256 (or set COWOK_ALLOW_UNVERIFIED_DEV_BIN=1 for local dev)"
    );
    err.exitCode = 3;
    throw err;
  }

  const target = resolvePlatformTarget(process.platform, process.arch, manifest);
  if (!isValidSha256Hex(target.sha256)) {
    const err = new Error(
      `E_CHECKSUM_MISMATCH: release manifest checksum for '${target.key}' is unconfigured or invalid ('${target.sha256}')`
    );
    err.exitCode = 3;
    throw err;
  }

  const cacheDir = resolveCacheDir(manifest.version);
  fs.mkdirSync(cacheDir, { recursive: true, mode: 0o700 });
  const binPath = path.join(cacheDir, "cowok-design");

  if (fs.existsSync(binPath)) {
    const cachedCheck = verifyFileSha256(binPath, target.sha256, {
      deleteOnMismatch: true,
    });
    if (cachedCheck.ok) {
      return binPath;
    }
  }

  const tmpPath = path.join(
    cacheDir,
    `.cowok-design-download-${randomBytes(16).toString("hex")}.tmp`
  );
  await downloadToTemp(target.url, tmpPath, target.sha256);
  const check = verifyFileSha256(tmpPath, target.sha256, {
    deleteOnMismatch: true,
  });
  if (!check.ok) {
    const err = new Error(
      `${check.code}: ${check.reason} (expected=${check.expected}, actual=${check.actual})`
    );
    err.exitCode = 3;
    throw err;
  }
  fs.chmodSync(tmpPath, 0o755);
  fs.renameSync(tmpPath, binPath);
  return binPath;
}

function spawnWithSignalForwarding(binPath, args) {
  return new Promise((resolve) => {
    const child = spawn(binPath, args, { stdio: "inherit" });
    const forwardSigInt = () => child.kill("SIGINT");
    const forwardSigTerm = () => child.kill("SIGTERM");
    process.on("SIGINT", forwardSigInt);
    process.on("SIGTERM", forwardSigTerm);

    child.on("exit", (code, signal) => {
      process.off("SIGINT", forwardSigInt);
      process.off("SIGTERM", forwardSigTerm);
      if (signal === "SIGINT") {
        resolve(130);
      } else if (signal === "SIGTERM") {
        resolve(143);
      } else {
        resolve(code ?? 1);
      }
    });
    child.on("error", (err) => {
      process.off("SIGINT", forwardSigInt);
      process.off("SIGTERM", forwardSigTerm);
      process.stderr.write(`[cowok-design] spawn failed: ${err.message}\n`);
      resolve(1);
    });
  });
}

async function main() {
  const args = process.argv.slice(2);
  const manifest = loadManifest();

  if (args[0] === "--print-manifest") {
    process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
    process.exit(0);
  }

  if (args[0] === "--verify-binary") {
    const binIdx = 1;
    const targetFile = args[binIdx];
    const shaFlagIdx = args.indexOf("--expected-sha256");
    const expectedSha = shaFlagIdx >= 0 ? args[shaFlagIdx + 1] : "";
    const deleteOnMismatch = args.includes("--delete-on-mismatch");
    if (!targetFile) {
      process.stderr.write("E_INVALID_ARGS: --verify-binary <path> required\n");
      process.exit(2);
    }
    const res = verifyFileSha256(path.resolve(targetFile), expectedSha, {
      deleteOnMismatch,
    });
    if (!res.ok) {
      process.stderr.write(`${res.code}: ${res.reason}\n`);
      process.exit(3);
    }
    process.stdout.write(`${JSON.stringify(res)}\n`);
    process.exit(0);
  }

  try {
    const binPath = await ensureVerifiedBinary(manifest);
    const exitCode = await spawnWithSignalForwarding(binPath, args);
    process.exit(exitCode);
  } catch (err) {
    process.stderr.write(`${err.message}\n`);
    process.exit(err.exitCode || 1);
  }
}

function isDirectExecution() {
  if (!process.argv[1]) {
    return false;
  }
  try {
    return (
      fs.realpathSync(path.resolve(process.argv[1])) ===
      fs.realpathSync(__filename)
    );
  } catch {
    return path.resolve(process.argv[1]) === __filename;
  }
}

if (isDirectExecution()) {
  main();
}
