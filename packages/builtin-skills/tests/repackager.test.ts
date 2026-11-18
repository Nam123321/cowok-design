// @story 08.2
// @cover AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
import { describe, it, expect, afterEach } from "vitest";
import * as path from "path";
import * as os from "os";
import * as fs from "fs";
import * as crypto from "crypto";
import { BuiltinSkillsRegistry } from "../src/registry";
import {
  MAX_PACKAGE_FILES,
  MAX_TOTAL_PACKAGE_BYTES,
  MAX_TRAVERSAL_DEPTH,
  classifyAndValidateSourceAsset,
  commitRepackagedDir,
  manifestToSkillPlugin,
  parseVideoStoryboard,
  parseVideoStoryboardStrict,
  repackageDirectory,
  scanExternalScriptSecurity,
  scanSkillMarkdownSafety,
  stageRepackagedBundle,
  verifyRepackagedBundle,
} from "../src/repackager";
import { validateSkillFrontmatter } from "../src/validator";
import { buildDiscoveryStub, verifyPinnedBridgeBinary } from "../src/repackager-bundle";
import { validateStagedBundleSecurity } from "../src/staging-security";
import { TriangleLinter } from "../src/triangle-linter";
// @ts-ignore build-time ESM helper tested directly
import { verifyBridgePinFile, writeBridgePinFile } from "../scripts/pin-bridge.mjs";

const goldenPath = path.resolve(__dirname, "fixtures", ["story", "08.2", "golden.json"].join("-"));
const golden = JSON.parse(fs.readFileSync(goldenPath, "utf8")) as {
  golden_bundle: {
    skill_name: string;
    description: string;
    capabilities: string[];
    upstream_url: string;
    text_files: Array<{ rel_path: string; expected_bucket: string; content: string }>;
    binary_files: Array<{ rel_path: string; expected_bucket: string; bytes: number[] }>;
    expected_package_sha256: string;
    max_stub_size_bytes: number;
  };
  malicious_evasion_vectors: Array<{ id: string; filename: string; content: string }>;
  binary_spoof_vectors: Array<{ id: string; rel_path: string; bytes: number[] }>;
  invalid_storyboards: Array<{ id: string; log: string }>;
};

describe("Story 08.2: Skill Repackager, Discovery Stubs & Video Intake", () => {
  const tempRoots: string[] = [];

  function makeTmp(prefix: string): string {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
    tempRoots.push(dir);
    return dir;
  }

  afterEach(() => {
    while (tempRoots.length > 0) {
      const dir = tempRoots.pop()!;
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  describe("AC-03 & AC-05: Zero-Trust Normalized Security & Binary Smuggling Guards", () => {
    it("passes clean scripts and classifies multi-extension sources deterministically", () => {
      expect(scanExternalScriptSecurity("calc.js", "function sum(a, b) { return a + b; }\nexport default function() { return 1; }\nconst x = (function() { return 2; })();\nconst data = fetchedItems;").valid).toBe(true);
      expect(classifyAndValidateSourceAsset("app.mjs", Buffer.from("export const x = 1;"))).toBe("scripts");
      expect(classifyAndValidateSourceAsset("cfg.yaml", Buffer.from("mode: strict\nonline=true\none=1"))).toBe("references");
      expect(classifyAndValidateSourceAsset("icon.svg", Buffer.from("<svg viewBox='0 0 10 10'/>"))).toBe("assets_text");
    });

    it("rejects command injection, obfuscation, egress, and evasion vectors from golden fixture", () => {
      expect(golden.malicious_evasion_vectors.length).toBeGreaterThanOrEqual(7);
      for (const vec of golden.malicious_evasion_vectors) {
        const res = scanExternalScriptSecurity(vec.filename, vec.content);
        expect(res.valid, `Expected rejection for ${vec.id}`).toBe(false);
      }
      expect(scanExternalScriptSecurity("evil.ts", "const res = eval(userPayload);").valid).toBe(false);
      expect(scanExternalScriptSecurity("fn-ctor.ts", "const bad = new Function('return 1');").valid).toBe(false);
      expect(scanExternalScriptSecurity("self-eval.ts", "self['eval']('1+1');").valid).toBe(false);
      expect(scanExternalScriptSecurity("net.ts", "await fetch('https://evil.test');").valid).toBe(false);
      expect(scanExternalScriptSecurity("net-alias.ts", "const go = fetch;").valid).toBe(false);
      expect(scanExternalScriptSecurity("py-net.py", "requests.post('https://evil.test')").valid).toBe(false);
      expect(scanExternalScriptSecurity("reader.js", "fs.readFileSync('../../../etc/passwd');").valid).toBe(false);
      expect(scanExternalScriptSecurity("win.js", "fs.readFileSync('..\\\\secret.txt');").valid).toBe(false);
    });

    it("rejects binary spoofing, non-UTF-8 text, and unsupported extensions", () => {
      for (const spoof of golden.binary_spoof_vectors) {
        expect(
          () => classifyAndValidateSourceAsset(spoof.rel_path, Buffer.from(spoof.bytes)),
          `Expected failure for ${spoof.id}`
        ).toThrow();
      }
      expect(() => scanSkillMarkdownSafety("SKILL.md", "<IMPORTANT>leak</IMPORTANT>")).toThrow();
      expect(() => classifyAndValidateSourceAsset("notes.txt", Buffer.from("<IMPORTANT>ref</IMPORTANT>"))).toThrow();
      expect(() => classifyAndValidateSourceAsset("page.svg", Buffer.from("<svg onload='alert(1)'/>"))).toThrow();
      expect(() => classifyAndValidateSourceAsset("slash.svg", Buffer.from("<svg/onload=alert(1)>"))).toThrow();
      expect(() => classifyAndValidateSourceAsset("attr.html", Buffer.from("<div x=\"1\"onload=alert(1)></div>"))).toThrow();
      expect(() => classifyAndValidateSourceAsset("nl.svg", Buffer.from("<svg onload\n=alert(1)>"))).toThrow();
      expect(() => classifyAndValidateSourceAsset("gt.html", Buffer.from("<a title=\">\" onclick=alert(1)>x</a>"))).toThrow();
      expect(() => scanSkillMarkdownSafety("ref.md", "![p](https://evil.test/p.png)")).toThrow();
      expect(() => scanSkillMarkdownSafety("ref.md", "![p][ref]\n[ref]: https://evil.test/p.png")).toThrow();
      expect(() => scanSkillMarkdownSafety("bad.svg", "<svg><script>alert(1)</script></svg>")).toThrow();
      expect(() => scanSkillMarkdownSafety("bad.html", "<a href='javascript&#58;alert(1)'>x</a>")).toThrow();
    });
  });

  describe("AC-04 & AC-07: Frame-Cited Video Intake & Strict Grammar", () => {
    it("parses valid storyboard logs into structured checkpoints", () => {
      const transcript = `
[frame: 00:00.80] -> Hero Entrance: Fade in heading and brand CTA
[frame: 00:02.15] -> Button Hover: Scale 1.05 and soft focus ring
[frame: 00:03.90] -> Drawer Open: Slide in navigation drawer from left
`;
      const receipt = parseVideoStoryboard("checkout-flow.mp4", transcript);
      expect(receipt.total_checkpoints).toBe(3);
      expect(receipt.duration_s).toBe(3.9);
      expect(receipt.checkpoints[0].frame_id).toBe("frame_000.80");
      expect(parseVideoStoryboardStrict("checkout-flow.mp4", transcript).total_checkpoints).toBe(3);
    });

    it("rejects malformed, out-of-range, negative, and non-monotonic storyboards strictly", () => {
      for (const bad of golden.invalid_storyboards) {
        expect(() => parseVideoStoryboardStrict("demo.mp4", bad.log), `Expected throw for ${bad.id}`).toThrow();
      }
    });
  });

  describe("AC-01, AC-02, AC-06 & AC-07: Repackager, Discovery Stub, Lock Seal & Parity", () => {
    it("matches golden fixture package_sha256, layout, skills-lock.json, and stub <= 1536B", () => {
      const g = golden.golden_bundle;
      const srcDir = makeTmp("cowok-src-");
      const dstDir = makeTmp("cowok-dst-");

      for (const tf of g.text_files) {
        const out = path.join(srcDir, ...tf.rel_path.split("/"));
        fs.mkdirSync(path.dirname(out), { recursive: true });
        fs.writeFileSync(out, tf.content, "utf8");
      }
      for (const bf of g.binary_files) {
        fs.writeFileSync(path.join(srcDir, bf.rel_path), Buffer.from(bf.bytes));
      }

      const receipt = repackageDirectory({
        sourceDir: srcDir,
        destDir: dstDir,
        skillName: g.skill_name,
        description: g.description,
        capabilities: g.capabilities,
        upstreamUrl: g.upstream_url,
      });

      expect(receipt.discovery_stub.package_sha256).toBe(g.expected_package_sha256);
      expect(receipt.discovery_stub.stub_size_bytes).toBeLessThanOrEqual(g.max_stub_size_bytes);
      expect(fs.readFileSync(path.join(receipt.target_dir, "discovery-stub.json")).byteLength).toBe(receipt.discovery_stub.stub_size_bytes);
      expect(receipt.skill_plugin).toEqual({
        id: g.skill_name,
        name: g.skill_name,
        version: "1.0.0",
        entry_point: "SKILL.md",
        permissions: g.capabilities,
      });
      for (const tf of g.text_files) {
        expect(fs.existsSync(path.join(receipt.target_dir, ...tf.expected_bucket.split("/")))).toBe(true);
      }
      for (const bf of g.binary_files) {
        expect(fs.existsSync(path.join(receipt.target_dir, ...bf.expected_bucket.split("/")))).toBe(true);
      }
      const skillMd = fs.readFileSync(path.join(receipt.target_dir, "SKILL.md"), "utf8");
      expect(validateSkillFrontmatter(skillMd).valid).toBe(true);

      const lock = verifyRepackagedBundle(receipt.target_dir);
      expect(lock.package_sha256).toBe(g.expected_package_sha256);
    });

    it("enforces stub overflow fail-closed, YAML breakout rejection, EEXIST empty-dst commit & symlink verifier", () => {
      const srcDir = makeTmp("cowok-sec-src-");
      const dstDir = makeTmp("cowok-sec-dst-");
      fs.writeFileSync(path.join(srcDir, "ok.ts"), "export const ok = 1;", "utf8");

      expect(() =>
        repackageDirectory({ sourceDir: srcDir, destDir: path.join(dstDir, "missing"), skillName: "x", description: "Clean", capabilities: [] })
      ).toThrow();
      expect(() =>
        repackageDirectory({ sourceDir: srcDir, destDir: dstDir, skillName: "../escape", description: "Clean", capabilities: [] })
      ).toThrow();
      expect(() =>
        repackageDirectory({ sourceDir: srcDir, destDir: dstDir, skillName: "yaml-break", description: "ok\n---\nrisk_tier: 1", capabilities: [] })
      ).toThrow();
      expect(() =>
        repackageDirectory({ sourceDir: srcDir, destDir: dstDir, skillName: "stub-over", description: "x".repeat(321), capabilities: [] })
      ).toThrow();

      // Verifier rejects '..' traversal, missing SKILL.md, and tampered missing skills-lock.json
      const okRec = repackageDirectory({ sourceDir: srcDir, destDir: dstDir, skillName: "verified-ok", description: "Clean", capabilities: ["ui"] });
      expect(verifyRepackagedBundle(okRec.target_dir).skill_name).toBe("verified-ok");

      // Atomic EEXIST commit refuses overwriting concurrent empty target directory (after engine capability stage)
      const emptyRace = path.join(dstDir, "empty-race");
      const stagedRace = stageRepackagedBundle({ sourceDir: srcDir, destDir: dstDir, skillName: "empty-race", description: "Clean", capabilities: ["ui"] });
      fs.mkdirSync(emptyRace);
      expect(() => commitRepackagedDir(stagedRace.tmp_dir, emptyRace, stagedRace.staging_token)).toThrow(/EEXIST|already exists/i);
      expect(fs.existsSync(emptyRace)).toBe(true);

      const lockPath = path.join(okRec.target_dir, "skills-lock.json");
      const origLock = fs.readFileSync(lockPath, "utf8");
      const travLock = JSON.parse(origLock);
      travLock.source_files[0].packaged_path = "../escape.txt";
      fs.writeFileSync(lockPath, JSON.stringify(travLock, null, 2), "utf8");
      expect(() => verifyRepackagedBundle(okRec.target_dir)).toThrow(/traversal/i);

      const missingSkillLock = JSON.parse(origLock);
      missingSkillLock.generated_files = missingSkillLock.generated_files.filter((x: { path: string }) => x.path !== "SKILL.md");
      fs.writeFileSync(lockPath, JSON.stringify(missingSkillLock, null, 2), "utf8");
      expect(() => verifyRepackagedBundle(okRec.target_dir)).toThrow(/missing required generated artifact/i);

      // Intermediate symlinked subdirectory inside bundle fails closed (ELOOP / IntermediateSymlinkRejected)
      fs.writeFileSync(lockPath, origLock, "utf8");
      const outDir = makeTmp("cowok-outside-");
      fs.writeFileSync(path.join(outDir, "ok.ts"), "export const ok = 1;", "utf8");
      const scriptsDir = path.join(okRec.target_dir, "scripts");
      fs.rmSync(scriptsDir, { recursive: true, force: true });
      fs.symlinkSync(outDir, scriptsDir, "dir");
      expect(() => verifyRepackagedBundle(okRec.target_dir)).toThrow();

      // Symlinked subdirectory inside sourceDir rejected fail closed
      const symSrc = makeTmp("cowok-symsrc-");
      fs.symlinkSync(outDir, path.join(symSrc, "sub"), "dir");
      expect(() =>
        repackageDirectory({ sourceDir: symSrc, destDir: dstDir, skillName: "sym-sub-fail", description: "Clean", capabilities: [] })
      ).toThrow();

      fs.rmSync(lockPath);
      expect(() => verifyRepackagedBundle(okRec.target_dir)).toThrow();

      // Reserved root filename collision and quota depth > 8 reject fail closed
      const reservedSrc = makeTmp("cowok-reserved-");
      fs.writeFileSync(path.join(reservedSrc, "SKILL.md"), "# conflict", "utf8");
      expect(() =>
        repackageDirectory({ sourceDir: reservedSrc, destDir: dstDir, skillName: "reserved-fail", description: "Clean", capabilities: [] })
      ).toThrow(/ReservedPackageFilenameCollision/);

      const deepSrc = makeTmp("cowok-deep-");
      let nested = deepSrc;
      for (let i = 0; i < 10; i++) nested = path.join(nested, `d${i}`);
      fs.mkdirSync(nested, { recursive: true });
      fs.writeFileSync(path.join(nested, "leaf.ts"), "export const x = 1;", "utf8");
      expect(() =>
        repackageDirectory({ sourceDir: deepSrc, destDir: dstDir, skillName: "deep-fail", description: "Clean", capabilities: [] })
      ).toThrow(/PackageQuotaExceeded/);
    });

    it("enforces party-mode hardening: untracked files, hardlinks, scanner parity, strict video, and bridge limits", () => {
      expect(() => scanSkillMarkdownSafety("bad.html", "<a href='javascript&#x3A;alert(1)'>x</a>")).toThrow();
      expect(() => scanSkillMarkdownSafety("empty.md", "![]()")).toThrow();
      expect(() => scanSkillMarkdownSafety("css.html", "<style>@import url(https://evil.test/x.css);</style>")).toThrow();
      expect(scanExternalScriptSecurity("eq.js", "const same = a == 'eval' || b != 'fetch';").valid).toBe(true);
      expect(scanExternalScriptSecurity("brk.js", "globalThis[`eval`]('1+1');").valid).toBe(false);
      expect(scanExternalScriptSecurity("ex.js", "require('child_process')['exec']('id');").valid).toBe(false);
      expect(() => parseVideoStoryboard("   ", "[frame: 00:01.00] -> Click: CTA")).toThrow();
      expect(() => parseVideoStoryboard("demo.mp4", "[frame: 00:02.00] -> A: b\n[frame: 00:01.00] -> B: c")).toThrow();

      const src = makeTmp("cowok-pm-src-");
      const dst = makeTmp("cowok-pm-dst-");
      fs.writeFileSync(path.join(src, "helper.ts"), "export const ok = 1;", "utf8");
      const rec = repackageDirectory({
        sourceDir: src,
        destDir: dst,
        skillName: "pm-hardened",
        description: "Party mode verified bundle",
        capabilities: ["read_file"],
      });

      const dropin = path.join(rec.target_dir, "scripts", "extra.sh");
      fs.writeFileSync(dropin, "echo rogue", "utf8");
      expect(() => verifyRepackagedBundle(rec.target_dir)).toThrow(/untracked/i);
      const regBase = makeTmp("cowok-reg-");
      const skillsSub = path.join(regBase, "skills");
      fs.mkdirSync(skillsSub);
      fs.cpSync(rec.target_dir, path.join(skillsSub, "pm-hardened"), { recursive: true });
      const badReg = new BuiltinSkillsRegistry(regBase);
      badReg.discover();
      expect(badReg.getSkill("pm-hardened")).toBeUndefined();
      fs.rmSync(dropin);

      // Valid repackaged bundle loads into BuiltinSkillsRegistry when skills-lock.json verifies cleanly
      const goodBase = makeTmp("cowok-reg-ok-");
      fs.mkdirSync(path.join(goodBase, "skills"));
      fs.cpSync(rec.target_dir, path.join(goodBase, "skills", "pm-hardened"), { recursive: true });
      const goodReg = new BuiltinSkillsRegistry(goodBase);
      goodReg.discover();
      expect(goodReg.getSkill("pm-hardened")?.name).toBe("pm-hardened");

      const stagedHardlink = stageRepackagedBundle({ sourceDir: src, destDir: dst, skillName: "never-committed", description: "Party mode verified bundle", capabilities: ["read_file"] });
      fs.rmSync(path.join(stagedHardlink.tmp_dir, "scripts", "helper.ts"));
      fs.linkSync(path.join(src, "helper.ts"), path.join(stagedHardlink.tmp_dir, "scripts", "helper.ts"));
      const rejectedTarget = path.join(dst, "never-committed");
      expect(() => commitRepackagedDir(stagedHardlink.tmp_dir, rejectedTarget, stagedHardlink.staging_token)).toThrow(/HardlinkRejected/i);
      expect(fs.existsSync(rejectedTarget)).toBe(false);
    });

    it("enforces exhaustive quotas, entity/quote evasions, commit happy/cross-dir paths & content tamper detection", () => {
      expect(() => scanSkillMarkdownSafety("zpad.html", "<a href='javascript&#058;alert(1)'>x</a>")).toThrow();
      expect(() => scanSkillMarkdownSafety("zhx.html", "<a href='javascript&#x003A;alert(1)'>x</a>")).toThrow();
      expect(() => scanSkillMarkdownSafety("duri.html", "<a href='data:text/html,<b>hi</b>'>x</a>")).toThrow();
      expect(scanExternalScriptSecurity("mq.js", "window[\"ev\",   'al']('1');").valid).toBe(false);
      expect(scanExternalScriptSecurity("sh.sh", "ev\"\"al id").valid).toBe(false);
      for (const bad of golden.invalid_storyboards) {
        expect(() => parseVideoStoryboard("demo.mp4", bad.log)).toThrow();
      }

      const src = makeTmp("cowok-ext-src-");
      const dst = makeTmp("cowok-ext-dst-");
      const otherDst = makeTmp("cowok-ext-other-");
      fs.writeFileSync(path.join(src, "clean.ts"), "export const ok = 1;", "utf8");

      expect(() => repackageDirectory({ sourceDir: src, destDir: dst, skillName: "c17", description: "Ok", capabilities: Array.from({ length: 17 }, (_, i) => `c_${i}`) })).toThrow(/DiscoveryStub/);
      expect(() => repackageDirectory({ sourceDir: src, destDir: dst, skillName: "clen", description: "Ok", capabilities: ["c".repeat(49)] })).toThrow(/DiscoveryStub/);
      expect(() => repackageDirectory({ sourceDir: src, destDir: dst, skillName: "ulen", description: "Ok", capabilities: [], upstreamUrl: `https://example.com/${"u".repeat(250)}` })).toThrow(/DiscoveryStub/);
      expect(() =>
        buildDiscoveryStub({
          skillName: "s".repeat(64),
          description: "d".repeat(320),
          capabilities: Array.from({ length: 16 }, () => "c".repeat(48)),
          upstreamUrl: `https://example.com/${"u".repeat(236)}`,
          packageSha256: "a".repeat(64),
        })
      ).toThrow(/DiscoveryStubFailClosed/);

      const bigSrc = makeTmp("cowok-big-");
      fs.writeFileSync(path.join(bigSrc, "huge.txt"), Buffer.alloc(1048577, 97));
      expect(() => repackageDirectory({ sourceDir: bigSrc, destDir: dst, skillName: "huge-f", description: "Ok", capabilities: [] })).toThrow(/PackageQuotaExceeded/);

      const stagedHappy = stageRepackagedBundle({ sourceDir: src, destDir: dst, skillName: "committed-clean", description: "Clean", capabilities: ["ui"] });
      const targetOk = path.join(dst, "committed-clean");
      commitRepackagedDir(stagedHappy.tmp_dir, targetOk, stagedHappy.staging_token);
      expect(verifyRepackagedBundle(targetOk).skill_name).toBe("committed-clean");
      // Single-use staging_token cannot be replayed
      expect(() => commitRepackagedDir(stagedHappy.tmp_dir, path.join(dst, "replay-dst"), stagedHappy.staging_token)).toThrow(/MissingOrInvalidStagingCapability/);

      const stagedCross = stageRepackagedBundle({ sourceDir: src, destDir: dst, skillName: "x", description: "Clean", capabilities: ["ui"] });
      expect(() => commitRepackagedDir(stagedCross.tmp_dir, path.join(otherDst, "x"), stagedCross.staging_token)).toThrow(/CrossDirectoryCommitRejected/);
      expect(() => commitRepackagedDir(targetOk, path.join(dst, "non-tmp-commit"), stagedCross.staging_token)).toThrow(/InvalidStagingPrefix/);

      // Repackaged bundle missing skills-lock.json (with discovery-stub.json or 'repackaged' tag) rejected fail-closed by BuiltinSkillsRegistry
      const delBase = makeTmp("cowok-reg-del-");
      const delSkillDir = path.join(delBase, "skills", "committed-clean");
      fs.cpSync(targetOk, delSkillDir, { recursive: true });
      fs.rmSync(path.join(delSkillDir, "skills-lock.json"));
      const delReg = new BuiltinSkillsRegistry(delBase);
      delReg.discover();
      expect(delReg.getSkill("committed-clean")).toBeUndefined();

      // Immutable production trust root rejects missing compile-time pin and ignores env-var digest spoofing
      const debugBin = path.resolve(__dirname, "../../../target/debug/plugin_host_repackager");
      const debugBinSha = crypto.createHash("sha256").update(fs.readFileSync(debugBin)).digest("hex");
      expect(verifyPinnedBridgeBinary(debugBin, debugBinSha)).toBe(debugBin);
      expect(() => verifyPinnedBridgeBinary(debugBin, "0".repeat(64))).toThrow(/PluginHostBridgeUnavailable/);

      const prevEnvMode = process.env.NODE_ENV;
      const prevSha = process.env.COWOK_PLUGIN_HOST_REPACKAGER_SHA256;
      try {
        process.env.NODE_ENV = "production";
        delete process.env.COWOK_PLUGIN_HOST_REPACKAGER_SHA256;
        expect(() => verifyRepackagedBundle(targetOk)).toThrow(/PluginHostBridgeUnavailable/);
        process.env.COWOK_PLUGIN_HOST_REPACKAGER_SHA256 = debugBinSha;
        expect(() => verifyRepackagedBundle(targetOk)).toThrow(/PluginHostBridgeUnavailable/);
      } finally {
        if (prevEnvMode === undefined) delete process.env.NODE_ENV;
        else process.env.NODE_ENV = prevEnvMode;
        if (prevSha === undefined) delete process.env.COWOK_PLUGIN_HOST_REPACKAGER_SHA256;
        else process.env.COWOK_PLUGIN_HOST_REPACKAGER_SHA256 = prevSha;
      }

      // Forged staging directory / forged staging_token without engine issuance rejected
      const forgedStage = path.join(dst, ".tmp-forged-stage");
      fs.cpSync(targetOk, forgedStage, { recursive: true });
      const badPinDst = path.join(dst, "never-forged-dst");
      expect(() => commitRepackagedDir(forgedStage, badPinDst, "f".repeat(64))).toThrow(/MissingOrInvalidStagingCapability/);
      expect(fs.existsSync(badPinDst)).toBe(false);

      // Ancestor symlink directory rejected by component-wise O_NOFOLLOW traversal
      const ancReal = path.join(otherDst, "real_anc");
      fs.mkdirSync(path.join(ancReal, "nested"), { recursive: true });
      const ancSym = path.join(otherDst, "sym_anc");
      fs.symlinkSync(ancReal, ancSym, "dir");
      expect(() => verifyRepackagedBundle(path.join(ancSym, "nested"))).toThrow();

      // Scanner catches multi-char empty quotes, inline shell backslash escapes, and spaced bracket syntax
      for (const badPayload of [
        "e''v''a''l id",
        "ev\\al id",
        "globalThis[ 'eval' ]('1+1')",
        "window[ 'fetch' ]('https://evil.test')",
      ]) {
        expect(scanExternalScriptSecurity("bad.sh", badPayload).valid).toBe(false);
      }

      // Quota constants & manifestToSkillPlugin DTO conversion & pin-bridge build-time pinning
      expect(MAX_TRAVERSAL_DEPTH).toBe(8);
      expect(MAX_PACKAGE_FILES).toBe(256);
      expect(MAX_TOTAL_PACKAGE_BYTES).toBe(8388608);
      const okRec = repackageDirectory({ sourceDir: src, destDir: dst, skillName: "dto-check", description: "Clean", capabilities: ["ui"] });
      const pluginJson = JSON.parse(fs.readFileSync(path.join(okRec.target_dir, "plugin.json"), "utf8"));
      expect(manifestToSkillPlugin(pluginJson).permissions).toEqual(["ui"]);

      const validRegBase = makeTmp("cowok-reg-ok-");
      fs.cpSync(targetOk, path.join(validRegBase, "skills", "committed-clean"), { recursive: true });
      const okReg = new BuiltinSkillsRegistry(validRegBase);
      okReg.discover();
      expect(okReg.getSkill("committed-clean")?.name).toBe("committed-clean");

      const pinTmp = path.join(makeTmp("cowok-pin-"), "bridge-pin.ts");
      expect(writeBridgePinFile(debugBin, pinTmp)).toBe(debugBinSha);
      expect(verifyBridgePinFile(debugBin, pinTmp)).toBe(debugBinSha);

      fs.writeFileSync(path.join(targetOk, "scripts", "clean.ts"), "export const tampered = true;", "utf8");
      expect(() => verifyRepackagedBundle(targetOk)).toThrow(/BundleIntegrityMismatch/);

      expect(() =>
        buildDiscoveryStub({
          skillName: "ssrf-stub",
          description: "Clean",
          capabilities: [],
          upstreamUrl: "http://169.254.169.254/latest/meta-data",
          packageSha256: "a".repeat(64),
        })
      ).toThrow(/DiscoveryStub/);
      expect(() =>
        buildDiscoveryStub({
          skillName: "ssrf-stub-2",
          description: "Clean",
          capabilities: [],
          upstreamUrl: "https://127.0.0.1/repo",
          packageSha256: "a".repeat(64),
        })
      ).toThrow(/DiscoveryStub/);
    });

    it("enforces Round 4 parity: staged references/assets, script tells, entry_point & design-systems symlink rejection, and case-insensitive lint_and_fix", () => {
      // 1. Repackager security rejects require('fs'), node:vm/import('node:'), websocket(, xmlhttprequest
      for (const bad of [
        "const fs = require('fs');",
        "await import('node:child_process');",
        "new WebSocket('wss://evil.test');",
        "new XMLHttpRequest();",
      ]) {
        expect(scanExternalScriptSecurity("bad.js", bad).valid).toBe(false);
      }

      // 2. Staged bundle security rejects unsafe staged references and text assets
      const baseManifest = {
        name: "safe-skill",
        version: "1.0.0",
        description: "Clean skill",
        entry_point: "SKILL.md",
        capabilities_required: ["specs.read"],
        risk_tier: 1 as const,
        parameters: null,
        tags: [],
        prompt_overview: null,
      };
      const baseSkillMd =
        '---\nname: "safe-skill"\nversion: "1.0.0"\ndescription: "Clean skill"\ncapabilities_required: ["specs.read"]\nrisk_tier: 1\n---\n# Clean\n';
      expect(() =>
        validateStagedBundleSecurity({
          manifest: baseManifest,
          skill_md: baseSkillMd,
          scripts: [],
          references: [{ rel_path: "guide.md", content: "<script>alert(1)</script>" }],
        })
      ).toThrow(/StagingPromotionRejected/);
      expect(() =>
        validateStagedBundleSecurity({
          manifest: baseManifest,
          skill_md: baseSkillMd,
          scripts: [],
          assets: [{ rel_path: "badge.svg", bytes: Buffer.from("<svg onload=alert(1)/>", "utf8") }],
        })
      ).toThrow(/StagingPromotionRejected/);

      // 3. Registry rejects symlinked design-systems & missing entry_point / mismatched capabilities_required
      const regDir = makeTmp("cowok-r4-reg-");
      const outTarget = makeTmp("cowok-r4-out-");
      fs.symlinkSync(outTarget, path.join(regDir, "design-systems"), "dir");
      const badSkillDir = path.join(regDir, "skills", "bad-ep");
      fs.mkdirSync(badSkillDir, { recursive: true });
      const missingEntry = ["scripts", "missing.ts"].join("/");
      fs.writeFileSync(
        path.join(badSkillDir, "SKILL.md"),
        `---\nname: "bad-ep"\nversion: "1.0.0"\ndescription: "Clean"\nentry_point: "${missingEntry}"\ncapabilities_required: ["specs.read"]\nrisk_tier: 1\n---\n# Body\n`,
        "utf8"
      );
      const reg = new BuiltinSkillsRegistry(regDir);
      reg.discover();
      expect(reg.getDesignSystem("v1")).toBeUndefined();
      expect(reg.getSkill("bad-ep")).toBeUndefined();

      // 4. Case-insensitive autoFix, fixed_content on report, & <head>\n + mixed-line shift invariance
      const { content: fixedHtml, report } = TriangleLinter.lintAndFix(
        "<HTML lang='en'><HEAD>\n</HEAD>\n<p style='LINE-HEIGHT: 1; padding: 3px; color: #6366F1'>Hi</p></HTML>"
      );
      expect(fixedHtml).toContain('name="viewport"');
      expect(fixedHtml).toContain("line-height: 1.5;");
      expect(fixedHtml).toContain("var(--action-primary");
      expect(report.fixed_content).toBe(fixedHtml);
      expect(report.new_in_this_call).toBe(0);
    });
  });
});
