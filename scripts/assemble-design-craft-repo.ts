import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export interface AssembleOptions {
  repoRoot: string;
  outputDir: string;
}

export interface SkillFrontmatterCheck {
  relativePath: string;
  name: string;
  tokens: number;
  bytes: number;
}

export interface AssembleReport {
  outputDir: string;
  manifestVersion: string;
  phaseSkills: string[];
  domainSkills: string[];
  craftSkills: string[];
  agents: string[];
  builderTools: string[];
  qcAuditTools: string[];
  validatedSkills: SkillFrontmatterCheck[];
  backReferenceViolations: string[];
}

const MAX_HEADER_TOKENS = 100;
const MAX_HEADER_BYTES = 1024;
const FORBIDDEN_BACKREFS = [
  /packages\/builtin-skills/,
  /@cowok-design\//,
  /crates\//,
  /src\/features\//,
];
const IMPORT_SPECIFIER_RE =
  /(?:\b(?:import|export)\s+(?:[^'"]*?\s+from\s+)?|\bimport\s*\(\s*)["']([^"']+)["']/g;

function estimateTokenCount(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const charEst = Math.ceil(trimmed.length / 4);
  const wordEst = Math.ceil(trimmed.split(/\s+/).length * 1.33);
  return Math.max(charEst, wordEst);
}

function validateSkillFrontmatter(filePath: string, relPath: string): SkillFrontmatterCheck {
  const raw = fs.readFileSync(filePath, "utf8").replace(/^\uFEFF/, "");
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/);
  if (!match) {
    throw new Error(`Invalid SKILL.md frontmatter delimiters in ${relPath}`);
  }
  const body = match[1].trim();
  const bytes = Buffer.byteLength(body, "utf8");
  const tokens = estimateTokenCount(body);
  if (bytes > MAX_HEADER_BYTES) {
    throw new Error(`SKILL.md frontmatter exceeds ${MAX_HEADER_BYTES} bytes (${bytes}) in ${relPath}`);
  }
  if (tokens > MAX_HEADER_TOKENS) {
    throw new Error(`SKILL.md frontmatter exceeds ${MAX_HEADER_TOKENS} tokens (${tokens}) in ${relPath}`);
  }
  const nameMatch = body.match(/^(?:name|id|title):\s*["']?([^"'\r\n]+)["']?/m);
  const descMatch = body.match(/^(?:description|desc):\s*["']?([^"'\r\n]+)["']?/m);
  if (!nameMatch || !descMatch) {
    throw new Error(`SKILL.md frontmatter missing required name/description in ${relPath}`);
  }
  return { relativePath: relPath, name: nameMatch[1].trim(), tokens, bytes };
}

function copyDirRecursive(srcDir: string, dstDir: string): void {
  fs.mkdirSync(dstDir, { recursive: true });
  for (const entry of fs.readdirSync(srcDir, { withFileTypes: true })) {
    const srcPath = path.join(srcDir, entry.name);
    const dstPath = path.join(dstDir, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Symlink not allowed during clean-room assembly: ${srcPath}`);
    }
    if (entry.isDirectory()) {
      copyDirRecursive(srcPath, dstPath);
    } else if (entry.isFile()) {
      fs.copyFileSync(srcPath, dstPath);
    }
  }
}

function collectFilesRecursive(rootDir: string, subDir = ""): string[] {
  const current = subDir ? path.join(rootDir, subDir) : rootDir;
  if (!fs.existsSync(current)) return [];
  const results: string[] = [];
  for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
    const rel = subDir ? path.join(subDir, entry.name) : entry.name;
    if (entry.isDirectory()) {
      results.push(...collectFilesRecursive(rootDir, rel));
    } else if (entry.isFile()) {
      results.push(rel);
    }
  }
  return results.sort();
}

function writeStandalonePackageJson(outputDir: string, version: string): void {
  const pkg = {
    name: "design-craft",
    version,
    type: "module",
    license: "MIT",
    description: "Standalone Cognitive Progressive Design & Presentation Operating Kit (agentskills.io/v1)",
    bin: {
      "design-craft": "./bin/design-craft.mjs",
      "design-craft-mcp": "./mcp/server.mjs",
    },
    engines: { node: ">=20.0.0" },
    dependencies: {},
  };
  fs.writeFileSync(path.join(outputDir, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
}

function auditMjsImports(outputDir: string): string[] {
  const mjsFiles = collectFilesRecursive(outputDir).filter((f) => f.endsWith(".mjs"));
  const violations: string[] = [];
  for (const rel of mjsFiles) {
    const text = fs.readFileSync(path.join(outputDir, rel), "utf8");
    for (const match of text.matchAll(IMPORT_SPECIFIER_RE)) {
      const spec = match[1];
      const allowed = spec.startsWith("node:") || spec.startsWith("./") || spec.startsWith("../");
      if (!allowed) {
        violations.push(`${rel} imports disallowed external specifier '${spec}'`);
      }
    }
  }
  return violations;
}

function auditNoCowokBackRefs(outputDir: string): string[] {
  const scanDirs = ["engine", "mcp", "bin", "flows", "agents", "catalogs", "rubrics", "skills", "templates"];
  const files = ["package.json", ...scanDirs.flatMap((d) => collectFilesRecursive(outputDir, d))];
  const violations: string[] = [];
  for (const rel of files) {
    const abs = path.join(outputDir, rel);
    if (!fs.existsSync(abs)) continue;
    const text = fs.readFileSync(abs, "utf8");
    for (const pattern of FORBIDDEN_BACKREFS) {
      if (pattern.test(text)) {
        violations.push(`${rel} matched forbidden back-reference ${pattern}`);
      }
    }
  }
  violations.push(...auditMjsImports(outputDir));
  return violations;
}

function assertSafeOutputDir(resolvedRepoRoot: string, resolvedOut: string): void {
  const fsRoot = path.parse(resolvedOut).root;
  if (
    resolvedOut === fsRoot ||
    resolvedOut === resolvedRepoRoot ||
    resolvedRepoRoot.startsWith(resolvedOut + path.sep)
  ) {
    throw new Error(`Refusing destructive clean-room rmSync on unsafe outputDir '${resolvedOut}'`);
  }
}

function listImmediateDirs(dirPath: string): string[] {
  if (!fs.existsSync(dirPath)) return [];
  return fs
    .readdirSync(dirPath, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();
}

function writeSemverGateWorkflow(outputDir: string): void {
  const workflowDir = path.join(outputDir, ".github", "workflows");
  fs.mkdirSync(workflowDir, { recursive: true });
  const workflowYaml = `name: design-craft SemVer & Anti-Goodhart Gate

on:
  pull_request:
    branches: [main, master]
  workflow_dispatch:

permissions: read-all

jobs:
  semver-and-ablation-gate:
    name: Verify agentskills.io Frontmatter & Paired Ablation Gate
    runs-on: ubuntu-latest
    steps:
      - name: Checkout design-craft
        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2

      - name: Setup Node.js 22
        uses: actions/setup-node@1d0ff469b7ec7b3cb9d8673fde0c81c44821de2a # v4.2.0
        with:
          node-version: 22

      - name: Validate plugin.json & SKILL.md budgets (<=100 tokens, <=1024 bytes)
        run: |
          node -e '
            const fs = require("node:fs");
            const path = require("node:path");
            const manifest = JSON.parse(fs.readFileSync("plugin.json", "utf8"));
            if (!/^1\\.\\d+\\.\\d+/.test(String(manifest.version || ""))) {
              throw new Error("plugin.json version must be SemVer 1.x: " + manifest.version);
            }
            console.log("Verified design-craft plugin.json version:", manifest.version);
          '

      - name: Run cowok-design Stage Paired Ablation Gate (when CLI available)
        run: |
          if command -v cowok-design >/dev/null 2>&1; then
            cowok-design eval run --tier stage --candidate-dir . --json
          else
            echo "::notice::Install cowok-design CLI to run live Story 13.1 paired ablation gate in CI."
          fi
`;
  fs.writeFileSync(path.join(workflowDir, "semver-gate.yml"), workflowYaml, "utf8");
}

function makeExecutablesRunnable(outputDir: string): void {
  for (const rel of ["bin/design-craft.mjs", "mcp/server.mjs"]) {
    const abs = path.join(outputDir, rel);
    if (fs.existsSync(abs)) {
      fs.chmodSync(abs, 0o755);
    }
  }
}

export function assembleDesignCraftRepo(options: AssembleOptions): AssembleReport {
  const resolvedRepoRoot = path.resolve(options.repoRoot);
  const resolvedOut = path.resolve(options.outputDir);
  assertSafeOutputDir(resolvedRepoRoot, resolvedOut);
  const canonicalBundle = path.join(resolvedRepoRoot, "packages", "design-craft");
  const fallbackBundle = path.join(resolvedRepoRoot, "packages", "builtin-skills", "design-craft");
  const sourceBundle = fs.existsSync(path.join(canonicalBundle, "plugin.json"))
    ? canonicalBundle
    : fallbackBundle;
  const forgedSkillsDir = path.join(resolvedRepoRoot, "packages", "builtin-skills", "skills");
  fs.rmSync(resolvedOut, { recursive: true, force: true });
  copyDirRecursive(sourceBundle, resolvedOut);
  if (fs.existsSync(forgedSkillsDir)) {
    const existingSkills = new Set<string>([
      ...listImmediateDirs(path.join(resolvedOut, "skills", "craft")),
      ...listImmediateDirs(path.join(resolvedOut, "skills", "domain-business")),
      ...listImmediateDirs(path.join(resolvedOut, "skills")),
    ]);
    for (const skillName of listImmediateDirs(forgedSkillsDir)) {
      if (existingSkills.has(skillName)) {
        // Never overwrite upgraded v2.x skills already present in design-craft with legacy v1.0.0 copies
        continue;
      }
      copyDirRecursive(path.join(forgedSkillsDir, skillName), path.join(resolvedOut, "skills", "craft", skillName));
    }
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(resolvedOut, "plugin.json"), "utf8"));
  writeStandalonePackageJson(resolvedOut, String(manifest.version ?? "1.0.0"));
  writeSemverGateWorkflow(resolvedOut);
  makeExecutablesRunnable(resolvedOut);
  const skillMdFiles = collectFilesRecursive(resolvedOut, "skills").filter((f) => path.basename(f) === "SKILL.md");
  const validatedSkills = skillMdFiles.map((rel) => validateSkillFrontmatter(path.join(resolvedOut, rel), rel));
  const backReferenceViolations = auditNoCowokBackRefs(resolvedOut);
  if (backReferenceViolations.length > 0) {
    throw new Error(`Back-reference leakage detected:\n${backReferenceViolations.join("\n")}`);
  }
  const phaseSkills = listImmediateDirs(path.join(resolvedOut, "skills")).filter((d) => d.startsWith("phase-"));
  return {
    outputDir: resolvedOut,
    manifestVersion: manifest.version,
    phaseSkills,
    domainSkills: listImmediateDirs(path.join(resolvedOut, "skills", "domain-business")),
    craftSkills: listImmediateDirs(path.join(resolvedOut, "skills", "craft")),
    agents: manifest.agents ?? [],
    builderTools: manifest.capabilities?.builderTools ?? [],
    qcAuditTools: manifest.capabilities?.qcAuditTools ?? [],
    validatedSkills,
    backReferenceViolations,
  };
}

function runCli(): void {
  const outIdx = process.argv.indexOf("--out");
  const outputDir = outIdx >= 0 && process.argv[outIdx + 1] ? process.argv[outIdx + 1] : "";
  if (!outputDir) {
    process.stderr.write("Usage: assemble-design-craft-repo.ts --out <outputDir>\n");
    process.exit(1);
  }
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const report = assembleDesignCraftRepo({ repoRoot, outputDir });
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
}

function isMainEntry(): boolean {
  if (!process.argv[1]) return false;
  try {
    return pathToFileURL(fs.realpathSync(path.resolve(process.argv[1]))).href ===
      pathToFileURL(fs.realpathSync(fileURLToPath(import.meta.url))).href;
  } catch {
    return false;
  }
}

if (isMainEntry()) {
  runCli();
}
