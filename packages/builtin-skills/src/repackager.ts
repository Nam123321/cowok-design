import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import { SkillManifest } from "./types";
import { validateSkillFrontmatter } from "./validator";

export interface VideoFrameCheckpoint {
  frame_id: string;
  timestamp_s: number;
  action_label: string;
  ui_intent: string;
  coordinate_bounds?: [number, number, number, number];
}

export interface VideoStoryboardReceipt {
  video_source: string;
  duration_s: number;
  total_checkpoints: number;
  checkpoints: VideoFrameCheckpoint[];
  generated_at: number;
}

export interface DiscoveryStub {
  name: string;
  version: string;
  description: string;
  upstream_url?: string;
  capabilities: string[];
  risk_tier: number;
  package_sha256: string;
  stub_size_bytes: number;
}

export interface RepackageReceipt {
  skill_name: string;
  version: string;
  target_dir: string;
  files_repackaged: number;
  discovery_stub: DiscoveryStub;
  created_at: number;
}

/**
 * Scans external scripts for dangerous code execution patterns (AC-03)
 */
export function scanExternalScriptSecurity(
  filename: string,
  content: string
): { valid: boolean; error?: string } {
  const lines = content.split("\n");

  const dangerousCommands = [
    "curl | bash",
    "curl | sh",
    "wget -o- | bash",
    "child_process.exec",
    "child_process.spawn",
    "system(",
    "popen(",
    "subprocess.call",
    "subprocess.popen",
    "rm -rf /",
  ];

  for (let idx = 0; idx < lines.length; idx++) {
    const line = lines[idx];
    const lineNum = idx + 1;
    const lower = line.toLowerCase();

    for (const cmd of dangerousCommands) {
      if (lower.includes(cmd)) {
        return {
          valid: false,
          error: `Detected dangerous command execution pattern: ${cmd} at line ${lineNum}`,
        };
      }
    }

    if (
      lower.includes("eval(") ||
      lower.includes("new function(") ||
      lower.includes("window.eval") ||
      (lower.includes("exec(") && (filename.endsWith(".py") || filename.endsWith(".js") || filename.endsWith(".ts")))
    ) {
      return {
        valid: false,
        error: `Detected arbitrary code evaluation (eval / Function) at line ${lineNum}`,
      };
    }

    if (lower.includes("../../../") || lower.includes("/etc/passwd") || lower.includes("/root/")) {
      return {
        valid: false,
        error: `Detected path traversal pattern at line ${lineNum}`,
      };
    }
  }

  return { valid: true };
}

/**
 * Parses timestamped video storyboard transcripts into structured frame checkpoints (AC-04)
 */
export function parseVideoStoryboard(videoSource: string, rawLog: string): VideoStoryboardReceipt {
  const checkpoints: VideoFrameCheckpoint[] = [];
  let durationMax = 0;
  const lines = rawLog.split("\n");

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("[")) continue;

    const closeBracket = trimmed.indexOf("]");
    if (closeBracket === -1) continue;

    const header = trimmed.slice(1, closeBracket);
    const rest = trimmed.slice(closeBracket + 1).trim();

    // Parse timestamp (e.g. "frame: 00:01.25" or "01:23.45")
    const colonIdx = header.indexOf(":");
    const timeStr = (colonIdx !== -1 ? header.slice(colonIdx + 1) : header).trim();
    const parts = timeStr.split(":").map(Number);
    let timestamp_s = 0;
    if (parts.length === 1) timestamp_s = parts[0] || 0;
    else if (parts.length === 2) timestamp_s = (parts[0] || 0) * 60 + (parts[1] || 0);
    else if (parts.length === 3) timestamp_s = (parts[0] || 0) * 3600 + (parts[1] || 0) * 60 + (parts[2] || 0);

    if (timestamp_s > durationMax) durationMax = timestamp_s;

    let action_label = "Observed Frame";
    let ui_intent = rest;

    const arrowIdx = rest.indexOf("->");
    if (arrowIdx !== -1) {
      const afterArrow = rest.slice(arrowIdx + 2).trim();
      const subColon = afterArrow.indexOf(":");
      if (subColon !== -1) {
        action_label = afterArrow.slice(0, subColon).trim();
        ui_intent = afterArrow.slice(subColon + 1).trim();
      } else {
        action_label = afterArrow;
        ui_intent = "User interaction";
      }
    }

    const frame_id = `frame_${timestamp_s.toFixed(2).padStart(6, "0")}`;
    checkpoints.push({
      frame_id,
      timestamp_s,
      action_label,
      ui_intent,
    });
  }

  return {
    video_source: videoSource,
    duration_s: durationMax,
    total_checkpoints: checkpoints.length,
    checkpoints,
    generated_at: Date.now(),
  };
}

/**
 * Repackages an external skill directory into certified agentskills.io format
 */
export function repackageDirectory(params: {
  sourceDir: string;
  destDir: string;
  skillName: string;
  description: string;
  capabilities: string[];
  upstreamUrl?: string;
}): RepackageReceipt {
  const { sourceDir, destDir, skillName, description, capabilities, upstreamUrl } = params;
  const dst = path.join(destDir, skillName);

  fs.mkdirSync(dst, { recursive: true });
  const scriptsDir = path.join(dst, "scripts");
  const refsDir = path.join(dst, "references");
  const assetsDir = path.join(dst, "assets");
  fs.mkdirSync(scriptsDir, { recursive: true });
  fs.mkdirSync(refsDir, { recursive: true });
  fs.mkdirSync(assetsDir, { recursive: true });

  let filesCount = 0;
  const hasher = crypto.createHash("sha256");

  const entries = fs.readdirSync(sourceDir);
  for (const entry of entries) {
    const srcPath = path.join(sourceDir, entry);
    const stat = fs.statSync(srcPath);

    if (stat.isFile()) {
      const content = fs.readFileSync(srcPath, "utf8");
      const sec = scanExternalScriptSecurity(entry, content);
      if (!sec.valid) {
        throw new Error(`Security scan failed on ${entry}: ${sec.error}`);
      }

      hasher.update(content);
      filesCount++;

      if (entry.endsWith(".py") || entry.endsWith(".js") || entry.endsWith(".ts") || entry.endsWith(".sh")) {
        fs.copyFileSync(srcPath, path.join(scriptsDir, entry));
      } else if (entry.endsWith(".md") || entry.endsWith(".txt") || entry.endsWith(".json")) {
        fs.copyFileSync(srcPath, path.join(refsDir, entry));
      } else {
        fs.copyFileSync(srcPath, path.join(assetsDir, entry));
      }
    }
  }

  const package_sha256 = hasher.digest("hex");

  // Compact SKILL.md
  const skillMdContent = `---
name: ${skillName}
version: 1.0.0
description: ${description}
capabilities_required: ${JSON.stringify(capabilities)}
risk_tier: 2
---

# ${skillName} Operating Skill

## 1. Overview
${description}

## 2. Repackaged Structure
This skill was repackaged into standard agentskills.io layout from external sources.
`;

  const valid = validateSkillFrontmatter(skillMdContent);
  if (!valid.valid) {
    throw new Error(`Generated SKILL.md violated frontmatter rule: ${valid.error}`);
  }

  fs.writeFileSync(path.join(dst, "SKILL.md"), skillMdContent, "utf8");
  filesCount++;

  // plugin.json
  const manifest: SkillManifest = {
    name: skillName,
    version: "1.0.0",
    description,
    entry_point: "SKILL.md",
    capabilities_required: capabilities,
    risk_tier: 2,
    tags: ["repackaged"],
    prompt_overview: description,
  };
  fs.writeFileSync(path.join(dst, "plugin.json"), JSON.stringify(manifest, null, 2), "utf8");
  filesCount++;

  // discovery-stub.json (~1.2KB) (AC-04)
  const stub: DiscoveryStub = {
    name: skillName,
    version: "1.0.0",
    description,
    upstream_url: upstreamUrl,
    capabilities,
    risk_tier: 2,
    package_sha256,
    stub_size_bytes: 1200,
  };
  fs.writeFileSync(path.join(dst, "discovery-stub.json"), JSON.stringify(stub, null, 2), "utf8");
  filesCount++;

  return {
    skill_name: skillName,
    version: "1.0.0",
    target_dir: dst,
    files_repackaged: filesCount,
    discovery_stub: stub,
    created_at: Date.now(),
  };
}
