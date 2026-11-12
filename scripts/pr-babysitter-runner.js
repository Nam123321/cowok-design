#!/usr/bin/env node
/**
 * 🤖 PR Babysitter Safe Runner (v2.0)
 * 
 * Features:
 * 1. 3-Strike Circuit Breaker (EC-HARNESS-P2-001) - Stops runaway bot loops.
 * 2. Fork PR Privilege Separation (PwnRequest Defense) - Strictly Read-Only for fork PRs.
 * 3. Fencing Token & Anti-Loop Detection - Ignores commits authored by github-actions[bot].
 * 4. State Persistence into _iwish-output/pr-babysitter-state.md.
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '..');
const STATE_FILE = path.join(REPO_ROOT, '_iwish-output', 'pr-babysitter-state.md');
const MAX_ATTEMPTS = 3;

/**
 * Ensures state file exists and has headers.
 */
function ensureStateFile() {
  const dir = path.dirname(STATE_FILE);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  if (!fs.existsSync(STATE_FILE)) {
    const header = `# 🤖 PR Babysitter Runtime Ledger & Fencing State

- **Trạng thái Circuit Breaker**: \`ARMED\`
- **Ngưỡng tối đa (Max Attempts)**: \`${MAX_ATTEMPTS}\`
- **Chính sách an toàn**: Tách quyền (Fork PR chỉ Review, Internal PR Auto-Fix)

| PR # | Branch | Lỗi Phát Hiện | Lần Thử | Trạng Thái Xử Lý | Fencing Commit SHA |
| :---: | :--- | :--- | :---: | :---: | :--- |
`;
    fs.writeFileSync(STATE_FILE, header, 'utf8');
  }
}

/**
 * Reads the attempt count for the current PR from state file.
 * @param {string|number} prNumber 
 * @returns {number}
 */
function getPrAttempts(prNumber) {
  ensureStateFile();
  const content = fs.readFileSync(STATE_FILE, 'utf8');
  const lines = content.split('\n');
  let count = 0;
  const prPattern = new RegExp(`^\\|\\s*#?${prNumber}\\s*\\|`);
  for (const line of lines) {
    if (prPattern.test(line)) {
      count++;
    }
  }
  return count;
}

/**
 * Appends a triage record to the state ledger.
 */
function recordPrState(prNumber, branch, errorDesc, attempt, status, commitSha) {
  ensureStateFile();
  const row = `| #${prNumber} | \`${branch}\` | ${errorDesc} | \`${attempt}/${MAX_ATTEMPTS}\` | **${status}** | \`${commitSha || 'N/A'}\` |\n`;
  fs.appendFileSync(STATE_FILE, row, 'utf8');
}

/**
 * Main Babysitter entrypoint.
 */
export async function runPrBabysitter() {
  console.log('🤖 [PR-BABYSITTER] Starting Safe PR Babysitter Agent...');
  
  const isFork = process.env.IS_FORK === 'true';
  const prNumber = process.env.PR_NUMBER || 'LOCAL-DEV';
  const branch = process.env.GITHUB_HEAD_REF || 'local-branch';
  const actor = process.env.GITHUB_ACTOR || 'developer';

  // 1. Check Anti-Loop: If trigger came from github-actions[bot], halt immediately.
  if (actor === 'github-actions[bot]') {
    console.log('🛑 [PR-BABYSITTER] Triggered by bot commit. Halting to prevent infinite loop recursion.');
    return { status: 'skipped', reason: 'bot-author' };
  }

  // 2. Check 3-Strike Circuit Breaker
  const currentAttempts = getPrAttempts(prNumber);
  console.log(`📊 [PR-BABYSITTER] PR #${prNumber} Attempt Count: ${currentAttempts}/${MAX_ATTEMPTS}`);

  if (currentAttempts >= MAX_ATTEMPTS) {
    console.warn(`🚨 [CIRCUIT-BREAKER TRIPPED] PR #${prNumber} reached maximum retry limit (${MAX_ATTEMPTS}).`);
    recordPrState(prNumber, branch, 'Max Retries Exceeded', currentAttempts + 1, 'CIRCUIT_BREAKER_LOCKED', 'LOCKED');
    console.log('🛑 Halting PR Babysitter. Manual human maintainer review required.');
    return { status: 'locked', reason: 'max-attempts-exceeded' };
  }

  // 3. PwnRequest Isolation: Check if PR is from an external Fork
  if (isFork) {
    console.log('⚠️ [PR-BABYSITTER] External Fork PR detected. Operating in READ-ONLY mode (PwnRequest Defense).');
    recordPrState(prNumber, branch, 'External Fork PR', currentAttempts + 1, 'READ_ONLY_TRIAGE', 'NONE');
    console.log('✅ Fork PR triage completed without running untrusted scripts or write actions.');
    return { status: 'read-only-triage', isFork: true };
  }

  // 4. Internal Branch Auto-Triage & Safe Formatting
  console.log('🔧 [PR-BABYSITTER] Running Local Verification Suite...');
  let fixedSomething = false;
  let errorSummary = 'Clean';

  try {
    // Check if linter or typecheck issues can be auto-formatted safely
    console.log('   ↳ Checking Rust formatting (cargo fmt --all --check)...');
    try {
      execSync('cargo fmt --all --check', { cwd: REPO_ROOT, stdio: 'pipe' });
    } catch {
      console.log('   ↳ Auto-formatting Rust code (cargo fmt --all)...');
      execSync('cargo fmt --all', { cwd: REPO_ROOT, stdio: 'pipe' });
      fixedSomething = true;
      errorSummary = 'Auto-repaired cargo fmt';
    }

    // Check workspace hygiene
    console.log('   ↳ Checking Workspace Hygiene (workspace-janitor.py)...');
    try {
      execSync('python3 .agent/scripts/workspace-janitor.py --auto-clean --enforce-structure', { cwd: REPO_ROOT, stdio: 'pipe' });
    } catch (err) {
      console.warn('   ⚠️ Workspace janitor sweep non-zero:', err.message);
    }

    // Check git diff
    const statusOutput = execSync('git status -s', { cwd: REPO_ROOT, encoding: 'utf8' }).trim();
    if (statusOutput && fixedSomething) {
      console.log('📝 [PR-BABYSITTER] Safe auto-fixes applied. Changes detected:');
      console.log(statusOutput);

      // In CI, commit and push with [skip ci] [babysitter]
      if (process.env.CI) {
        execSync('git config user.name "github-actions[bot]"', { cwd: REPO_ROOT });
        execSync('git config user.email "github-actions[bot]@users.noreply.github.com"', { cwd: REPO_ROOT });
        execSync('git add -A', { cwd: REPO_ROOT });
        execSync(`git commit -m "chore(babysitter): auto-repair formatting and hygiene [skip ci] [babysitter]"`, { cwd: REPO_ROOT });
        const commitSha = execSync('git rev-parse --short HEAD', { cwd: REPO_ROOT, encoding: 'utf8' }).trim();
        execSync('git push', { cwd: REPO_ROOT });
        recordPrState(prNumber, branch, errorSummary, currentAttempts + 1, 'AUTO_FIXED_COMMITTED', commitSha);
        console.log(`✅ [PR-BABYSITTER] Auto-fix committed and pushed: ${commitSha}`);
        return { status: 'auto-fixed', commitSha };
      } else {
        recordPrState(prNumber, branch, errorSummary, currentAttempts + 1, 'LOCAL_FIX_VERIFIED', 'LOCAL');
        console.log('✅ [PR-BABYSITTER] Local dry-run verification completed.');
        return { status: 'local-dry-run-ok' };
      }
    } else {
      recordPrState(prNumber, branch, 'No auto-fixable errors detected', currentAttempts + 1, 'VERIFIED_PRISTINE', 'NONE');
      console.log('✨ [PR-BABYSITTER] Codebase is pristine. No auto-fixes needed.');
      return { status: 'pristine' };
    }
  } catch (err) {
    console.error('❌ [PR-BABYSITTER] Error during babysitter execution:', err.message);
    recordPrState(prNumber, branch, err.message.slice(0, 50), currentAttempts + 1, 'TRIAGE_FAILED', 'ERROR');
    return { status: 'error', error: err.message };
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  runPrBabysitter().then(res => {
    console.log('🏁 [PR-BABYSITTER] Exit Status:', res.status);
    process.exit(0);
  }).catch(err => {
    console.error('💥 [PR-BABYSITTER] Fatal error:', err);
    process.exit(1);
  });
}
