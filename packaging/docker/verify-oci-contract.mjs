#!/usr/bin/env node
// Story 12.3: Deterministic OCI Image, Entrypoint & Compose Contract Verifier (Zero External Dependencies)
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "../..");

const contractPath = path.join(__dirname, "oci-image-contract.json");
const dockerfilePath = path.join(__dirname, "Dockerfile");
const entrypointPath = path.join(__dirname, "entrypoint.sh");
const composePath = path.join(__dirname, "docker-compose.ci.yml");
const ciGateRsPath = path.join(repoRoot, "crates/harness-daemon/src/ci_gate.rs");

const violations = [];
const checks = [];

function check(id, condition, detail) {
  checks.push({ id, passed: Boolean(condition), detail });
  if (!condition) {
    violations.push(`${id}: ${detail}`);
  }
}

function readUtf8(filePath) {
  return fs.readFileSync(filePath, "utf8");
}

function parseComposeServices(composeText) {
  const lines = composeText.split(/\r?\n/);
  const services = new Map();
  let inServices = false;
  let currentService = null;

  for (const line of lines) {
    if (/^\s*#/.test(line) || line.trim() === "") {
      continue;
    }
    if (/^services:\s*$/.test(line)) {
      inServices = true;
      currentService = null;
      continue;
    }
    if (inServices && /^\S/.test(line)) {
      inServices = false;
      currentService = null;
      continue;
    }
    if (!inServices) {
      continue;
    }
    const svcMatch = line.match(/^  ([A-Za-z0-9_.-]+):\s*$/);
    if (svcMatch) {
      currentService = svcMatch[1];
      services.set(currentService, []);
      continue;
    }
    if (currentService) {
      services.get(currentService).push(line);
    }
  }
  return services;
}

function main() {
  const contract = JSON.parse(readUtf8(contractPath));
  const dockerfile = readUtf8(dockerfilePath);
  const entrypoint = readUtf8(entrypointPath);
  const compose = readUtf8(composePath);

  // 1. Contract JSON invariants
  check(
    "contract.image",
    contract.image === "ghcr.io/cowok-ai/uiux-os",
    `expected ghcr.io/cowok-ai/uiux-os, got ${contract.image}`,
  );
  check(
    "contract.platforms",
    Array.isArray(contract.platforms) &&
      contract.platforms.includes("linux/amd64") &&
      contract.platforms.includes("linux/arm64"),
    "must include linux/amd64 and linux/arm64",
  );
  check(
    "contract.user",
    contract.user?.uid === 10001 &&
      contract.user?.gid === 10001 &&
      contract.user?.nonRootRequired === true,
    "must pin non-root user uid/gid 10001",
  );
  check(
    "contract.pooled_cdp",
    contract.pooledChromiumCdp?.mode === "pooled-1-chromium-cdp" &&
      contract.pooledChromiumCdp?.maxConcurrency === 1 &&
      contract.pooledChromiumCdp?.shmFallbackThresholdBytes === 268435456 &&
      contract.pooledChromiumCdp?.containerNoSandboxFlag === "--no-sandbox",
    "must pin Pooled 1-Chromium CDP with 256 MiB /dev/shm threshold and container --no-sandbox contract",
  );
  check(
    "contract.invariants",
    contract.invariants?.zeroLibreoffice === true &&
      contract.invariants?.zeroCircularDepOnStory13_1 === true &&
      contract.invariants?.tiniSubreaperFlag === "-s",
    "must enforce zeroLibreoffice, zeroCircularDepOnStory13_1, and tini -s subreaper",
  );

  // 2. Dockerfile checks (stage-aware, final USER enforcement, multi-arch safety, include_str! assets)
  const stages = dockerfile.split(/^(?=FROM\s+)/m).filter((s) => /^FROM\s+/m.test(s));
  const runtimeStage = stages.length > 0 ? stages[stages.length - 1] : dockerfile;

  check(
    "dockerfile.multiarch_args",
    dockerfile.includes("TARGETPLATFORM") && dockerfile.includes("TARGETARCH"),
    "Dockerfile must declare TARGETPLATFORM and TARGETARCH for multi-arch buildx",
  );
  const usesBuildPlatform = /FROM\s+--platform=\$BUILDPLATFORM\b/.test(dockerfile);
  const passesCargoCrossTarget = /cargo\s+build[\s\S]*--target\b/.test(dockerfile);
  check(
    "dockerfile.multiarch_target_arch_match",
    !usesBuildPlatform || passesCargoCrossTarget,
    "Dockerfile must not use FROM --platform=$BUILDPLATFORM without explicit cargo --target cross-compilation",
  );
  check(
    "dockerfile.include_str_assets",
    /COPY\s+packages\/builtin-skills\s+\.\/packages\/builtin-skills/.test(dockerfile) &&
      /COPY\s+packaging\/plugins\s+\.\/packaging\/plugins/.test(dockerfile),
    "Dockerfile must COPY packages/builtin-skills and packaging/plugins for compile-time include_str! assets",
  );
  check(
    "dockerfile.cargo_locked",
    /cargo\s+build[^\n]*--locked\b/.test(dockerfile),
    "Dockerfile must pass --locked to cargo build for reproducible dependency resolution",
  );
  check(
    "dockerfile.chromium_and_tini",
    /\bchromium\b/.test(runtimeStage) && /\btini\b/.test(runtimeStage),
    "Dockerfile runtime stage must install chromium and tini",
  );

  const runtimeUserMatches = [...runtimeStage.matchAll(/^\s*USER\s+(\S+)\s*$/gm)];
  const finalRuntimeUser =
    runtimeUserMatches.length > 0
      ? runtimeUserMatches[runtimeUserMatches.length - 1][1]
      : null;
  check(
    "dockerfile.final_nonroot_user",
    finalRuntimeUser === "10001:10001",
    `Dockerfile final runtime USER must be 10001:10001 (found: ${finalRuntimeUser ?? "none"})`,
  );
  check(
    "dockerfile.container_no_sandbox_env",
    dockerfile.includes("COWOK_CONTAINER_MODE=1") &&
      dockerfile.includes("COWOK_CHROMIUM_NO_SANDBOX=1"),
    "Dockerfile must set COWOK_CONTAINER_MODE=1 and COWOK_CHROMIUM_NO_SANDBOX=1 for hardened container Chromium",
  );
  check(
    "dockerfile.aliases",
    dockerfile.includes("/usr/local/bin/cowok-design") &&
      dockerfile.includes("/usr/local/bin/uiux-os"),
    "Dockerfile must install cowok-design and uiux-os alias",
  );
  check(
    "dockerfile.healthcheck",
    /HEALTHCHECK[\s\S]*\/api\/v1\/health/.test(runtimeStage),
    "Dockerfile must configure HEALTHCHECK targeting /api/v1/health",
  );
  check(
    "dockerfile.entrypoint_tini",
    dockerfile.includes(
      'ENTRYPOINT ["/usr/bin/tini", "-s", "--", "/usr/local/bin/cowok-entrypoint.sh"]',
    ),
    "Dockerfile must run entrypoint via /usr/bin/tini -s --",
  );
  check(
    "dockerfile.zero_libreoffice",
    !/(libreoffice|soffice)/i.test(dockerfile),
    "Dockerfile must contain ZERO LibreOffice/soffice packages or references (ADR-006)",
  );
  check(
    "dockerfile.zero_story_13_1",
    !/(eval_benchmark_runs|story[_-]?13[._]1)/i.test(dockerfile),
    "Dockerfile must have ZERO circular dependency on Story 13.1",
  );

  // 3. Entrypoint checks (syntax + hardening guards + zombie-safe reaping + atomic mktemp)
  const bashSyntax = spawnSync("bash", ["-n", entrypointPath], { encoding: "utf8" });
  check(
    "entrypoint.bash_syntax",
    bashSyntax.status === 0,
    bashSyntax.stderr?.trim() || "bash -n passed",
  );
  check(
    "entrypoint.strict_mode",
    entrypoint.includes("set -euo pipefail"),
    "entrypoint.sh must enable set -euo pipefail",
  );
  check(
    "entrypoint.sigterm_trap_and_reaper",
    entrypoint.includes("trap - TERM INT") &&
      entrypoint.includes("SELF_PGID") &&
      entrypoint.includes("is_pid_running_non_zombie") &&
      entrypoint.includes("trap 'reap_children TERM' TERM") &&
      entrypoint.includes("trap 'reap_children INT' INT"),
    "entrypoint.sh must trap TERM/INT, guard against self-PGID signaling, and use zombie-aware child polling",
  );
  check(
    "entrypoint.atomic_mktemp_probe",
    entrypoint.includes("mktemp") && !entrypoint.includes(".cowok-uid-probe.$$"),
    "entrypoint.sh must probe workspace writability atomically via mktemp",
  );
  check(
    "entrypoint.dev_shm_guard",
    entrypoint.includes("/dev/shm") &&
      entrypoint.includes("--disable-dev-shm-usage") &&
      entrypoint.includes("COWOK_CHROMIUM_DISABLE_DEV_SHM=1"),
    "entrypoint.sh must guard /dev/shm < 256 MiB with --disable-dev-shm-usage",
  );
  check(
    "entrypoint.readonly_wal_staging",
    entrypoint.includes('${RUNTIME_ROOT}/data') &&
      entrypoint.includes("XDG_DATA_HOME") &&
      entrypoint.includes("XDG_STATE_HOME") &&
      entrypoint.includes("XDG_CACHE_HOME"),
    "entrypoint.sh must stage XDG runtime dirs under ${RUNTIME_ROOT} for read-only mounts",
  );

  // 4. docker-compose.ci.yml per-service hardening & prohibited ports
  const services = parseComposeServices(compose);
  check(
    "compose.services_present",
    services.size >= 2,
    `docker-compose.ci.yml must define daemon and ci-gate services (found ${services.size})`,
  );

  for (const [svcName, svcLines] of services.entries()) {
    const block = svcLines.join("\n");
    check(
      `compose.${svcName}.nonroot_user`,
      /^\s+user:\s*"10001:10001"\s*$/m.test(block),
      `service '${svcName}' must enforce user: "10001:10001"`,
    );
    check(
      `compose.${svcName}.read_only`,
      /^\s+read_only:\s*true\s*$/m.test(block),
      `service '${svcName}' must enforce read_only: true`,
    );
    check(
      `compose.${svcName}.no_new_privileges`,
      block.includes("no-new-privileges:true"),
      `service '${svcName}' must enforce no-new-privileges:true`,
    );
    check(
      `compose.${svcName}.cap_drop_all`,
      /cap_drop:\s*\n\s*-\s*"?ALL"?/.test(block),
      `service '${svcName}' must drop ALL Linux capabilities`,
    );
    check(
      `compose.${svcName}.no_double_init`,
      !/^\s+init:\s*true\s*$/m.test(block),
      `service '${svcName}' must omit redundant init: true because Dockerfile ENTRYPOINT uses tini -s`,
    );
    check(
      `compose.${svcName}.shm_and_mem`,
      /shm_size:\s*"(256m|512m)"/.test(block) && /mem_limit:\s*"/.test(block),
      `service '${svcName}' must configure shm_size (>=256m) and mem_limit`,
    );
    check(
      `compose.${svcName}.tmpfs_and_container_env`,
      block.includes("/tmp:rw,noexec,nosuid") &&
        block.includes('COWOK_CONTAINER_MODE: "1"') &&
        block.includes('COWOK_CHROMIUM_NO_SANDBOX: "1"'),
      `service '${svcName}' must mount /tmp tmpfs and set COWOK_CONTAINER_MODE/COWOK_CHROMIUM_NO_SANDBOX`,
    );
  }

  check(
    "compose.loopback_port_publish",
    compose.includes("127.0.0.1:4170:4170"),
    "docker-compose.ci.yml must publish host port strictly on 127.0.0.1:4170:4170",
  );

  const prohibitedPorts = contract.endpoints?.prohibitedPorts ?? [];
  const exposedTargets = `${dockerfile}\n${compose}`;
  const foundProhibited = [];
  for (const port of prohibitedPorts) {
    const portRe = new RegExp(`(?:[:=]|EXPOSE\\s+|PORT[:=]\\s*["']?)${port}\\b`, "i");
    if (portRe.test(exposedTargets)) {
      foundProhibited.push(port);
    }
  }
  check(
    "ports.zero_prohibited",
    foundProhibited.length === 0,
    foundProhibited.length === 0
      ? "zero prohibited infrastructure ports referenced"
      : `prohibited ports detected: ${foundProhibited.join(", ")}`,
  );

  // 5. Fail-closed check for ci_gate.rs & zero circular dependency on Story 13.1
  const ciGateExists = fs.existsSync(ciGateRsPath);
  check(
    "rust.ci_gate_exists",
    ciGateExists,
    ciGateExists
      ? "crates/harness-daemon/src/ci_gate.rs present"
      : `missing required Rust implementation file: ${ciGateRsPath}`,
  );
  if (ciGateExists) {
    const ciGateRs = readUtf8(ciGateRsPath);
    check(
      "rust.zero_story_13_1_dep",
      !/(eval_benchmark_runs|story_13_1|story-13\.1)/i.test(ciGateRs),
      "ci_gate.rs must have ZERO imports or references to Story 13.1",
    );
  }

  const report = {
    ok: violations.length === 0,
    storyId: contract.storyId,
    image: contract.image,
    checksTotal: checks.length,
    checksPassed: checks.filter((c) => c.passed).length,
    violations,
    checks,
  };

  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (violations.length > 0) {
    process.exit(1);
  }
}

main();
