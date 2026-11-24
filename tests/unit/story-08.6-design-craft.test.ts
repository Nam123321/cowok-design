import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { assembleDesignCraftRepo, type AssembleReport } from "../../scripts/assemble-design-craft-repo";

const REPO_ROOT = path.resolve(__dirname, "../..");
let cleanRoomDir = "";
let workspaceJailDir = "";
let outsideSecretFile = "";
let report: AssembleReport;

async function sendMcpRequests(serverPath: string, rootDir: string, requests: unknown[]): Promise<unknown[]> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [serverPath], {
      cwd: cleanRoomDir,
      env: { ...process.env, DESIGN_CRAFT_ROOT: rootDir },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const responses: unknown[] = [];
    let buffer = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      buffer += chunk.toString("utf8");
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        if (line.trim()) responses.push(JSON.parse(line));
      }
      if (responses.length >= requests.length) {
        child.stdin.end();
      }
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0 && responses.length < requests.length) {
        reject(new Error(`MCP server exited with code ${code}: ${stderr}`));
      } else {
        resolve(responses);
      }
    });
    for (const req of requests) {
      child.stdin.write(`${JSON.stringify(req)}\n`);
    }
  });
}

function runCliSync(cliPath: string, args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cliPath, ...args], { cwd: cleanRoomDir, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (c) => (stdout += c.toString("utf8")));
    child.stderr.on("data", (c) => (stderr += c.toString("utf8")));
    child.on("error", reject);
    child.on("close", (code) => resolve({ code: code ?? 1, stdout, stderr }));
  });
}

beforeAll(() => {
  const baseTmp = fs.mkdtempSync(path.join(os.tmpdir(), "design-craft-cleanroom-"));
  cleanRoomDir = path.join(baseTmp, "design-craft-repo");
  workspaceJailDir = path.join(baseTmp, "jail-project");
  outsideSecretFile = path.join(baseTmp, "outside-secret.txt");
  fs.mkdirSync(workspaceJailDir, { recursive: true });
  fs.writeFileSync(outsideSecretFile, "TOP_SECRET_OUTSIDE_JAIL", "utf8");
  fs.writeFileSync(
    path.join(workspaceJailDir, "screen.html"),
    `<main data-ff-id="root"><h1 data-ff-id="hero-title">Invoice Overview</h1><div data-state="loading"></div><div data-state="empty"></div><div data-state="error"></div><button type="button">Create Invoice</button></main>`,
    "utf8"
  );
  fs.symlinkSync(outsideSecretFile, path.join(workspaceJailDir, "escape-link.txt"));
  report = assembleDesignCraftRepo({ repoRoot: REPO_ROOT, outputDir: cleanRoomDir });
});

afterAll(() => {
  if (cleanRoomDir) {
    fs.rmSync(path.dirname(cleanRoomDir), { recursive: true, force: true });
  }
});

describe("Story 08.6: Standalone design-craft 4-Pillar Assembly & Runtime", () => {
  it("AC-01: assembles standalone clean-room repo with 0 cowok-design back-references and <=100-tok SKILL.md headers", () => {
    expect(report.phaseSkills).toHaveLength(5);
    expect(report.domainSkills).toHaveLength(5);
    expect(report.craftSkills.length).toBeGreaterThanOrEqual(15);
    expect(report.agents).toHaveLength(5);
    expect(report.builderTools).toHaveLength(7);
    expect(report.qcAuditTools).toHaveLength(6);
    expect(report.backReferenceViolations).toEqual([]);
    for (const skill of report.validatedSkills) {
      expect(skill.tokens).toBeLessThanOrEqual(100);
    }
    const pkg = JSON.parse(fs.readFileSync(path.join(cleanRoomDir, "package.json"), "utf8"));
    expect(pkg.name).toBe("design-craft");
    expect(pkg.bin["design-craft"]).toBe("./bin/design-craft.mjs");
    expect(pkg.bin["design-craft-mcp"]).toBe("./mcp/server.mjs");
    expect(pkg.dependencies).toEqual({});
  });

  it("AC-02 & AC-06: runs all 7 Builder tools with custom themeModes, MICRO_POLISH fast-path, and native upgrade detection", async () => {
    const builderMod = await import(pathToFileURL(path.join(cleanRoomDir, "engine/builder-tools.mjs")).href);
    const micro = builderMod.dePhaseRouter({ prompt: "Nudge button padding by 4px" });
    expect(micro.entryMode).toBe("MICRO_POLISH");
    expect(micro.spawnSubagents).toBe(false);
    expect(micro.maxAgents).toBe(1);
    expect(micro.directTool).toBe("de-patch-dom");

    expect(() => builderMod.dePhaseRouter({ entryMode: "GREENFIELD", autoPilot: true })).toThrow(/HeritagePack/i);
    const autoOk = builderMod.dePhaseRouter({ entryMode: "GREENFIELD", autoPilot: true, inheritedHeritagePackId: "hp-01" });
    expect(autoOk.pendingHitlGate).toBe("NONE");

    const intake = builderMod.deIntake(
      { projectId: "acme-billing", themeModes: ["warm", "dim"], files: ["screen.html"] },
      { rootDir: workspaceJailDir }
    );
    expect(intake.cognitiveState.themeModes).toEqual(["warm", "dim"]);
    expect(intake.ingestedFiles).toHaveLength(1);

    const scan = builderMod.deScanSketch({ filePath: "screen.html" }, { rootDir: workspaceJailDir });
    expect(scan.passed).toBe(true);
    expect(scan.missingStates).toEqual([]);

    const tokens = builderMod.deTokensOklch({ themeModes: ["warm", "dim"], brandHue: 245 });
    expect(tokens.themeModes).toEqual(["warm", "dim"]);
    expect(Object.keys(tokens.themes)).toEqual(["warm", "dim"]);
    expect(tokens.css).toContain('[data-theme="warm"]');
    expect(tokens.css).toContain('[data-theme="dim"]');
    expect(tokens.css).not.toContain('[data-theme="light"]');
    expect(tokens.css).not.toContain('[data-theme="dark"]');

    const preview = builderMod.deBrandPreview({ brandName: "Acme Ledger", themeModes: ["warm", "dim"] });
    expect(preview.html).toContain('data-theme="warm"');
    expect(preview.html).toContain('data-theme="dim"');
    expect(preview.html).not.toContain('data-theme="light"');

    const contract = builderMod.deExtractContract({ archetypeId: "editorial-warm-studio" });
    expect(contract.sourceMode).toBe("offline-archetype-fallback");
    expect(contract.matrix.doNotCopy.length).toBeGreaterThan(0);

    const patched = builderMod.dePatchDom(
      { filePath: "screen.html", patches: [{ targetId: "hero-title", op: "setText", value: "Updated Billing" }] },
      { rootDir: workspaceJailDir }
    );
    expect(patched.appliedCount).toBe(1);
    expect(patched.patchedHtml).toContain("Updated Billing");

    const nativeUpgrade = builderMod.resolveHostUpgrade("de-tokens-oklch", { COWOK_DESIGN_NATIVE: "1" });
    expect(nativeUpgrade.mode).toBe("native");
    expect(nativeUpgrade.target).toBe("cowok-design::html-forge::oklch");
  });

  it("AC-02: runs all 6 QC/Audit tools and catches jargon, dark: classes, contrast failures, and overflow", async () => {
    const qcMod = await import(pathToFileURL(path.join(cleanRoomDir, "engine/qc-tools.mjs")).href);
    const contrastPass = qcMod.qcA11yContrast({
      pairs: [{ name: "warm-body", fg: "oklch(0.200 0.014 72)", bg: "oklch(0.960 0.022 72)" }],
    });
    expect(contrastPass.passed).toBe(true);
    const contrastFail = qcMod.qcA11yContrast({ pairs: [{ name: "low", fg: "#999999", bg: "#aaaaaa" }] });
    expect(contrastFail.passed).toBe(false);

    const slop = qcMod.qcAntiSlopLinter({
      content: `<div class="dark:bg-slate-900">Configure BYOS and SQLite WAL — Lorem ipsum #ff00aa</div>`,
    });
    expect(slop.passed).toBe(false);
    expect(slop.violations.some((v: { ruleId: string }) => v.ruleId === "ANTI_SLOP_JARGON")).toBe(true);
    expect(slop.violations.some((v: { ruleId: string }) => v.ruleId === "FORBIDDEN_TAILWIND_DARK")).toBe(true);
    expect(slop.violations.some((v: { ruleId: string }) => v.ruleId === "ANTI_SLOP_PLACEHOLDER")).toBe(true);

    const overflow = qcMod.qcLayoutOverflow({
      preset: "slide-16-9",
      boxes: [{ id: "hero", x: 0, y: 0, width: 1400, height: 800, layer: 0 }],
    });
    expect(overflow.passed).toBe(false);
    expect(overflow.overflows.length).toBe(2);

    const chart = qcMod.qcChartAndData({
      charts: [{ id: "rev", type: "bar", title: "Revenue grew 18%", unit: "USD", series: [10, 12, 14] }],
    });
    expect(chart.passed).toBe(true);

    const parity = qcMod.qcSpecParity({
      requiredScreens: ["overview", "detail"],
      requiredThemeModes: ["warm", "dim"],
      actualScreens: ["overview", "detail"],
      actualThemeModes: ["warm", "dim"],
    });
    expect(parity.passed).toBe(true);

    const popular = qcMod.qcPopularCoverage({
      outputTrack: "app-fe-be",
      usedComponents: ["kpi-metric-line", "advanced-filter-bar", "data-table-view", "right-drawer", "toast-feedback"],
    });
    expect(popular.passed).toBe(true);
  });

  it("AC-02 & AC-06: spawns Zero-Shell stdio MCP server and enforces Zero-Trust Path Jail", async () => {
    const serverPath = path.join(cleanRoomDir, "mcp/server.mjs");
    const responses = (await sendMcpRequests(serverPath, workspaceJailDir, [
      { jsonrpc: "2.0", id: 1, method: "initialize", params: {} },
      { jsonrpc: "2.0", id: 2, method: "tools/list", params: {} },
      { jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "de-tokens-oklch", arguments: { themeModes: ["warm", "dim"] } } },
      { jsonrpc: "2.0", id: 4, method: "tools/call", params: { name: "de-scan-sketch", arguments: { filePath: "../outside-secret.txt" } } },
      { jsonrpc: "2.0", id: 5, method: "tools/call", params: { name: "de-scan-sketch", arguments: { filePath: "escape-link.txt" } } },
      { jsonrpc: "2.0", id: 6, method: "tools/call", params: { name: "de-scan-sketch", arguments: { filePath: "screen.html; cat /etc/passwd" } } },
    ])) as Array<{ id: number; result: { tools?: unknown[]; isError?: boolean; content?: Array<{ text: string }> } }>;

    expect(responses[0].result).toBeDefined();
    expect(responses[1].result.tools).toHaveLength(14);

    const okPayload = JSON.parse(responses[2].result.content?.[0].text ?? "{}");
    expect(responses[2].result.isError).toBe(false);
    expect(okPayload.themeModes).toEqual(["warm", "dim"]);

    for (const blockedResp of [responses[3], responses[4], responses[5]]) {
      expect(blockedResp.result.isError).toBe(true);
      const errBody = JSON.parse(blockedResp.result.content?.[0].text ?? "{}");
      expect(errBody.error).toMatch(/TRAVERSAL_BLOCKED|SYMLINK_ESCAPE|SHELL_METACHAR/);
    }
  });

  it("AC-03 & AC-04: exports multi-harness agent packs for all 6 targets and recommends upstream REA with --skip-rea fallback", async () => {
    const cliPath = path.join(cleanRoomDir, "bin/design-craft.mjs");
    const targets = [
      { target: "claude", expectedFile: ".claude/agents/design-auditor.md" },
      { target: "codex", expectedFile: ".codex/agents/design-auditor.toml" },
      { target: "cursor", expectedFile: ".cursor/agents/design-auditor.md" },
      { target: "antigravity", expectedFile: ".agents/agents/design-auditor.md" },
      { target: "openclaw", expectedFile: "agent-pack.json" },
      { target: "cowok", expectedFile: ".cowok/plugins/design-craft/plugin.json" },
    ];
    for (const item of targets) {
      const targetOut = fs.mkdtempSync(path.join(os.tmpdir(), `dc-target-${item.target}-`));
      try {
        const res = await runCliSync(cliPath, ["install", "--target", item.target, "--dir", targetOut, "--json"]);
        expect(res.code).toBe(0);
        const receipt = JSON.parse(res.stdout);
        expect(receipt.reaRecommendation.recommended).toBe(true);
        expect(receipt.reaRecommendation.upstreamRepoUrl).toContain("https://github.com/");
        expect(fs.existsSync(path.join(targetOut, item.expectedFile))).toBe(true);
        if (item.target === "codex") {
          const toml = fs.readFileSync(path.join(targetOut, item.expectedFile), "utf8");
          expect(toml).toContain("product_write_allowed = false");
        }
      } finally {
        fs.rmSync(targetOut, { recursive: true, force: true });
      }
    }

    const skipOut = fs.mkdtempSync(path.join(os.tmpdir(), "dc-skip-rea-"));
    try {
      const skipRes = await runCliSync(cliPath, ["install", "--target", "openclaw", "--dir", skipOut, "--skip-rea", "--json"]);
      expect(skipRes.code).toBe(0);
      const skipReceipt = JSON.parse(skipRes.stdout);
      expect(skipReceipt.reaRecommendation.skipped).toBe(true);
      expect(skipReceipt.reaRecommendation.fallback).toBe("catalogs/reference-archetypes.json");
    } finally {
      fs.rmSync(skipOut, { recursive: true, force: true });
    }
  });

  it("F-01: blocks dangling-symlink arbitrary writes, backslash, drive-letter, and UNC paths in path-jail", async () => {
    const jailMod = await import(pathToFileURL(path.join(cleanRoomDir, "engine/path-jail.mjs")).href);
    const outsideNewTarget = path.join(path.dirname(workspaceJailDir), "pwned-via-dangling.txt");
    const danglingLink = path.join(workspaceJailDir, "dangling-link.txt");
    fs.symlinkSync(outsideNewTarget, danglingLink);

    const expectCode = (fn: () => unknown, expectedCode: string) => {
      try {
        fn();
        expect.unreachable(`Expected error code ${expectedCode}`);
      } catch (err) {
        expect((err as { code?: string }).code).toBe(expectedCode);
      }
    };

    expectCode(() => jailMod.writeJailedFile(workspaceJailDir, "dangling-link.txt", "OWNED"), "SYMLINK_BLOCKED");
    expect(fs.existsSync(outsideNewTarget)).toBe(false);

    expectCode(() => jailMod.assertSafePath(workspaceJailDir, "foo\\..\\bar.txt"), "BACKSLASH_BLOCKED");
    expectCode(() => jailMod.assertSafePath(workspaceJailDir, "C:secret.txt"), "DRIVE_PATH_BLOCKED");
    expectCode(() => jailMod.assertSafePath(workspaceJailDir, "//evil-host/share/file.txt"), "UNC_PATH_BLOCKED");

    const okFile = jailMod.writeJailedFile(workspaceJailDir, "nested/safe.txt", "SAFE_CONTENT");
    expect(fs.readFileSync(okFile, "utf8")).toBe("SAFE_CONTENT");
  });

  it("F-02: bounds MCP stdio line size (1 MiB -> -32600), rejects prototype tools & invalid params (-32602)", async () => {
    const serverPath = path.join(cleanRoomDir, "mcp/server.mjs");
    const serverMod = await import(pathToFileURL(serverPath).href);
    expect(serverMod.MAX_LINE_BYTES).toBe(1_048_576);

    const nullParamsResp = serverMod.handleJsonRpcRequest({ jsonrpc: "2.0", id: 10, method: "tools/call", params: null });
    expect(nullParamsResp.error?.code).toBe(-32602);

    const arrayParamsResp = serverMod.handleJsonRpcRequest({ jsonrpc: "2.0", id: 11, method: "tools/call", params: [] });
    expect(arrayParamsResp.error?.code).toBe(-32602);

    const nonStringNameResp = serverMod.handleJsonRpcRequest({ jsonrpc: "2.0", id: 12, method: "tools/call", params: { name: 42 } });
    expect(nonStringNameResp.error?.code).toBe(-32602);

    const protoResp = serverMod.handleJsonRpcRequest({ jsonrpc: "2.0", id: 13, method: "tools/call", params: { name: "toString", arguments: {} } });
    expect(protoResp.result?.isError).toBe(true);
    expect(JSON.parse(protoResp.result.content[0].text).error).toBe("UNKNOWN_TOOL");

    const cliPath = path.join(cleanRoomDir, "bin/design-craft.mjs");
    const cliProto = await runCliSync(cliPath, ["run", "constructor"]);
    expect(cliProto.code).not.toBe(0);
    expect(cliProto.stderr).toContain("Unknown tool");

    // Live stdio overflow test (> 1 MiB line returns -32600 and recovers on next valid line)
    const oversizedResponses = await new Promise<Array<{ id: number | null; error?: { code: number }; result?: unknown }>>((resolve, reject) => {
      const child = spawn(process.execPath, [serverPath], {
        cwd: cleanRoomDir,
        env: { ...process.env, DESIGN_CRAFT_ROOT: workspaceJailDir },
        stdio: ["pipe", "pipe", "pipe"],
      });
      const out: Array<{ id: number | null; error?: { code: number }; result?: unknown }> = [];
      let buf = "";
      child.stdout.on("data", (c) => {
        buf += c.toString("utf8");
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const l of lines) {
          if (l.trim()) out.push(JSON.parse(l));
        }
        if (out.length >= 2) child.stdin.end();
      });
      child.on("error", reject);
      child.on("close", () => resolve(out));
      child.stdin.write(`${"x".repeat(1_048_576 + 128)}\n`);
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: 99, method: "ping" })}\n`);
    });
    expect(oversizedResponses[0].error?.code).toBe(-32600);
    expect(oversizedResponses[1].id).toBe(99);
    expect(oversizedResponses[1].result).toEqual({});
  });

  it("F-03, F-05 & F-10: enforces frontmatter-only auditor contract, strict GitHub URL, safe TOML escaping, agent names, and O_NOFOLLOW install", async () => {
    const cliMod = await import(pathToFileURL(path.join(cleanRoomDir, "bin/design-craft.mjs")).href);

    // F-05: Strict GitHub URL + installHint validation
    expect(cliMod.isValidGitHubRepoUrl("https://github.com/nicobailey/reverse-engineer-anything")).toBe(true);
    expect(cliMod.isValidGitHubRepoUrl("https://github.com/nicobailey/reverse-engineer-anything?x=1")).toBe(false);
    expect(cliMod.isValidGitHubRepoUrl("https://github.com/nicobailey/reverse-engineer-anything#frag")).toBe(false);
    expect(cliMod.isValidGitHubRepoUrl("https://github.com/../evil")).toBe(false);
    expect(cliMod.isValidGitHubRepoUrl("https://github.com/owner/repo;rm -rf /")).toBe(false);
    expect(() =>
      cliMod.validateOptionalPeer({
        id: "rea-upstream",
        repoUrl: "https://github.com/nicobailey/reverse-engineer-anything",
        installHint: "git clone https://github.com/evil/other && id",
      })
    ).toThrow(/Invalid installHint/);

    // F-10: TOML backslash-before-quote escaping and trailing quote safety
    const toml = cliMod.renderCodexAgentToml({
      name: "design-auditor",
      role: 'Auditor "C:\\path\\"',
      isReadOnly: true,
      content: 'Use C:\\temp\\"""code""" and end with "',
    });
    expect(toml).toContain('role = "Auditor \\"C:\\\\path\\\\\\""');
    expect(toml).toContain('Use C:\\\\temp\\\\\\"\\"\\"code\\"\\"\\" and end with \\"');

    // F-10: Agent name validation
    expect(() => cliMod.loadAgentFiles(["../escape"])).toThrow(/Invalid agent name/);

    // F-03: Frontmatter-only read-only enforcement rejects body-decoy bypass
    const fakeBundle = fs.mkdtempSync(path.join(os.tmpdir(), "dc-auditor-bypass-"));
    try {
      fs.mkdirSync(path.join(fakeBundle, "agents"), { recursive: true });
      fs.writeFileSync(
        path.join(fakeBundle, "agents", "design-auditor.md"),
        "---\nname: design-auditor\nrole: Auditor\nproduct_write_allowed: true\nread_only: false\nwrite_scope: [screens/**]\n---\nBody mentions product_write_allowed: false and read_only: true\n",
        "utf8"
      );
      expect(() => cliMod.loadAgentFiles(["design-auditor"], fakeBundle)).toThrow(/design-auditor frontmatter/);
    } finally {
      fs.rmSync(fakeBundle, { recursive: true, force: true });
    }

    // F-10: CLI install refuses dangling symlink target via O_NOFOLLOW
    const installTmp = fs.mkdtempSync(path.join(os.tmpdir(), "dc-install-nofollow-"));
    try {
      const outsideTrap = path.join(installTmp, "outside-trap.json");
      const targetOut = path.join(installTmp, "out");
      fs.mkdirSync(targetOut, { recursive: true });
      fs.symlinkSync(outsideTrap, path.join(targetOut, "agent-pack.json"));
      expect(() => cliMod.runInstallCommand({ target: "openclaw", dir: targetOut })).toThrow(/symlink/i);
      expect(fs.existsSync(outsideTrap)).toBe(false);
    } finally {
      fs.rmSync(installTmp, { recursive: true, force: true });
    }
  });

  it("F-06: refuses destructive rmSync on repoRoot/parent/root and catches crates/ backrefs + external .mjs imports", () => {
    expect(() => assembleDesignCraftRepo({ repoRoot: REPO_ROOT, outputDir: REPO_ROOT })).toThrow(/Refusing destructive/);
    expect(() => assembleDesignCraftRepo({ repoRoot: REPO_ROOT, outputDir: path.dirname(REPO_ROOT) })).toThrow(/Refusing destructive/);
    expect(() => assembleDesignCraftRepo({ repoRoot: REPO_ROOT, outputDir: path.parse(REPO_ROOT).root })).toThrow(/Refusing destructive/);

    // Synthetic repo with crates/ backref in templates/ and external npm import in .mjs
    const fakeRepo = fs.mkdtempSync(path.join(os.tmpdir(), "dc-fake-repo-"));
    const fakeOut = fs.mkdtempSync(path.join(os.tmpdir(), "dc-fake-out-"));
    try {
      const dstBundle = path.join(fakeRepo, "packages", "builtin-skills", "design-craft");
      fs.cpSync(path.join(REPO_ROOT, "packages", "builtin-skills", "design-craft"), dstBundle, { recursive: true });
      fs.writeFileSync(path.join(dstBundle, "templates", "specs", "01-intent-brief.md"), "Leaks crates/spec-registry/src/lib.rs\n", "utf8");
      expect(() => assembleDesignCraftRepo({ repoRoot: fakeRepo, outputDir: fakeOut })).toThrow(/crates\\\//);

      fs.writeFileSync(path.join(dstBundle, "templates", "specs", "01-intent-brief.md"), "# Clean template\n", "utf8");
      const origEngine = fs.readFileSync(path.join(dstBundle, "engine", "path-jail.mjs"), "utf8");
      fs.writeFileSync(path.join(dstBundle, "engine", "path-jail.mjs"), `import lodash from "lodash";\n${origEngine}`, "utf8");
      expect(() => assembleDesignCraftRepo({ repoRoot: fakeRepo, outputDir: fakeOut })).toThrow(/disallowed external specifier 'lodash'/);
    } finally {
      fs.rmSync(fakeRepo, { recursive: true, force: true });
      fs.rmSync(fakeOut, { recursive: true, force: true });
    }
  });

  it("F-07, F-08, F-09 & F-11: neutralizes XSS/$-corruption, rejects NaN/prototype viewports, includes data-viz-storyteller, and fixes MICRO_POLISH + omitted themeModes", async () => {
    const builderMod = await import(pathToFileURL(path.join(cleanRoomDir, "engine/builder-tools.mjs")).href);
    const qcMod = await import(pathToFileURL(path.join(cleanRoomDir, "engine/qc-tools.mjs")).href);

    // F-07: deBrandPreview ignores raw input.css and validates themeModes
    const preview = builderMod.deBrandPreview({
      brandName: "Acme Studio",
      themeModes: ["warm"],
      themes: { warm: {} },
      css: "</style><script>alert(1)</script>",
    });
    expect(preview.html).not.toContain("<script>alert(1)</script>");
    expect(preview.html).toContain('[data-theme="warm"]');
    expect(() => builderMod.deBrandPreview({ themeModes: ['warm"><script>alert(1)</script>'], themes: {} })).toThrow(/Invalid themeMode/i);

    // F-07: dePatchDom HTML-escapes setText/setAttr/addClass and preserves $100 / $&
    const patched = builderMod.dePatchDom({
      html: `<main><h1 data-ff-id="price">Old</h1><div data-ff-id="card" class="base">Body</div></main>`,
      patches: [
        { targetId: "price", op: "setText", value: "$100 & $& <img src=x onerror=alert(1)>" },
        { targetId: "card", op: "setAttr", attr: "data-label", value: 'a" onclick="evil()' },
        { targetId: "card", op: "addClass", value: 'hero"><svg>' },
      ],
    });
    expect(patched.patchedHtml).toContain("$100 &amp; $&amp; &lt;img src=x onerror=alert(1)&gt;");
    expect(patched.patchedHtml).toContain('data-label="a&quot; onclick=&quot;evil()"');
    expect(patched.patchedHtml).toContain('class="base hero&quot;&gt;&lt;svg&gt;"');

    // F-08: qcLayoutOverflow guards prototype preset and rejects NaN / non-positive viewports
    const protoPreset = qcMod.qcLayoutOverflow({ preset: "toString", boxes: [{ id: "a", x: 0, y: 0, width: 2000, height: 100, layer: 0 }] });
    expect(protoPreset.passed).toBe(false);
    expect(protoPreset.viewport.width).toBe(1440);
    expect(() => qcMod.qcLayoutOverflow({ viewport: { width: Number.NaN, height: 900 } })).toThrow(/finite positive/i);
    expect(() => qcMod.qcLayoutOverflow({ viewport: { width: 0, height: 900 } })).toThrow(/finite positive/i);
    expect(() => qcMod.qcLayoutOverflow({ viewport: { width: 1440, height: -10 } })).toThrow(/finite positive/i);

    // F-09: plugin.json phase-3 includes data-viz-storyteller
    const manifest = JSON.parse(fs.readFileSync(path.join(cleanRoomDir, "plugin.json"), "utf8"));
    expect(manifest.phaseSkillMap["phase-3"]).toContain("data-viz-storyteller");

    // F-11: MICRO_POLISH precedes DECK_FROM_APP, unknown entryMode throws, omitted themeModes returns [] + requiresUserThemeChoice
    const microSlide = builderMod.dePhaseRouter({ outputTrack: "presentation-16-9", prompt: "Fix typo on slide 2" });
    expect(microSlide.entryMode).toBe("MICRO_POLISH");
    expect(microSlide.spawnSubagents).toBe(false);
    expect(() => builderMod.dePhaseRouter({ entryMode: "NON_EXISTENT_MODE" })).toThrow(/Unknown entryMode/i);

    const omittedThemes = builderMod.deIntake({ projectId: "user-choice-app" });
    expect(omittedThemes.cognitiveState.themeModes).toEqual([]);
    expect(omittedThemes.requiresUserThemeChoice).toBe(true);
    expect(omittedThemes.cognitiveState.requiresUserThemeChoice).toBe(true);
  });
});
