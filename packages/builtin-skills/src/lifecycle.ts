export type PluginState =
  | "Discovered"
  | "Validated"
  | "Installed"
  | "Active"
  | "Deprecated"
  | "Archived";

export interface ManagedPluginRecord {
  id: string;
  name: string;
  version: string;
  state: PluginState;
  breaking_change_isolated: boolean;
  staging_version?: string;
  updated_at: number;
}

export class PluginStateMachine {
  private plugin: ManagedPluginRecord;

  constructor(id: string, name: string, version: string) {
    this.plugin = {
      id,
      name,
      version,
      state: "Discovered",
      breaking_change_isolated: false,
      updated_at: Date.now(),
    };
  }

  public get snapshot(): ManagedPluginRecord {
    return { ...this.plugin };
  }

  public transitionTo(next: PluginState): void {
    const valid = this.isValidTransition(this.plugin.state, next);
    if (!valid) {
      throw new Error(`Invalid state transition from ${this.plugin.state} to ${next}`);
    }
    this.plugin.state = next;
    this.plugin.updated_at = Date.now();
  }

  private isValidTransition(from: PluginState, to: PluginState): boolean {
    switch (from) {
      case "Discovered":
        return to === "Validated" || to === "Archived";
      case "Validated":
        return to === "Installed";
      case "Installed":
        return to === "Active";
      case "Active":
        return to === "Deprecated" || to === "Archived";
      case "Deprecated":
        return to === "Active" || to === "Archived";
      case "Archived":
        return false;
      default:
        return false;
    }
  }

  public applyUpdate(newVersion: string, isBreaking: boolean): void {
    if (isBreaking && this.plugin.state === "Active") {
      this.plugin.breaking_change_isolated = true;
      this.plugin.staging_version = newVersion;
      this.plugin.updated_at = Date.now();
      throw new Error(
        `Breaking change detected: active version ${this.plugin.version} kept active; candidate ${newVersion} isolated in staging`
      );
    }

    this.plugin.version = newVersion;
    this.plugin.breaking_change_isolated = false;
    this.plugin.staging_version = undefined;
    this.plugin.updated_at = Date.now();
  }
}

export type LearnCardCategory = "Rule" | "Pattern" | "AntiPattern" | "Lesson" | "Flow";
export type CardStatus = "Proposed" | "Accepted" | "Rejected";

export interface KnowledgeCard {
  id: string;
  category: LearnCardCategory;
  title: string;
  content: string;
  status: CardStatus;
  source_evidence: string;
  created_at: number;
  accepted_at?: number;
}

export class LearnPipeline {
  private cards: Map<string, KnowledgeCard> = new Map();

  public proposeCard(
    id: string,
    category: LearnCardCategory,
    title: string,
    content: string,
    sourceEvidence: string
  ): KnowledgeCard {
    const card: KnowledgeCard = {
      id,
      category,
      title,
      content,
      status: "Proposed",
      source_evidence: sourceEvidence,
      created_at: Date.now(),
    };
    this.cards.set(id, card);
    return card;
  }

  public acceptCard(id: string): boolean {
    const card = this.cards.get(id);
    if (card) {
      card.status = "Accepted";
      card.accepted_at = Date.now();
      return true;
    }
    return false;
  }

  public rejectCard(id: string): boolean {
    const card = this.cards.get(id);
    if (card) {
      card.status = "Rejected";
      return true;
    }
    return false;
  }

  public listAllCards(): KnowledgeCard[] {
    return Array.from(this.cards.values());
  }

  /**
   * EC-LEARN-001 (Zero Unapproved Poisoning):
   * Strictly filters and returns ONLY cards where status === "Accepted".
   * Proposed and Rejected cards are NEVER injected into system prompts without explicit human approval.
   */
  public getActivePromptInjections(): KnowledgeCard[] {
    return Array.from(this.cards.values()).filter((c) => c.status === "Accepted");
  }
}
