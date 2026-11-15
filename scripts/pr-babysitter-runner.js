#!/usr/bin/env node
/**
 * PR Babysitter Safe Triage & Self-Healing Runner (v3.0)
 *
 * Canary & Land-and-Deploy Safeguards:
 * 1. Read-Only Triage by Default (`BABYSITTER_AUTO_PUSH=false`): prevents remote bot commits from
 *    colliding with local developer edits or invalidating Watchmen ECC `code_hash` seals.
 * 2. Draft & Fork Guard: never pushes commits to external fork PRs or Draft PRs even if enabled.
 * 3. No `[skip ci]` Blind Spots: when `BABYSITTER_AUTO_PUSH=true` is explicitly opted in, pushes
 *    trigger normal CI; infinite loops are blocked by `github.actor != 'github-actions[bot]'`
 *    plus a 3-strike circuit breaker backed by PR comment ledger + git log `[babysitter]` history.
 * 4. Fail-Closed Exit Codes: `error` and `locked` exit with code 1 instead of masking failures.
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const DEFAULT_REPO_ROOT = path.resolve(__dirname, '..');
const MAX_ATTEMPTS = 3;
const LEDGER_MARKER = '<!-- pr-babysitter-ledger -->';

export function sanitizeMessage(msg) {
  if (!msg) return '';
  return String(msg)
    .replace(/AIza[0-9A-Za-z-_]{35}/g, '[REDACTED_GEMINI_KEY]')
    .replace(/ghp_[0-9A-Za-z]{36}/g, '[REDACTED_GH_TOKEN]')
    .replace(/ghs_[0-9A-Za-z]{36}/g, '[REDACTED_GH_APP_TOKEN]')
    .replace(/(Bearer\s+)[A-Za-z0-9._~+/-]+=*/gi, '$1[REDACTED_BEARER]');
}

function getStateFile(repoRoot, customStateFile) {
  return customStateFile || path.join(repoRoot, '_iwish-output', 'pr-babysitter-state.md');
}

function ensureStateFile(stateFile, fsApi = fs) {
  const dir = path.dirname(stateFile);
  if (!fsApi.existsSync(dir)) {
    fsApi.mkdirSync(dir, { recursive: true });
  }
  if (!fsApi.existsSync(stateFile)) {
    const header = `# PR Babysitter Runtime Ledger & Fencing State

- **Circuit Breaker**: \`ARMED\`
- **Max Attempts**: \`${MAX_ATTEMPTS}\`
- **Default Mode**: \`READ_ONLY_TRIAGE\` (\`BABYSITTER_AUTO_PUSH=false\`)

| PR # | Branch | Diagnostic | Attempt | Status | Commit SHA |
| :---: | :--- | :--- | :---: | :---: | :--- |
`;
    fsApi.writeFileSync(stateFile, header, 'utf8');
  }
}

export function countPrAttemptsInMarkdown(content, prNumber) {
  if (!content) return 0;
  const escaped = String(prNumber).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const prPattern = new RegExp(`^\\|\\s*#?${escaped}\\s*\\|`, 'gm');
  return (content.match(prPattern) || []).length;
}

function getLocalAttempts(stateFile, prNumber, fsApi = fs) {
  ensureStateFile(stateFile, fsApi);
  const content = fsApi.readFileSync(stateFile, 'utf8');
  return countPrAttemptsInMarkdown(content, prNumber);
}

function recordLocalState(stateFile, prNumber, branch, errorDesc, attempt, status, commitSha, fsApi = fs) {
  ensureStateFile(stateFile, fsApi);
  const cleanDesc = sanitizeMessage(errorDesc).replace(/\|/g, '/').slice(0, 100);
  const row = `| #${prNumber} | \`${branch}\` | ${cleanDesc} | \`${attempt}/${MAX_ATTEMPTS}\` | **${status}** | \`${commitSha || 'N/A'}\` |\n`;
  fsApi.appendFileSync(stateFile, row, 'utf8');
}

function getGitBabysitterCommitCount(repoRoot, baseRef, execFn = execSync) {
  try {
    const safeBaseRef = baseRef ? String(baseRef).replace(/[^A-Za-z0-9._/-]/g, '') : '';
    const cmd = safeBaseRef
      ? `git log origin/${safeBaseRef}..HEAD --grep="\\[babysitter\\]" --format="%H"`
      : `git log -n 20 --grep="\\[babysitter\\]" --format="%H"`;
    const out = execFn(cmd, { cwd: repoRoot, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }).trim();
    return out ? out.split('\n').filter(Boolean).length : 0;
  } catch (err) {
    console.warn(`[pr-babysitter] Warning inspecting git babysitter commits: ${sanitizeMessage(err instanceof Error ? err.message : String(err))}`);
    return 0;
  }
}

const MAX_COMMENT_PAGES = 5;

async function findExistingLedgerComment(baseUrl, headers, fetchFn) {
  for (let page = 1; page <= MAX_COMMENT_PAGES; page += 1) {
    const pageUrl = `${baseUrl}?per_page=100&page=${page}`;
    const res = await fetchFn(pageUrl, { headers });
    if (!res.ok) {
      console.warn(`[pr-babysitter] Warning fetching PR comments page ${page}: HTTP ${res.status}`);
      return null;
    }
    const comments = await res.json();
    if (!Array.isArray(comments) || comments.length === 0) {
      return null;
    }
    const found = comments.find((c) => typeof c?.body === 'string' && c.body.includes(LEDGER_MARKER));
    if (found) {
      return found;
    }
    if (comments.length < 100) {
      return null;
    }
  }
  return null;
}

async function syncGitHubLedger({ env, fetchFn, prNumber, branch, diagnostic, status, commitSha, readOnly = false }) {
  const token = env.GITHUB_TOKEN;
  const repo = env.GITHUB_REPOSITORY;
  if (!token || !repo || !/^\d+$/.test(String(prNumber)) || typeof fetchFn !== 'function') {
    return { remoteAttempts: 0 };
  }

  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'User-Agent': 'cowok-design-pr-babysitter',
  };
  const commentsBaseUrl = `https://api.github.com/repos/${repo}/issues/${prNumber}/comments`;

  try {
    const existing = await findExistingLedgerComment(commentsBaseUrl, headers, fetchFn);
    const priorBody = existing?.body || `${LEDGER_MARKER}\n### PR Babysitter Triage Ledger\n\n| PR # | Branch | Diagnostic | Attempt | Status | Commit |\n| :---: | :--- | :--- | :---: | :---: | :--- |\n`;
    const remoteAttempts = countPrAttemptsInMarkdown(priorBody, prNumber);

    if (!diagnostic || readOnly) {
      return { remoteAttempts };
    }

    const nextAttempt = remoteAttempts + 1;
    const cleanDiag = sanitizeMessage(diagnostic).replace(/\|/g, '/').slice(0, 120);
    const nextRow = `| #${prNumber} | \`${branch}\` | ${cleanDiag} | \`${nextAttempt}/${MAX_ATTEMPTS}\` | **${status}** | \`${commitSha || 'N/A'}\` |\n`;
    const nextBody = `${priorBody}${priorBody.endsWith('\n') ? '' : '\n'}${nextRow}`;

    const targetUrl = existing
      ? `https://api.github.com/repos/${repo}/issues/comments/${existing.id}`
      : commentsBaseUrl;
    await fetchFn(targetUrl, {
      method: existing ? 'PATCH' : 'POST',
      headers: { ...headers, 'Content-Type': 'application/json' },
      body: JSON.stringify({ body: nextBody }),
    });

    return { remoteAttempts };
  } catch (err) {
    console.warn(`[pr-babysitter] Warning syncing GitHub ledger: ${sanitizeMessage(err instanceof Error ? err.message : String(err))}`);
    return { remoteAttempts: 0 };
  }
}

function appendStepSummary(env, markdown, fsApi = fs) {
  if (!env.GITHUB_STEP_SUMMARY) return;
  try {
    fsApi.appendFileSync(env.GITHUB_STEP_SUMMARY, `${markdown}\n`, 'utf8');
  } catch (err) {
    console.warn(`[pr-babysitter] Warning writing GITHUB_STEP_SUMMARY: ${err instanceof Error ? err.message : String(err)}`);
  }
}

function checkRustFormatting(repoRoot, execFn = execSync) {
  try {
    execFn('cargo fmt --all --check', { cwd: repoRoot, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
    return { needsFormat: false, details: '' };
  } catch (err) {
    const out = `${err?.stdout || ''}\n${err?.stderr || ''}`.trim();
    if (/not found|ENOENT|is not recognized/i.test(out)) {
      throw new Error(`cargo fmt unavailable: ${out}`);
    }
    return { needsFormat: true, details: out.slice(0, 1000) };
  }
}

function runOptionalWorkspaceJanitor(repoRoot, fsApi = fs, execFn = execSync) {
  const janitorScript = path.join(repoRoot, '.agent', 'scripts', 'workspace-janitor.py');
  if (!fsApi.existsSync(janitorScript)) {
    console.log('   ↳ Skipping workspace-janitor.py (.agent/ is absent in this checkout).');
    return;
  }
  execFn(`python3 "${janitorScript}" --auto-clean --enforce-structure`, {
    cwd: repoRoot,
    stdio: 'pipe',
  });
}

/**
 * Runs PR Babysitter triage/remediation with dependency injection for testing.
 */
export async function runPrBabysitter(options = {}) {
  const env = options.env || process.env;
  const repoRoot = options.repoRoot || DEFAULT_REPO_ROOT;
  const fsApi = options.fsApi || fs;
  const execFn = options.execFn || execSync;
  const fetchFn = options.fetchFn ?? globalThis.fetch;
  const stateFile = getStateFile(repoRoot, options.stateFile);

  const isFork = env.IS_FORK === 'true';
  const isDraft = env.IS_DRAFT === 'true';
  const autoPushEnabled = env.BABYSITTER_AUTO_PUSH === 'true';
  const prNumber = env.PR_NUMBER || 'LOCAL-DEV';
  const branch = env.GITHUB_HEAD_REF || 'local-branch';
  const actor = env.GITHUB_ACTOR || 'developer';

  console.log('🤖 [PR-BABYSITTER] Starting Safe Triage Runner (v3.0)...');

  if (actor === 'github-actions[bot]') {
    console.log('🛑 [PR-BABYSITTER] Bot-authored trigger blocked to prevent loops.');
    return { status: 'skipped', reason: 'bot-author', exitCode: 0 };
  }

  const localAttempts = getLocalAttempts(stateFile, prNumber, fsApi);
  const gitAttempts = getGitBabysitterCommitCount(repoRoot, env.GITHUB_BASE_REF, execFn);
  const { remoteAttempts } = await syncGitHubLedger({ env, fetchFn, prNumber, branch });
  const currentAttempts = Math.max(localAttempts, gitAttempts, remoteAttempts);

  console.log(`📊 [PR-BABYSITTER] PR #${prNumber} Attempts: ${currentAttempts}/${MAX_ATTEMPTS} (autoPush=${autoPushEnabled}, fork=${isFork}, draft=${isDraft})`);

  if (isFork) {
    console.log('⚠️ [PR-BABYSITTER] External Fork PR detected -> READ-ONLY triage (no remote mutation).');
    recordLocalState(stateFile, prNumber, branch, 'External Fork PR', currentAttempts + 1, 'READ_ONLY_TRIAGE', 'NONE', fsApi);
    return { status: 'read-only-triage', isFork: true, exitCode: 0 };
  }

  if (currentAttempts >= MAX_ATTEMPTS) {
    console.warn(`🚨 [CIRCUIT-BREAKER TRIPPED] PR #${prNumber} reached max limit (${MAX_ATTEMPTS}).`);
    recordLocalState(stateFile, prNumber, branch, 'Circuit breaker tripped', currentAttempts + 1, 'CIRCUIT_BREAKER_LOCKED', 'LOCKED', fsApi);
    await syncGitHubLedger({
      env,
      fetchFn,
      prNumber,
      branch,
      diagnostic: `Circuit breaker tripped (${currentAttempts}/${MAX_ATTEMPTS})`,
      status: 'CIRCUIT_BREAKER_LOCKED',
      commitSha: 'LOCKED',
    });
    return { status: 'locked', reason: 'max-attempts-exceeded', exitCode: 1 };
  }

  try {
    runOptionalWorkspaceJanitor(repoRoot, fsApi, execFn);
    const fmtResult = checkRustFormatting(repoRoot, execFn);

    if (!fmtResult.needsFormat) {
      recordLocalState(stateFile, prNumber, branch, 'Codebase formatting verified pristine', currentAttempts + 1, 'VERIFIED_PRISTINE', 'NONE', fsApi);
      console.log('✨ [PR-BABYSITTER] Formatting pristine. No action needed.');
      return { status: 'pristine', exitCode: 0 };
    }

    const canAutoMutate = Boolean(env.CI) && autoPushEnabled && !isDraft;
    if (!canAutoMutate) {
      const modeReason = isDraft ? 'Draft PR guard active' : 'Read-only default (`BABYSITTER_AUTO_PUSH=false`)';
      const diagMsg = `Rust formatting drift detected (${modeReason}). Run \`cargo fmt --all\` BEFORE Watchmen ECC sealing.`;
      console.warn(`::warning::${diagMsg}`);
      appendStepSummary(
        env,
        `### PR Babysitter Advisory\n- **Mode**: ${modeReason}\n- **Finding**: \`cargo fmt --all --check\` failed.\n- **Action**: Run \`cargo fmt --all\` locally before Watchmen ECC sealing & push.\n\n\`\`\`\n${fmtResult.details}\n\`\`\`\n`,
        fsApi
      );
      recordLocalState(stateFile, prNumber, branch, diagMsg, currentAttempts + 1, 'TRIAGE_ADVISORY', 'NONE', fsApi);
      await syncGitHubLedger({
        env,
        fetchFn,
        prNumber,
        branch,
        diagnostic: diagMsg,
        status: 'TRIAGE_ADVISORY',
        commitSha: 'NONE',
      });
      return { status: 'triage-advisory', needsFormat: true, exitCode: 0 };
    }

    console.log('🔧 [PR-BABYSITTER] Applying opt-in `cargo fmt --all` auto-repair...');
    execFn('cargo fmt --all', { cwd: repoRoot, stdio: 'pipe' });
    const statusOutput = execFn('git status -s', { cwd: repoRoot, encoding: 'utf8' }).trim();
    if (!statusOutput) {
      return { status: 'pristine', exitCode: 0 };
    }

    execFn('git config user.name "github-actions[bot]"', { cwd: repoRoot });
    execFn('git config user.email "github-actions[bot]@users.noreply.github.com"', { cwd: repoRoot });
    execFn('git add -u', { cwd: repoRoot });
    execFn('git commit -m "chore(babysitter): auto-repair Rust formatting [babysitter]"', { cwd: repoRoot });
    const commitSha = execFn('git rev-parse --short HEAD', { cwd: repoRoot, encoding: 'utf8' }).trim();
    execFn('git push', { cwd: repoRoot });

    recordLocalState(stateFile, prNumber, branch, 'Auto-repaired cargo fmt', currentAttempts + 1, 'AUTO_FIXED_COMMITTED', commitSha, fsApi);
    await syncGitHubLedger({
      env,
      fetchFn,
      prNumber,
      branch,
      diagnostic: 'Auto-repaired cargo fmt',
      status: 'AUTO_FIXED_COMMITTED',
      commitSha,
    });
    return { status: 'auto-fixed', commitSha, exitCode: 0 };
  } catch (err) {
    const cleanError = sanitizeMessage(err?.message || 'Unknown error');
    console.error('❌ [PR-BABYSITTER] Runner failed:', cleanError);
    recordLocalState(stateFile, prNumber, branch, cleanError, currentAttempts + 1, 'TRIAGE_FAILED', 'ERROR', fsApi);
    return { status: 'error', error: cleanError, exitCode: 1 };
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  runPrBabysitter()
    .then((res) => {
      console.log('🏁 [PR-BABYSITTER] Exit Status:', res.status);
      process.exit(typeof res.exitCode === 'number' ? res.exitCode : 0);
    })
    .catch((err) => {
      console.error('💥 [PR-BABYSITTER] Fatal error:', sanitizeMessage(err?.message || err));
      process.exit(1);
    });
}
