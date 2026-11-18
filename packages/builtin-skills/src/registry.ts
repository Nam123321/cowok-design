// @story 08.1
// @implements AC-01, AC-02, AC-05, AC-06
import * as fs from "fs";
import * as path from "path";
import {
  SkillManifest,
  CraftRule,
  DesignTemplate,
  DesignSystemV1,
  ScenarioDef,
  AtomDef,
} from "./types";
import { StagedSkillSource } from "./staging";
import { readVerifiedRegularUtf8 } from "./repackager-bundle";
import { loadSkillFolderWithHash, lstatSafe } from "./registry-loader";

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
    this.discoverSkillsAxis();
    this.discoverCraftRules();
    this.discoverJsonAxis<DesignTemplate>("design-templates", this.designTemplates);
    this.discoverDesignSystems();
    this.discoverJsonAxis<ScenarioDef>("scenarios", this.scenarios);
    this.discoverJsonAxis<AtomDef>("atoms", this.atoms);
    this.discoverPromptTemplates();
  }

  private discoverSkillsAxis(): void {
    const skillsDir = path.join(this.baseDir, "skills");
    const dirStat = lstatSafe(skillsDir);
    if (!dirStat || dirStat.isSymbolicLink() || !dirStat.isDirectory()) return;
    const entries = fs.readdirSync(skillsDir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name));
    for (const entry of entries) {
      if (!entry.isDirectory() || entry.isSymbolicLink()) continue;
      const folder = path.join(skillsDir, entry.name);
      const loaded = loadSkillFolderWithHash(folder);
      if (!loaded) continue;
      this.skills.set(loaded.manifest.name, loaded.manifest);
      if (loaded.skillMdSha256) this.skillMdHashes.set(loaded.manifest.name, loaded.skillMdSha256);
    }
  }

  private discoverCraftRules(): void {
    const craftDir = path.join(this.baseDir, "craft");
    const dirStat = lstatSafe(craftDir);
    if (!dirStat || dirStat.isSymbolicLink() || !dirStat.isDirectory()) return;
    for (const file of fs.readdirSync(craftDir).sort()) {
      if (!file.endsWith(".md")) continue;
      try {
        const id = path.basename(file, ".md");
        const content = readVerifiedRegularUtf8(path.join(craftDir, file));
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
      } catch (err) {
        console.warn(`[BuiltinSkillsRegistry] skipping unsafe/unreadable craft/${file}:`, err instanceof Error ? err.message : String(err));
      }
    }
  }

  private discoverPromptTemplates(): void {
    const promptsDir = path.join(this.baseDir, "prompt-templates");
    const dirStat = lstatSafe(promptsDir);
    if (!dirStat || dirStat.isSymbolicLink() || !dirStat.isDirectory()) return;
    for (const file of fs.readdirSync(promptsDir).sort()) {
      if (!file.endsWith(".md")) continue;
      try {
        this.promptTemplates.set(path.basename(file, ".md"), readVerifiedRegularUtf8(path.join(promptsDir, file)));
      } catch (err) {
        console.warn(`[BuiltinSkillsRegistry] skipping unsafe/unreadable prompt-templates/${file}:`, err instanceof Error ? err.message : String(err));
      }
    }
  }

  private discoverJsonAxis<T extends { id: string }>(dirName: string, target: Map<string, T>): void {
    const dir = path.join(this.baseDir, dirName);
    const dirStat = lstatSafe(dir);
    if (!dirStat || dirStat.isSymbolicLink() || !dirStat.isDirectory()) return;
    for (const file of fs.readdirSync(dir).sort()) {
      if (!file.endsWith(".json")) continue;
      try {
        const parsed = JSON.parse(readVerifiedRegularUtf8(path.join(dir, file))) as T;
        target.set(parsed.id, parsed);
      } catch (err) {
        console.warn(`[BuiltinSkillsRegistry] skipping malformed ${dirName}/${file}:`, err instanceof Error ? err.message : String(err));
      }
    }
  }

  private discoverDesignSystems(): void {
    const dsDir = path.join(this.baseDir, "design-systems");
    const dsStat = lstatSafe(dsDir);
    if (!dsStat || dsStat.isSymbolicLink() || !dsStat.isDirectory()) return;
    for (const ver of fs.readdirSync(dsDir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (!ver.isDirectory() || ver.isSymbolicLink()) continue;
      const verPath = path.join(dsDir, ver.name);
      const tokensFile = path.join(verPath, "dtcg-tokens.json");
      const mapFile = path.join(verPath, "mapping.json");
      if (!lstatSafe(tokensFile)) continue;
      try {
        const tokens = JSON.parse(readVerifiedRegularUtf8(tokensFile));
        const mapping = lstatSafe(mapFile) ? JSON.parse(readVerifiedRegularUtf8(mapFile)) : {};
        this.designSystems.set(ver.name, { version: "v1", name: `Cowok Design System ${ver.name}`, tokens, mapping });
      } catch (err) {
        console.warn(`[BuiltinSkillsRegistry] skipping malformed design system ${ver.name}:`, err instanceof Error ? err.message : String(err));
      }
    }
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
