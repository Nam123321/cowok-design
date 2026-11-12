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
import { validateSkillFrontmatter } from "./validator";

export class BuiltinSkillsRegistry {
  private baseDir: string;
  private skills: Map<string, SkillManifest> = new Map();
  private craftRules: Map<string, CraftRule> = new Map();
  private designTemplates: Map<string, DesignTemplate> = new Map();
  private designSystems: Map<string, DesignSystemV1> = new Map();
  private scenarios: Map<string, ScenarioDef> = new Map();
  private atoms: Map<string, AtomDef> = new Map();
  private promptTemplates: Map<string, string> = new Map();

  constructor(baseDir: string) {
    this.baseDir = baseDir;
  }

  /**
   * Discovers and indexes all multi-axis capabilities from the directory structure
   */
  public discover(): void {
    if (!fs.existsSync(this.baseDir)) return;

    // 1. Discover Skills
    const skillsDir = path.join(this.baseDir, "skills");
    if (fs.existsSync(skillsDir)) {
      const entries = fs.readdirSync(skillsDir, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isDirectory()) {
          const folder = path.join(skillsDir, entry.name);
          const manifest = this.loadSkillFolder(folder);
          if (manifest) {
            this.skills.set(manifest.name, manifest);
          }
        }
      }
    }

    // 2. Discover Craft Rules
    const craftDir = path.join(this.baseDir, "craft");
    if (fs.existsSync(craftDir)) {
      const files = fs.readdirSync(craftDir);
      for (const file of files) {
        if (file.endsWith(".md")) {
          const id = path.basename(file, ".md");
          const content = fs.readFileSync(path.join(craftDir, file), "utf8");
          const rule: CraftRule = {
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
          };
          this.craftRules.set(id, rule);
        }
      }
    }

    // 3. Discover Design Templates
    const templatesDir = path.join(this.baseDir, "design-templates");
    if (fs.existsSync(templatesDir)) {
      const files = fs.readdirSync(templatesDir);
      for (const file of files) {
        if (file.endsWith(".json")) {
          try {
            const raw = fs.readFileSync(path.join(templatesDir, file), "utf8");
            const parsed = JSON.parse(raw) as DesignTemplate;
            this.designTemplates.set(parsed.id, parsed);
          } catch {
            // Ignore parse errors on templates
          }
        }
      }
    }

    // 4. Discover Design Systems
    const dsDir = path.join(this.baseDir, "design-systems");
    if (fs.existsSync(dsDir)) {
      const versions = fs.readdirSync(dsDir, { withFileTypes: true });
      for (const ver of versions) {
        if (ver.isDirectory()) {
          const verPath = path.join(dsDir, ver.name);
          const tokensFile = path.join(verPath, "dtcg-tokens.json");
          const mapFile = path.join(verPath, "mapping.json");
          if (fs.existsSync(tokensFile)) {
            try {
              const tokens = JSON.parse(fs.readFileSync(tokensFile, "utf8"));
              const mapping = fs.existsSync(mapFile) ? JSON.parse(fs.readFileSync(mapFile, "utf8")) : {};
              this.designSystems.set(ver.name, {
                version: "v1",
                name: `Cowok Design System ${ver.name}`,
                tokens,
                mapping,
              });
            } catch {
              // Ignore
            }
          }
        }
      }
    }

    // 5. Discover Scenarios
    const scenariosDir = path.join(this.baseDir, "scenarios");
    if (fs.existsSync(scenariosDir)) {
      const files = fs.readdirSync(scenariosDir);
      for (const file of files) {
        if (file.endsWith(".json")) {
          try {
            const raw = fs.readFileSync(path.join(scenariosDir, file), "utf8");
            const parsed = JSON.parse(raw) as ScenarioDef;
            this.scenarios.set(parsed.id, parsed);
          } catch {
            // Ignore
          }
        }
      }
    }

    // 6. Discover Atoms
    const atomsDir = path.join(this.baseDir, "atoms");
    if (fs.existsSync(atomsDir)) {
      const files = fs.readdirSync(atomsDir);
      for (const file of files) {
        if (file.endsWith(".json")) {
          try {
            const raw = fs.readFileSync(path.join(atomsDir, file), "utf8");
            const parsed = JSON.parse(raw) as AtomDef;
            this.atoms.set(parsed.id, parsed);
          } catch {
            // Ignore
          }
        }
      }
    }

    // 7. Discover Prompt Templates
    const promptsDir = path.join(this.baseDir, "prompt-templates");
    if (fs.existsSync(promptsDir)) {
      const files = fs.readdirSync(promptsDir);
      for (const file of files) {
        if (file.endsWith(".md")) {
          const id = path.basename(file, ".md");
          const content = fs.readFileSync(path.join(promptsDir, file), "utf8");
          this.promptTemplates.set(id, content);
        }
      }
    }
  }

  private loadSkillFolder(folder: string): SkillManifest | null {
    const pluginJsonPath = path.join(folder, "plugin.json");
    const skillMdPath = path.join(folder, "SKILL.md");

    let manifest: Partial<SkillManifest> = {};

    if (fs.existsSync(skillMdPath)) {
      const raw = fs.readFileSync(skillMdPath, "utf8");
      const validation = validateSkillFrontmatter(raw);
      if (!validation.valid) {
        return null;
      }
      manifest = {
        name: validation.fields?.name,
        version: validation.fields?.version || "1.0.0",
        description: validation.fields?.description,
        risk_tier: validation.fields?.risk_tier || 1,
      };
    }

    if (fs.existsSync(pluginJsonPath)) {
      try {
        const rawJson = fs.readFileSync(pluginJsonPath, "utf8");
        const jsonManifest = JSON.parse(rawJson);
        manifest = { ...manifest, ...jsonManifest };
      } catch {
        // Fallback to SKILL.md fields
      }
    }

    if (manifest.name && manifest.description) {
      return {
        name: manifest.name,
        version: manifest.version || "1.0.0",
        description: manifest.description,
        entry_point: manifest.entry_point || "SKILL.md",
        capabilities_required: manifest.capabilities_required || [],
        risk_tier: (manifest.risk_tier || 1) as 1 | 2 | 3,
        parameters: manifest.parameters,
        tags: manifest.tags || [],
        prompt_overview: manifest.prompt_overview,
      };
    }

    return null;
  }

  // Getters
  public getSkill(name: string): SkillManifest | undefined {
    return this.skills.get(name);
  }

  public listSkills(): SkillManifest[] {
    return Array.from(this.skills.values());
  }

  public getCraftRule(id: string): CraftRule | undefined {
    return this.craftRules.get(id);
  }

  public listCraftRules(): CraftRule[] {
    return Array.from(this.craftRules.values());
  }

  public getDesignTemplate(id: string): DesignTemplate | undefined {
    return this.designTemplates.get(id);
  }

  public listDesignTemplates(): DesignTemplate[] {
    return Array.from(this.designTemplates.values());
  }

  public getDesignSystem(version: string): DesignSystemV1 | undefined {
    return this.designSystems.get(version);
  }

  public getScenario(id: string): ScenarioDef | undefined {
    return this.scenarios.get(id);
  }

  public listScenarios(): ScenarioDef[] {
    return Array.from(this.scenarios.values());
  }

  public getAtom(id: string): AtomDef | undefined {
    return this.atoms.get(id);
  }

  public listAtoms(): AtomDef[] {
    return Array.from(this.atoms.values());
  }

  public getPromptTemplate(id: string): string | undefined {
    return this.promptTemplates.get(id);
  }
}
