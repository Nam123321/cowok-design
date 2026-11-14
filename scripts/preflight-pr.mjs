#!/usr/bin/env node
/**
 * Clean-Runner PR Preflight Parity Script
 *
 * Ensures local verification matches fresh GitHub Actions runners BEFORE
 * Watchmen ECC sealing, git push, or `gh pr create`:
 *   1. Frozen lockfile check (`pnpm install --frozen-lockfile --lockfile-only`)
 *   2. Rust format check (`cargo fmt --all --check`)
 *   3. Clean `@cowok-design/shared-types` build + app `typecheck` (wipes stale `dist/` first)
 *   4. Rust `cargo clippy --workspace --all-targets -- -D warnings` & Vitest (`pnpm run test`) unless `--fast`
 *   5. Optional `--with-e2e`: boots `harness-daemon` against an isolated `mktemp` workspace/SQLite
 *      with deterministic `/api/v1/health` polling and `CI=true` Playwright verification.
 */

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '..');

function runStep(label, cmd, extraEnv = {}) {
  console.log(`\n▶ [preflight] ${label}`);
  execSync(cmd, {
    cwd: REPO_ROOT,
    stdio: 'inherit',
    env: { ...process.env, ...extraEnv },
  });
}

function cleanSharedTypesDist() {
  const distDir = path.join(REPO_ROOT, 'packages', 'shared-types', 'dist');
  if (fs.existsSync(distDir)) {
    fs.rmSync(distDir, { recursive: true, force: true });
    console.log('🧹 [preflight] Cleared stale packages/shared-types/dist for clean-runner parity.');
  }
}

async function waitForDaemonHealth(port, childProc, logPath) {
  const url = `http://127.0.0.1:${port}/api/v1/health`;
  for (let i = 1; i <= 30; i++) {
    if (childProc.exitCode !== null) {
      const logTail = fs.existsSync(logPath) ? fs.readFileSync(logPath, 'utf8') : '';
      throw new Error(`harness-daemon exited prematurely (${childProc.exitCode}):\n${logTail}`);
    }
    try {
      const res = await fetch(url);
      if (res.ok) {
        console.log(`✅ [preflight] harness-daemon healthy on port ${port} (attempt ${i}/30).`);
        return;
      }
    } catch {
      // Daemon still binding loopback socket
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  const logTail = fs.existsSync(logPath) ? fs.readFileSync(logPath, 'utf8') : '';
  throw new Error(`harness-daemon health check timed out on ${url}:\n${logTail}`);
}

async function runIsolatedE2E() {
  runStep('Build harness-daemon binary', 'cargo build --bin harness-daemon');
  runStep('Build production frontend bundle', 'pnpm run build');

  const tempWorkspace = fs.mkdtempSync(path.join(os.tmpdir(), 'cowok-e2e-clean-'));
  const daemonPort = process.env.UXUI_OS_DAEMON_PORT || '4179';
  const uiPort = process.env.TARGET_PORT || '4174';
  const daemonLogPath = path.join(tempWorkspace, 'harness-daemon.log');
  const logFd = fs.openSync(daemonLogPath, 'w');
  const daemonBin = path.join(REPO_ROOT, 'target', 'debug', 'harness-daemon');

  console.log(`\n▶ [preflight] Launching isolated harness-daemon (port=${daemonPort}, workspace=${tempWorkspace})`);
  const daemonProc = spawn(daemonBin, [], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      UXUI_OS_WORKSPACE_ROOT: tempWorkspace,
      UXUI_OS_DAEMON_PORT: daemonPort,
    },
    stdio: ['ignore', logFd, logFd],
  });

  try {
    await waitForDaemonHealth(daemonPort, daemonProc, daemonLogPath);
    runStep(
      'Run Playwright E2E against isolated virgin SQLite (CI=true)',
      'pnpm exec playwright test --config=playwright.ui.config.ts',
      {
        CI: 'true',
        TARGET_PORT: uiPort,
        UXUI_OS_WORKSPACE_ROOT: tempWorkspace,
        UXUI_OS_DAEMON_PORT: daemonPort,
      }
    );
  } finally {
    if (daemonProc.exitCode === null) {
      daemonProc.kill('SIGTERM');
    }
    fs.closeSync(logFd);
    fs.rmSync(tempWorkspace, { recursive: true, force: true });
  }
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const fastMode = args.has('--fast');
  const withE2E = args.has('--with-e2e');

  console.log(`🛡️ [preflight-pr] Mode: ${fastMode ? 'FAST (<10s)' : withE2E ? 'FULL + ISOLATED E2E' : 'STANDARD PR PARITY'}`);

  runStep('Verify pnpm lockfile parity', 'pnpm install --frozen-lockfile --lockfile-only --ignore-scripts');
  runStep('Check Rust formatting', 'cargo fmt --all --check');

  cleanSharedTypesDist();
  runStep('Clean shared-types build & app typecheck', 'pnpm run typecheck');

  if (!fastMode) {
    runStep('Run Rust clippy (-D warnings)', 'cargo clippy --workspace --all-targets -- -D warnings');
    runStep('Run Vitest suite', 'pnpm run test');
  }

  if (withE2E) {
    await runIsolatedE2E();
  }

  console.log('\n✅ [preflight-pr] Clean-runner parity checks PASSED. Safe to Watchmen-seal & push.');
}

main().catch((err) => {
  console.error('\n❌ [preflight-pr] Failed:', err?.message || err);
  process.exit(1);
});
