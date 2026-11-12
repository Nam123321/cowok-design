import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import { SkillManifest, StagingRunReceipt } from "./types";

export class PerRunStagingManager {
  private baseStagingDir: string;

  constructor(baseStagingDir: string) {
    this.baseStagingDir = baseStagingDir;
  }

  /**
   * Computes a deterministic SHA-256 hash across sorted skill definitions
   * to guarantee KV prompt cache stability.
   */
  public static computeStablePromptHash(skills: SkillManifest[]): string {
    const sorted = [...skills].sort((a, b) => a.name.localeCompare(b.name));
    const hasher = crypto.createHash("sha256");

    for (const skill of sorted) {
      hasher.update(skill.name);
      hasher.update("::");
      hasher.update(skill.version);
      hasher.update("::");
      hasher.update(skill.description);
      hasher.update("::");
      for (const cap of skill.capabilities_required || []) {
        hasher.update(cap);
        hasher.update(",");
      }
      hasher.update(";");
    }

    return hasher.digest("hex");
  }

  /**
   * Stages active skills into an isolated run directory (.cowok-skills/<run_id>/)
   */
  public stageRun(runId: string, skills: SkillManifest[]): StagingRunReceipt {
    const runDir = path.join(this.baseStagingDir, runId);
    fs.mkdirSync(runDir, { recursive: true });

    const stagedSkills: string[] = [];

    for (const skill of skills) {
      const skillDir = path.join(runDir, skill.name);
      fs.mkdirSync(skillDir, { recursive: true });

      const manifestPath = path.join(skillDir, "plugin.json");
      fs.writeFileSync(manifestPath, JSON.stringify(skill, null, 2), "utf8");

      if (skill.prompt_overview) {
        fs.writeFileSync(path.join(skillDir, "PROMPT.md"), skill.prompt_overview, "utf8");
      }

      stagedSkills.push(skill.name);
    }

    const stablePromptHash = PerRunStagingManager.computeStablePromptHash(skills);
    const receipt: StagingRunReceipt = {
      run_id: runId,
      staged_skills: stagedSkills,
      stable_prompt_hash: stablePromptHash,
      staged_dir: runDir,
      timestamp: Date.now(),
    };

    fs.writeFileSync(path.join(runDir, "run-receipt.json"), JSON.stringify(receipt, null, 2), "utf8");
    return receipt;
  }
}
