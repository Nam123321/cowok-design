import { describe, it, expect } from "vitest";
import * as path from "path";
import * as fs from "fs";
import {
  scanExternalScriptSecurity,
  parseVideoStoryboard,
  repackageDirectory,
  validateSkillFrontmatter,
} from "../src/index";

describe("Story 08.2: Skill Repackager, Discovery Stubs & Video Intake", () => {
  const tmpBase = path.resolve(__dirname, "..", ".test-repackager");

  describe("AC-03 & Edge Case Guard: Zero-Trust Security Scan", () => {
    it("passes clean and safe scripts", () => {
      const cleanJs = "function sum(a, b) { return a + b; }";
      expect(scanExternalScriptSecurity("calc.js", cleanJs).valid).toBe(true);
    });

    it("rejects command injection tells (curl | bash, child_process.exec)", () => {
      const dangerous = "curl | bash -s -- --install";
      const res = scanExternalScriptSecurity("install.sh", dangerous);
      expect(res.valid).toBe(false);
      expect(res.error).toContain("curl | bash");

      const execTell = "const child = child_process.exec('ls');";
      const res2 = scanExternalScriptSecurity("helper.js", execTell);
      expect(res2.valid).toBe(false);
      expect(res2.error).toContain("child_process.exec");
    });

    it("rejects arbitrary code evaluation (eval / Function constructor)", () => {
      const evalTell = "const result = eval(userPayload);";
      const res = scanExternalScriptSecurity("engine.ts", evalTell);
      expect(res.valid).toBe(false);
      expect(res.error).toContain("eval / Function");
    });

    it("rejects path traversal patterns", () => {
      const traversal = "const file = fs.readFileSync('../../../etc/passwd');";
      const res = scanExternalScriptSecurity("reader.js", traversal);
      expect(res.valid).toBe(false);
      expect(res.error).toContain("path traversal");
    });
  });

  describe("AC-04: Frame-Cited Video Reference Intake", () => {
    it("parses timestamped storyboard logs into structured frame checkpoints", () => {
      const transcript = `
[frame: 00:00.80] -> Hero Entrance: Fade in heading and brand CTA
[frame: 00:02.15] -> Button Hover: Scale 1.05 and soft focus ring
[frame: 00:03.90] -> Drawer Open: Slide in navigation drawer from left
`;
      const receipt = parseVideoStoryboard("checkout-flow.mp4", transcript);
      expect(receipt.total_checkpoints).toBe(3);
      expect(receipt.duration_s).toBe(3.90);
      expect(receipt.checkpoints[0].frame_id).toBe("frame_000.80");
      expect(receipt.checkpoints[0].action_label).toBe("Hero Entrance");
      expect(receipt.checkpoints[1].action_label).toBe("Button Hover");
      expect(receipt.checkpoints[2].action_label).toBe("Drawer Open");
    });
  });

  describe("AC-01 & AC-02: End-to-End Skill Repackager & Discovery Stub", () => {
    it("repackages raw directory into agentskills.io layout with discovery stub", () => {
      fs.mkdirSync(tmpBase, { recursive: true });
      const srcDir = path.join(tmpBase, "upstream-plugin");
      const dstDir = path.join(tmpBase, "packaged");
      fs.mkdirSync(srcDir, { recursive: true });

      // Create sample upstream files
      fs.writeFileSync(path.join(srcDir, "action.py"), "def run(): return 42");
      fs.writeFileSync(path.join(srcDir, "manual.md"), "# Manual\nDocumentation content");

      const receipt = repackageDirectory({
        sourceDir: srcDir,
        destDir: dstDir,
        skillName: "repackaged-action",
        description: "An automated helper repackaged from GitHub",
        capabilities: ["automation"],
        upstreamUrl: "https://github.com/example/action",
      });

      expect(receipt.skill_name).toBe("repackaged-action");
      expect(receipt.files_repackaged).toBeGreaterThanOrEqual(4);

      const packagedDir = path.join(dstDir, "repackaged-action");
      expect(fs.existsSync(path.join(packagedDir, "SKILL.md"))).toBe(true);
      expect(fs.existsSync(path.join(packagedDir, "plugin.json"))).toBe(true);
      expect(fs.existsSync(path.join(packagedDir, "discovery-stub.json"))).toBe(true);
      expect(fs.existsSync(path.join(packagedDir, "scripts", "action.py"))).toBe(true);
      expect(fs.existsSync(path.join(packagedDir, "references", "manual.md"))).toBe(true);

      // Verify SKILL.md adheres to frontmatter <= 100 tokens
      const skillMd = fs.readFileSync(path.join(packagedDir, "SKILL.md"), "utf8");
      const validation = validateSkillFrontmatter(skillMd);
      expect(validation.valid).toBe(true);
      expect(validation.tokens).toBeLessThanOrEqual(100);

      // Verify Discovery Stub metadata
      const stub = JSON.parse(fs.readFileSync(path.join(packagedDir, "discovery-stub.json"), "utf8"));
      expect(stub.name).toBe("repackaged-action");
      expect(stub.package_sha256).toBeDefined();
      expect(stub.package_sha256.length).toBe(64);

      // Cleanup
      fs.rmSync(tmpBase, { recursive: true, force: true });
    });
  });
});
