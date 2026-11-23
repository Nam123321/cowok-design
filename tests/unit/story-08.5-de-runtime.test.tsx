// @vitest-environment jsdom
// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as deRuntimeApi from "../../src/api/de-runtime";
import { DeToolsStatusList } from "../../src/features/plugin-center/DeToolsStatusList";
import { EphemeralToolAuditDrawerPanel } from "../../src/features/plugin-center/EphemeralToolAuditDrawerPanel";
import { LearnLedgerQueueList } from "../../src/features/plugin-center/LearnLedgerQueueList";
import { RepoImportForm } from "../../src/features/plugin-center/RepoImportForm";

const TEST_PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const SAMPLE_SHA = "a".repeat(64);
const SAMPLE_TIMESTAMP = 1_730_500_000_000;
const HTTP_FORBIDDEN = 403;
const TOOL_COUNT = 7;

describe("Story 08.5 Split DE+DR Plugin Runtime, Tool Jail & LEARN Ledger UI", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("UT-08.5-UI-01: renders DeToolsStatusList with data-ff-id and executes all 7 DE tools at $0", async () => {
    vi.spyOn(deRuntimeApi, "runBuiltInDe").mockResolvedValue({
      ok: true,
      requestId: "req-de-1",
      data: {
        targetArtifactPath: "screens/home.tsx",
        passed: true,
        costUsd: 0,
        totalDurationMs: 4,
        withinSla: true,
        results: [
          { tool: "scan-ui", passed: true, durationMs: 1, durationMicros: 320, findings: [] },
          { tool: "intake", passed: true, durationMs: 0, durationMicros: 110, findings: [] },
          { tool: "tokens", passed: true, durationMs: 0, durationMicros: 180, findings: [] },
          { tool: "nfa-packs", passed: true, durationMs: 1, durationMicros: 450, findings: [] },
          { tool: "archify", passed: true, durationMs: 1, durationMicros: 210, findings: [] },
          { tool: "get_artifact", passed: true, durationMs: 0, durationMicros: 95, findings: [] },
          { tool: "notes-check", passed: true, durationMs: 0, durationMicros: 80, findings: [] },
        ],
      },
    });

    render(<DeToolsStatusList projectId={TEST_PROJECT_ID} />);
    expect(document.querySelector("[data-ff-id='de-tools-status-list']")).not.toBeNull();
    expect(screen.getByTestId("de-sla-badge").textContent).toMatch(/0[.,]00|\$0/);

    fireEvent.click(screen.getByTestId("btn-run-de-suite"));
    await waitFor(() => {
      expect(screen.getByTestId("de-suite-summary").textContent).toMatch(/Tất cả đạt|All checks passed/i);
    });
    expect(deRuntimeApi.runBuiltInDe).toHaveBeenCalledTimes(1);
    const callArg = vi.mocked(deRuntimeApi.runBuiltInDe).mock.calls[0][0];
    expect(callArg.tools).toHaveLength(TOOL_COUNT);
  });

  it("UT-08.5-UI-02: renders RepoImportForm with <=100 tok badge and packages MIT skill", async () => {
    vi.spyOn(deRuntimeApi, "listImportedUserSkills").mockResolvedValue({
      ok: true,
      requestId: "req-list-1",
      data: { items: [] },
    });
    vi.spyOn(deRuntimeApi, "importUserSkillRepo").mockResolvedValue({
      ok: true,
      requestId: "req-imp-1",
      data: {
        id: "usr-1",
        tenantId: "local-default",
        projectId: TEST_PROJECT_ID,
        skillSlug: "oklch-brand-guard",
        repoSourceUrl: "https://github.com/cowok-ai/oklch-brand-guard",
        headerTokens: 32,
        securityScanVerdict: "PASS",
        licenseClassification: "ADOPT_PRIOR_ART",
        skillMdPath: ".cowok-design/user-skills/oklch-brand-guard/SKILL.md",
        skillMdPreview: "---\nname: oklch-brand-guard\n---",
        createdAt: SAMPLE_TIMESTAMP,
      },
    });

    render(<RepoImportForm projectId={TEST_PROJECT_ID} />);
    expect(document.querySelector("[data-ff-id='plugin-repo-import-form']")).not.toBeNull();
    expect(screen.getByTestId("jit-header-token-badge").textContent).toMatch(/\/100\s+(tok|token)/i);

    fireEvent.click(screen.getByTestId("btn-submit-import-repo"));
    await waitFor(() => {
      expect(screen.getByTestId("repo-import-receipt").textContent).toContain("PASS");
    });
    expect(screen.getByTestId("user-skill-row-oklch-brand-guard")).toBeTruthy();
  });

  it("UT-08.5-UI-03: renders EphemeralToolAuditDrawerPanel, surfaces HUMAN_ONLY error & receipt", async () => {
    const execSpy = vi
      .spyOn(deRuntimeApi, "executeEphemeralTool")
      .mockRejectedValueOnce(
        new deRuntimeApi.DeRuntimeApiError(
          HTTP_FORBIDDEN,
          "HUMAN_APPROVAL_REQUIRED",
          "HumanWorkbenchSession required"
        )
      )
      .mockResolvedValueOnce({
        ok: true,
        requestId: "req-jail-2",
        data: {
          executionId: "exec-1",
          toolName: "token-line-counter",
          runtime: "sh",
          actorKind: "HUMAN_WORKBENCH_SESSION",
          scriptSha256: SAMPLE_SHA,
          exitCode: 0,
          timedOut: false,
          killedPgid: null,
          durationMs: 12,
          stdout: "JAIL_ISOLATION_OK\n",
          stderr: "",
          envCleared: true,
          networkIsolated: true,
          nofollowVerified: true,
          rlimitAsBytes: 268_435_456,
          rlimitNproc: 16,
        },
      });

    render(<EphemeralToolAuditDrawerPanel projectId={TEST_PROJECT_ID} />);
    expect(document.querySelector("[data-ff-id='ephemeral-tool-audit-drawer']")).not.toBeNull();
    expect(screen.getByTestId("jail-human-only-badge").textContent).toContain("HUMAN_ONLY");

    fireEvent.click(screen.getByTestId("btn-execute-ephemeral-tool"));
    await waitFor(() => {
      expect(screen.getByTestId("ephemeral-tool-error").textContent).toContain(
        "HUMAN_APPROVAL_REQUIRED"
      );
    });

    fireEvent.click(screen.getByTestId("btn-execute-ephemeral-tool"));
    await waitFor(() => {
      expect(screen.getByTestId("ephemeral-tool-stdout").textContent).toContain(
        "JAIL_ISOLATION_OK"
      );
    });
    expect(execSpy).toHaveBeenCalledTimes(2);
  });

  it("UT-08.5-UI-04: renders LearnLedgerQueueList with hysteresis metadata and PROPOSED cards", async () => {
    vi.spyOn(deRuntimeApi, "listLearnLedger").mockResolvedValue({
      ok: true,
      requestId: "req-ledger-1",
      data: {
        projectId: TEST_PROJECT_ID,
        minHitCount: 3,
        promoteSlprThreshold: 0.95,
        retireSlprThreshold: 0.85,
        entries: [
          {
            id: "gfl-1",
            tenantId: "local-default",
            projectId: TEST_PROJECT_ID,
            ruleId: "TELLS-001",
            fingerprintSha256: SAMPLE_SHA,
            sampleMessage: "Raw indigo gradient detected",
            hitCount: 3,
            slprScore: 0.96,
            promotedCardId: "learn-aaaaaaaaaaaa",
            status: "PROPOSED",
            lastSeenAt: SAMPLE_TIMESTAMP,
            qualifiesForProposal: true,
          },
        ],
      },
    });

    render(<LearnLedgerQueueList projectId={TEST_PROJECT_ID} />);
    expect(document.querySelector("[data-ff-id='learn-ledger-queue']")).not.toBeNull();
    await waitFor(() => {
      expect(screen.getByTestId("ledger-status-gfl-1").textContent).toBe("PROPOSED");
    });
    expect(screen.getByTestId("ledger-promoted-card-gfl-1").textContent).toContain(
      "learn-aaaaaaaaaaaa"
    );
  });
});
