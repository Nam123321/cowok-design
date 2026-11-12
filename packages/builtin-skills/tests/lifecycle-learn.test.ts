import { describe, it, expect } from "vitest";
import { PluginStateMachine, LearnPipeline } from "../src/index";

describe("Story 08.3: 6-State Plugin Lifecycle & Human-Gated LEARN Loop", () => {
  describe("AC-01 & AC-02: 6-State Plugin Lifecycle", () => {
    it("transitions sequentially through all 6 states", () => {
      const sm = new PluginStateMachine("p-1", "sdi-engine", "1.0.0");
      expect(sm.snapshot.state).toBe("Discovered");

      sm.transitionTo("Validated");
      expect(sm.snapshot.state).toBe("Validated");

      sm.transitionTo("Installed");
      expect(sm.snapshot.state).toBe("Installed");

      sm.transitionTo("Active");
      expect(sm.snapshot.state).toBe("Active");

      sm.transitionTo("Deprecated");
      expect(sm.snapshot.state).toBe("Deprecated");

      sm.transitionTo("Archived");
      expect(sm.snapshot.state).toBe("Archived");
    });

    it("rejects invalid state transitions", () => {
      const sm = new PluginStateMachine("p-2", "taste-guard", "1.0.0");
      expect(() => sm.transitionTo("Active")).toThrow("Invalid state transition");
    });
  });

  describe("AC-03 & Edge Case Guard: Breaking Change Isolation into Staging", () => {
    it("keeps active version intact and isolates breaking update in staging", () => {
      const sm = new PluginStateMachine("p-3", "spec-syn", "1.0.0");
      sm.transitionTo("Validated");
      sm.transitionTo("Installed");
      sm.transitionTo("Active");

      // Non-breaking update succeeds normally
      sm.applyUpdate("1.1.0", false);
      expect(sm.snapshot.version).toBe("1.1.0");
      expect(sm.snapshot.breaking_change_isolated).toBe(false);

      // Breaking update throws and isolates candidate in staging
      expect(() => sm.applyUpdate("2.0.0", true)).toThrow("Breaking change detected");
      expect(sm.snapshot.version).toBe("1.1.0"); // Active version kept!
      expect(sm.snapshot.breaking_change_isolated).toBe(true);
      expect(sm.snapshot.staging_version).toBe("2.0.0");
    });
  });

  describe("AC-04 & EC-LEARN-001: Zero Unapproved Poisoning Guard in LEARN Loop", () => {
    it("proposes cards and guarantees proposed cards are never injected before human acceptance", () => {
      const learn = new LearnPipeline();

      // Ingest gate failures into proposed cards
      learn.proposeCard(
        "card-contrast",
        "Rule",
        "Contrast Minimum 4.5:1",
        "Darken foreground text against light surfaces.",
        "gate-history.jsonl:104"
      );

      // Verify status is proposed
      const allCards = learn.listAllCards();
      expect(allCards.length).toBe(1);
      expect(allCards[0].status).toBe("Proposed");

      // EC-LEARN-001: Zero unapproved poisoning!
      expect(learn.getActivePromptInjections()).toHaveLength(0);

      // Human safety valve: accept card
      expect(learn.acceptCard("card-contrast")).toBe(true);
      expect(allCards[0].status).toBe("Accepted");

      // Now it is active for prompt injection
      const active = learn.getActivePromptInjections();
      expect(active).toHaveLength(1);
      expect(active[0].id).toBe("card-contrast");
    });

    it("ensures rejected cards are strictly excluded from prompt injection", () => {
      const learn = new LearnPipeline();
      learn.proposeCard(
        "card-bad",
        "AntiPattern",
        "Hallucinated Rule",
        "Unverified content",
        "user-inbox/note.txt"
      );

      learn.rejectCard("card-bad");
      expect(learn.getActivePromptInjections()).toHaveLength(0);
    });
  });
});
