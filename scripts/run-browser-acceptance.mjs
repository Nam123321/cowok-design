#!/usr/bin/env node
/**
 * Direct Browser Acceptance Runner for Story 04.1
 *
 * Bootstraps isolated harness-daemon + Playwright Chromium browser session,
 * records full acceptance video (.webm / .mp4) & captures milestone screenshots,
 * and outputs structured acceptance report.
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
  console.log(`\n▶ [acceptance-runner] ${label}`);
  execSync(cmd, {
    cwd: REPO_ROOT,
    stdio: 'inherit',
    env: { ...process.env, ...extraEnv },
  });
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
        console.log(`✅ [acceptance-runner] harness-daemon healthy on port ${port} (attempt ${i}/30).`);
        return;
      }
    } catch {
      // Daemon socket opening
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  const logTail = fs.existsSync(logPath) ? fs.readFileSync(logPath, 'utf8') : '';
  throw new Error(`harness-daemon health check timed out on ${url}:\n${logTail}`);
}

async function main() {
  console.log('======================================================================');
  console.log('🚀 COWOK-DESIGN: BROWSER ACCEPTANCE SUITE — STORY 04.1');
  console.log('======================================================================');

  // 1. Assign ports
  let fePort = 3100;
  let bePort = 3200;
  try {
    const portOut = execSync('python3 .agent/scripts/worktree-port-manager.py assign 04.1 "' + REPO_ROOT + '"', {
      cwd: REPO_ROOT,
      text: true,
    });
    console.log(portOut.trim());
  } catch (err) {
    console.warn(`[WARN] Could not assign via worktree-port-manager, using fallback ports ${fePort}/${bePort}`);
  }

  // 2. Ensure frontend bundle is fresh
  console.log('\n▶ [acceptance-runner] Checking production frontend build...');
  runStep('Build production frontend bundle', 'pnpm run build');

  // 3. Isolated temp workspace for SQLite WAL
  const tempWorkspace = fs.mkdtempSync(path.join(os.tmpdir(), 'cowok-browser-acceptance-'));
  const daemonLogPath = path.join(tempWorkspace, 'harness-daemon.log');
  const logFd = fs.openSync(daemonLogPath, 'w');
  const daemonBin = path.join(REPO_ROOT, 'target', 'debug', 'harness-daemon');

  console.log(`\n▶ [acceptance-runner] Launching isolated harness-daemon (port=${bePort}, workspace=${tempWorkspace})`);
  const daemonProc = spawn(daemonBin, [], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      UXUI_OS_WORKSPACE_ROOT: tempWorkspace,
      UXUI_OS_DAEMON_PORT: String(bePort),
    },
    stdio: ['ignore', logFd, logFd],
  });

  let testExitCode = 0;
  try {
    await waitForDaemonHealth(bePort, daemonProc, daemonLogPath);

    console.log(`\n▶ [acceptance-runner] Running Playwright Acceptance Test against http://127.0.0.1:${fePort}/ ...`);
    runStep(
      'Playwright Browser Test with Video Recording',
      'pnpm exec playwright test tests/e2e/acceptance-story-04.1.spec.ts --config=playwright.ui.config.ts',
      {
        CI: 'true',
        COWOK_ACCEPTANCE_RUN: '1',
        TARGET_PORT: String(fePort),
        UXUI_OS_WORKSPACE_ROOT: tempWorkspace,
        UXUI_OS_DAEMON_PORT: String(bePort),
      }
    );
  } catch (err) {
    testExitCode = err.status || 1;
    console.error(`\n❌ [acceptance-runner] Playwright execution exited with code ${testExitCode}`);
  } finally {
    if (daemonProc.exitCode === null) {
      daemonProc.kill('SIGTERM');
      console.log('🛑 [acceptance-runner] Terminated harness-daemon.');
    }
    try {
      execSync('python3 .agent/scripts/worktree-port-manager.py release 04.1', {
        cwd: REPO_ROOT,
        stdio: 'ignore',
      });
      console.log('🔓 [acceptance-runner] Released worktree ports.');
    } catch {
      // Ignore release error
    }
  }

  process.exit(testExitCode);
}

main();
