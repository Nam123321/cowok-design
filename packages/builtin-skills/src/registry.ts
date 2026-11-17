// @story 08.1
// @implements AC-01, AC-02, AC-05, AC-06
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import {
  SkillManifest,
  SkillManifestSchema,
  CraftRule,
  DesignTemplate,
  DesignSystemV1,
  ScenarioDef,
  AtomDef,
} from "./types";
import { StagedSkillSource } from "./staging";
import { validateSkillFrontmatter, ValidationResult } from "./validator";
import { readVerifiedRegularUtf8, verifyRepackagedBundle } from "./repackager-bundle";

function sha256Hex(content: string): string {
  return crypto.createHash("sha256").update(content, "utf8").digest("hex");
}

function isSafeSkillName(name: string): boolean {
  return Boolean(name) && name !== "." && name !== ".." && /^[a-zA-Z0-9._-]+$/.test(name);
}

function verifyFrontmatterParity(
  fields: NonNullable<ValidationResult["fields"]>,
  manifest: SkillManifest,
): void {
  if (fields.name !== manifest.name) {
    throw new Error(`SKILL.md name '${fields.name}' does not match plugin.json '${manifest.name}'`);
  }
  if (fields.version !== manifest.version) {
    throw new Error(`SKILL.md version '${fields.version ?? ""}' does not match plugin.json '${manifest.version}'`);
  }
  if (fields.description !== manifest.description) {
    throw new Error(`SKILL.md description mismatch for '${manifest.name}'`);
  }
  if (fields.risk_tier !== manifest.risk_tier) {
    throw new Error(`SKILL.md risk_tier '${fields.risk_tier ?? ""}' does not match plugin.json ${manifest.risk_tier}`);
  }
}

export class BuiltinSkillsRegistry {
  private baseDir: string;
  private skills: Map<string, SkillManifest> = new Map();
  private skillMdHashes: Map<string, string> = new Map();
  private craftRules: Map<string, CraftRule> = new Map();
  private designTemplates: Map<string, DesignTemplate> = new Map();
  private designSystems: Map<string, DesignSystemV1> = new Map();
  private scenarios: Map<string, ScenarioDef> = new Map();
  private atoms: Map<string, AtomDef> = new Map();
  private promptTemplates: Map<string, string> = new Map();

  constructor(baseDir: string) {
    this.baseDir = baseDir;
  }

  public discover(): void {
    if (!fs.existsSync(this.baseDir)) return;

    const skillsDir = path.join(this.baseDir, "skills");
    if (fs.existsSync(skillsDir)) {
      const entries = fs.readdirSync(skillsDir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name));
      for (const entry of entries) {
        if (entry.isDirectory() && !entry.isSymbolicLink()) {
          const folder = path.join(skillsDir, entry.name);
          const loaded = this.loadSkillFolderWithHash(folder);
          if (loaded) {
            this.skills.set(loaded.manifest.name, loaded.manifest);
            if (loaded.skillMdSha256) {
              this.skillMdHashes.set(loaded.manifest.name, loaded.skillMdSha256);
            }
          }
        }
      }
    }

    const craftDir = path.join(this.baseDir, "craft");
    if (fs.existsSync(craftDir)) {
      for (const file of fs.readdirSync(craftDir).sort()) {
        if (file.endsWith(".md")) {
          const id = path.basename(file, ".md");
          const content = fs.readFileSync(path.join(craftDir, file), "utf8");
          this.craftRules.set(id, {
            id,
            title: id.replace(/-/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
            category: (id.includes("typo")
              ? "typography"
              : id.includes("spac")
              ? "spacing"
              : id.includes("color")
              ? "color"
              : id.includes("hier")
              ? "hierarchy"
              : "interactions") as CraftRule["category"],
            summary: content.split("\n\n")[0]?.slice(0, 150) || "",
            content,
          });
        }
      }
    }

    this.discoverJsonAxis<DesignTemplate>("design-templates", this.designTemplates);
    this.discoverDesignSystems();
    this.discoverJsonAxis<ScenarioDef>("scenarios", this.scenarios);
    this.discoverJsonAxis<AtomDef>("atoms", this.atoms);

    const promptsDir = path.join(this.baseDir, "prompt-templates");
    if (fs.existsSync(promptsDir)) {
      for (const file of fs.readdirSync(promptsDir).sort()) {
        if (file.endsWith(".md")) {
          const id = path.basename(file, ".md");
          this.promptTemplates.set(id, fs.readFileSync(path.join(promptsDir, file), "utf8"));
        }
      }
    }
  }

  private discoverJsonAxis<T extends { id: string }>(dirName: string, target: Map<string, T>): void {
    const dir = path.join(this.baseDir, dirName);
    if (!fs.existsSync(dir)) return;
    for (const file of fs.readdirSync(dir).sort()) {
      if (!file.endsWith(".json")) continue;
      try {
        const parsed = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8")) as T;
        target.set(parsed.id, parsed);
      } catch {
        // Ignore malformed optional asset files
      }
    }
  }

  private discoverDesignSystems(): void {
    const dsDir = path.join(this.baseDir, "design-systems");
    if (!fs.existsSync(dsDir)) return;
    for (const ver of fs.readdirSync(dsDir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (!ver.isDirectory()) continue;
      const verPath = path.join(dsDir, ver.name);
      const tokensFile = path.join(verPath, "dtcg-tokens.json");
      const mapFile = path.join(verPath, "mapping.json");
      if (!fs.existsSync(tokensFile)) continue;
      try {
        const tokens = JSON.parse(fs.readFileSync(tokensFile, "utf8"));
        const mapping = fs.existsSync(mapFile) ? JSON.parse(fs.readFileSync(mapFile, "utf8")) : {};
        this.designSystems.set(ver.name, { version: "v1", name: `Cowok Design System ${ver.name}`, tokens, mapping });
      } catch {
        // Ignore malformed design system
      }
    }
  }

  private loadSkillFolderWithHash(folder: string): { manifest: SkillManifest; skillMdSha256?: string } | null {
    let verifiedRepackaged = false;
    try {
      if (fs.lstatSync(folder).isSymbolicLink()) return null;
      if (
        fs.existsSync(path.join(folder, "skills-lock.json")) ||
        fs.existsSync(path.join(folder, "discovery-stub.json"))
      ) {
        verifyRepackagedBundle(folder);
        verifiedRepackaged = true;
      }
    } catch {
      return null;
    }

    const pluginJsonPath = path.join(folder, "plugin.json");
    const skillMdPath = path.join(folder, "SKILL.md");

    let fields: ValidationResult["fields"];
    let skillMdSha256: string | undefined;

    if (fs.existsSync(skillMdPath)) {
      try {
        const raw = readVerifiedRegularUtf8(skillMdPath);
        const validation = validateSkillFrontmatter(raw);
        if (!validation.valid || !validation.fields) return null;
        fields = validation.fields;
        skillMdSha256 = sha256Hex(raw);
      } catch {
        return null;
      }
    }

    if (fs.existsSync(pluginJsonPath)) {
      try {
        const rawJson = readVerifiedRegularUtf8(pluginJsonPath);
        const parsed = SkillManifestSchema.parse(JSON.parse(rawJson));
        if (parsed.tags.includes("repackaged") && !verifiedRepackaged) verifyRepackagedBundle(folder);
        if (!isSafeSkillName(parsed.name)) return null;
        if (fields) verifyFrontmatterParity(fields, parsed);
        return { manifest: parsed, skillMdSha256 };
      } catch {
        return null;
      }
    }

    if (fields?.name && fields.description && isSafeSkillName(fields.name)) {
      return {
        manifest: {
          name: fields.name,
          version: fields.version || "1.0.0",
          description: fields.description,
          entry_point: "SKILL.md",
          capabilities_required: [],
          risk_tier: (fields.risk_tier || 1) as 1 | 2 | 3,
          tags: [],
        },
        skillMdSha256,
      };
    }

    return null;
  }

  public getSkillMdSha256(name: string): string | undefined {
    return this.skillMdHashes.get(name);
  }

  public getStagedSources(): StagedSkillSource[] {
    return this.listSkills().map((manifest) => ({
      manifest,
      skillMdSha256: this.skillMdHashes.get(manifest.name) ?? null,
    }));
  }

  public getSkill(name: string): SkillManifest | undefined {
    return this.skills.get(name);
  }

  public listSkills(): SkillManifest[] {
    return Array.from(this.skills.values()).sort((a, b) => a.name.localeCompare(b.name));
  }

  public getCraftRule(id: string): CraftRule | undefined {
    return this.craftRules.get(id);
  }

  public listCraftRules(): CraftRule[] {
    return Array.from(this.craftRules.values()).sort((a, b) => a.id.localeCompare(b.id));
  }

  public getDesignTemplate(id: string): DesignTemplate | undefined {
    return this.designTemplates.get(id);
  }

  public listDesignTemplates(): DesignTemplate[] {
    return Array.from(this.designTemplates.values()).sort((a, b) => a.id.localeCompare(b.id));
  }

  public getDesignSystem(version: string): DesignSystemV1 | undefined {
    return this.designSystems.get(version);
  }

  public getScenario(id: string): ScenarioDef | undefined {
    return this.scenarios.get(id);
  }

  public listScenarios(): ScenarioDef[] {
    return Array.from(this.scenarios.values()).sort((a, b) => a.id.localeCompare(b.id));
  }

  public getAtom(id: string): AtomDef | undefined {
    return this.atoms.get(id);
  }

  public listAtoms(): AtomDef[] {
    return Array.from(this.atoms.values()).sort((a, b) => a.id.localeCompare(b.id));
  }

  public getPromptTemplate(id: string): string | undefined {
    return this.promptTemplates.get(id);
  }
}
