// @vitest-environment jsdom
// @story 05.1
// @implements AC-01, AC-02, AC-03, AC-09 (SDI Engine, Gap-Driven Discovery & Zero-Tailwind Verification)
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import {
  computeAmbiguityAndRisk,
  generateGapDiscoveryQuestions,
  generateClarificationVariants,
} from "../utils/ambiguity-calculator";
import { ClarificationCard } from "../components/ClarificationCard";
import { SDIInspectorTab, IntentStatusCard } from "../components/SDIInspectorTab";
import { SDIPromptInput } from "../components/SDIPromptInput";
import { VisualClarificationModal } from "../components/VisualClarificationModal";
import type { DesignIntentResponse } from "../../../lib/validations/sdi";

describe("Story 05.1: SDI Engine Upgrade & Zero-Tailwind Purge", () => {
  describe("1. Ambiguity Scoring & Gap-Driven Discovery Engine", () => {
    it("routes to direct_edit mode when specific node and explicit terms are provided", () => {
      const result = computeAmbiguityAndRisk("Đổi màu button primary sang blue", "node-scr-01", {
        hasActiveSelection: true,
      });

      expect(result.ambiguityScore).toBeLessThan(0.45);
      expect(result.isAmbiguous).toBe(false);
      expect(result.routingMode).toBe("direct_edit");
    });

    it("routes to full_plan mode when broad or vague prompt is provided without specific node", () => {
      const result = computeAmbiguityAndRisk("Làm đẹp lại giao diện này");

      expect(result.ambiguityScore).toBeGreaterThanOrEqual(0.45);
      expect(result.isAmbiguous).toBe(true);
      expect(result.routingMode).toBe("full_plan");
    });

    it("generates 1 to 3 visual discovery gap questions with structured options", () => {
      const questions = generateGapDiscoveryQuestions("Tối ưu hóa luồng thanh toán");

      expect(questions.length).toBeGreaterThanOrEqual(1);
      expect(questions.length).toBeLessThanOrEqual(3);
      for (const q of questions) {
        expect(q.id).toBeTruthy();
        expect(q.question).toBeTruthy();
        expect(q.options.length).toBeGreaterThanOrEqual(2);
      }
    });

    it("preserves backward compatibility with 3 standard clarification variants", () => {
      const variants = generateClarificationVariants("Cải tiến giỏ hàng");
      expect(variants).toHaveLength(3);
      expect(variants[0].id).toBe("var-1-minimal");
      expect(variants[1].id).toBe("var-2-balanced");
      expect(variants[2].id).toBe("var-3-modular");
    });
  });

  describe("2. Zero-Tailwind Purge Across All SDI Components", () => {
    const sdiComponentDir = path.resolve(__dirname, "../components");

    it("verifies 0 Tailwind classes, 0 *ClassName props, and 0 clsx/cn in SDI component files", () => {
      const componentFiles = [
        "ClarificationCard.tsx",
        "SDIInspectorTab.tsx",
        "SDIPromptInput.tsx",
        "VisualClarificationModal.tsx",
      ];

      const tailwindRegex = /\b(?:bg-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d+|text-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d+|p-\d+|m-\d+|flex-col|rounded-lg|rounded-xl|border-border)\b/;

      for (const file of componentFiles) {
        const filePath = path.join(sdiComponentDir, file);
        const content = fs.readFileSync(filePath, "utf8");

        // No className attribute
        expect(content).not.toMatch(/className=/);
        // No clsx / cn import
        expect(content).not.toContain("clsx");
        expect(content).not.toContain("cn(");
        // No tailwind utilities
        expect(content).not.toMatch(tailwindRegex);
      }
    });

    it("renders ClarificationCard cleanly with semantic inline styling and handles clicks", () => {
      const mockVariant = {
        id: "var-test",
        title: "Balanced Architecture",
        description: "Align with DESIGN.md tokens",
        pros: ["100% token sync", "Component reusability"],
        cons: ["Requires viewport audit"],
      };

      const handleSelect = vi.fn();
      render(<ClarificationCard variant={mockVariant} isSelected={true} onSelect={handleSelect} />);

      const card = screen.getByTestId("clarification-card-var-test");
      expect(card).toBeTruthy();
      expect(card.textContent).toContain("Balanced Architecture");
      expect(card.textContent).toContain("100% token sync");

      fireEvent.click(card);
      expect(handleSelect).toHaveBeenCalledWith("var-test");
    });

    it("renders IntentStatusCard with ambiguity badge and triggers modal on ambiguous state", () => {
      const mockIntent: DesignIntentResponse = {
        id: "intent-123",
        tenantId: "tenant-1",
        projectId: "proj-1",
        rawPrompt: "Redesign checkout",
        intentType: "update_flow",
        targetScope: { nodeIds: [], screenKeys: [], components: [] },
        ambiguityScore: 0.65,
        riskScore: 0.4,
        status: "clarifying",
        createdAt: "2026-11-16T00:00:00Z",
        updatedAt: "2026-11-16T00:00:00Z",
      };

      const handleOpenModal = vi.fn();
      render(<IntentStatusCard intent={mockIntent} onOpenModal={handleOpenModal} />);

      const badge = screen.getByTestId("intent-ambiguity-badge");
      expect(badge.textContent).toMatch(/0[.,]65/);
      expect(badge.textContent).toContain("clarifying");

      const btn = screen.getByTestId("open-clarifications-btn");
      expect(btn).toBeTruthy();
      fireEvent.click(btn);
      expect(handleOpenModal).toHaveBeenCalledTimes(1);
    });

    it("renders SDIPromptInput without Tailwind classes and handles input change", () => {
      render(
        <SDIPromptInput
          projectId="proj-1"
          onIntentCompiled={vi.fn()}
        />
      );

      const textarea = screen.getByPlaceholderText("Mô tả thay đổi bằng ngôn ngữ tự nhiên...");
      expect(textarea).toBeTruthy();
      fireEvent.change(textarea, { target: { value: "Add biometric checkout" } });
      expect((textarea as HTMLTextAreaElement).value).toBe("Add biometric checkout");
    });

    it("renders VisualClarificationModal with zero Tailwind and handles variant selection", () => {
      const mockVariants = [
        {
          id: "var-1-minimal",
          title: "Minimal Plan",
          description: "Local fix",
          pros: ["Fast"],
          cons: ["Not scalable"],
        },
      ];

      const handleConfirm = vi.fn();
      const handleClose = vi.fn();

      render(
        <VisualClarificationModal
          isOpen={true}
          variants={mockVariants}
          onConfirm={handleConfirm}
          onClose={handleClose}
        />
      );

      expect(screen.getByTestId("visual-clarification-modal")).toBeTruthy();
      const card = screen.getByTestId("clarification-card-var-1-minimal");
      fireEvent.click(card);

      const confirmBtn = screen.getByText("Xác Nhận & Tiếp Tục");
      fireEvent.click(confirmBtn);
      expect(handleConfirm).toHaveBeenCalledWith("var-1-minimal");
    });
  });
});
