import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { countPrAttemptsInMarkdown, runPrBabysitter } from '../../scripts/pr-babysitter-runner.js';

const tempDirs: string[] = [];

function createTempRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'babysitter-test-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir && fs.existsSync(dir)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
});

describe('scripts/pr-babysitter-runner.js', () => {
  it('blocks bot-authored triggers cleanly without touching git or failing', async () => {
    const res = await runPrBabysitter({
      env: { GITHUB_ACTOR: 'github-actions[bot]' },
    });
    expect(res).toEqual({ status: 'skipped', reason: 'bot-author', exitCode: 0 });
  });

  it('defaults to read-only triage (BABYSITTER_AUTO_PUSH unset) when cargo fmt fails', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const summaryFile = path.join(repoRoot, 'summary.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) return '';
      if (cmd === 'cargo fmt --all --check') {
        const err = new Error('Diff in crates/domain/src/lib.rs') as Error & { stdout?: string };
        err.stdout = 'Diff in crates/domain/src/lib.rs';
        throw err;
      }
      throw new Error(`Unexpected mutating command: ${cmd}`);
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        PR_NUMBER: '99',
        GITHUB_HEAD_REF: 'feat/test',
        GITHUB_STEP_SUMMARY: summaryFile,
      },
      execFn,
    });

    expect(res.status).toBe('triage-advisory');
    expect(res.needsFormat).toBe(true);
    expect(res.exitCode).toBe(0);
    expect(fs.readFileSync(summaryFile, 'utf8')).toMatch(/before Watchmen ECC sealing/i);
    expect(execFn).not.toHaveBeenCalledWith('cargo fmt --all', expect.anything());
    expect(execFn).not.toHaveBeenCalledWith('git push', expect.anything());
  });

  it('blocks auto-push on Draft PRs even when BABYSITTER_AUTO_PUSH=true', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) return '';
      if (cmd === 'cargo fmt --all --check') {
        throw new Error('unformatted');
      }
      throw new Error(`Should not execute mutating command on draft PR: ${cmd}`);
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        BABYSITTER_AUTO_PUSH: 'true',
        IS_DRAFT: 'true',
        PR_NUMBER: '101',
      },
      execFn,
    });

    expect(res.status).toBe('triage-advisory');
    expect(res.exitCode).toBe(0);
  });

  it('trips circuit breaker with exitCode 1 when git log already has >= 3 babysitter commits', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) {
        return 'sha1\nsha2\nsha3\n';
      }
      return '';
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        PR_NUMBER: '102',
        GITHUB_BASE_REF: 'master',
      },
      execFn,
    });

    expect(res).toEqual({
      status: 'locked',
      reason: 'max-attempts-exceeded',
      exitCode: 1,
    });
  });

  it('skips workspace-janitor.py cleanly when .agent/ is absent and exits 1 on fatal runner errors', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) return '';
      if (cmd === 'cargo fmt --all --check') {
        const err = new Error('cargo fmt unavailable') as Error & { stderr?: string };
        err.stderr = 'cargo: not found';
        throw err;
      }
      return '';
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: { CI: 'true', PR_NUMBER: '103' },
      execFn,
    });

    expect(res.status).toBe('error');
    expect(res.exitCode).toBe(1);
    expect(execFn).not.toHaveBeenCalledWith(expect.stringContaining('workspace-janitor.py'), expect.anything());
  });

  it('only commits without [skip ci] when BABYSITTER_AUTO_PUSH=true on non-draft CI PR', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const commands: string[] = [];
    const execFn = vi.fn((cmd: string) => {
      commands.push(cmd);
      if (cmd.startsWith('git log')) return '';
      if (cmd === 'cargo fmt --all --check') throw new Error('unformatted');
      if (cmd === 'git status -s') return ' M crates/domain/src/lib.rs\n';
      if (cmd === 'git rev-parse --short HEAD') return 'abc1234\n';
      return '';
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        BABYSITTER_AUTO_PUSH: 'true',
        IS_DRAFT: 'false',
        IS_FORK: 'false',
        PR_NUMBER: '104',
      },
      execFn,
    });

    expect(res).toEqual({ status: 'auto-fixed', commitSha: 'abc1234', exitCode: 0 });
    const commitCmd = commands.find((c) => c.startsWith('git commit'));
    expect(commitCmd).toBeDefined();
    expect(commitCmd).not.toContain('[skip ci]');
    expect(commands).toContain('git push');
  });

  it('counts PR attempt rows accurately in markdown ledgers', () => {
    const md = `
| PR # | Branch | Diagnostic | Attempt | Status | Commit |
| :---: | :--- | :--- | :---: | :---: | :--- |
| #5 | \`a\` | d1 | \`1/3\` | **TRIAGE_ADVISORY** | \`NONE\` |
| #50 | \`b\` | d2 | \`1/3\` | **VERIFIED_PRISTINE** | \`NONE\` |
| #5 | \`a\` | d3 | \`2/3\` | **TRIAGE_ADVISORY** | \`NONE\` |
`;
    expect(countPrAttemptsInMarkdown(md, '5')).toBe(2);
    expect(countPrAttemptsInMarkdown(md, '50')).toBe(1);
  });

  it('never attempts mutating GitHub API writes on external fork PRs even when attempts >= 3', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) return 'sha1\nsha2\nsha3\n';
      return '';
    });
    const fetchFn = vi.fn(async (_url: string, init?: RequestInit) => {
      expect(init?.method ?? 'GET').toBe('GET');
      return {
        ok: true,
        json: async () => [],
      } as Response;
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        IS_FORK: 'true',
        PR_NUMBER: '205',
        GITHUB_TOKEN: 'test-token',
        GITHUB_REPOSITORY: 'org/repo',
        GITHUB_BASE_REF: 'master',
      },
      execFn,
      fetchFn,
    });

    expect(res).toEqual({ status: 'read-only-triage', isFork: true, exitCode: 0 });
    expect(fetchFn).toHaveBeenCalledTimes(1);
  });

  it('paginates PR comments beyond page 1 when finding existing ledger comment', async () => {
    const repoRoot = createTempRepo();
    const stateFile = path.join(repoRoot, 'state.md');
    const execFn = vi.fn((cmd: string) => {
      if (cmd.startsWith('git log')) return '';
      if (cmd === 'cargo fmt --all --check') {
        throw new Error('unformatted');
      }
      return '';
    });
    const page1 = Array.from({ length: 100 }, (_, i) => ({ id: i + 1, body: `comment ${i}` }));
    const fetchCalls: Array<{ url: string; method: string }> = [];
    const fetchFn = vi.fn(async (url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      fetchCalls.push({ url, method });
      if (url.includes('&page=1')) {
        return { ok: true, json: async () => page1 } as Response;
      }
      if (url.includes('&page=2')) {
        return {
          ok: true,
          json: async () => [
            {
              id: 999,
              body: '<!-- pr-babysitter-ledger -->\n| #206 | `b` | old | `1/3` | **TRIAGE_ADVISORY** | `NONE` |\n',
            },
          ],
        } as Response;
      }
      return { ok: true, json: async () => ({ id: 999 }) } as Response;
    });

    const res = await runPrBabysitter({
      repoRoot,
      stateFile,
      env: {
        CI: 'true',
        PR_NUMBER: '206',
        GITHUB_TOKEN: 'test-token',
        GITHUB_REPOSITORY: 'org/repo',
      },
      execFn,
      fetchFn,
    });

    expect(res).toEqual({ status: 'triage-advisory', needsFormat: true, exitCode: 0 });
    expect(fetchCalls.map((c) => `${c.method} ${c.url}`)).toEqual([
      'GET https://api.github.com/repos/org/repo/issues/206/comments?per_page=100&page=1',
      'GET https://api.github.com/repos/org/repo/issues/206/comments?per_page=100&page=2',
      'GET https://api.github.com/repos/org/repo/issues/206/comments?per_page=100&page=1',
      'GET https://api.github.com/repos/org/repo/issues/206/comments?per_page=100&page=2',
      'PATCH https://api.github.com/repos/org/repo/issues/comments/999',
    ]);
  });
});
