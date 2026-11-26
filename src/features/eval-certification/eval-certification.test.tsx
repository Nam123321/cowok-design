// @vitest-environment jsdom
// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Unit, Component & Static Hygiene Suite)
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  EvalCertificationApiClient,
  EvalCertificationApiError,
} from "../../api/eval-certification";
import { setGlobalLocale } from "../../hooks/useTranslation";
import { matchBffRoute } from "../../lib/vite-bff-routes";
import {
  type EvalBenchmarkRunDto,
  type EvalHistoryResponse,
  EvalHistoryResponseSchema,
  type EvalRunResponse,
  EvalRunResponseSchema,
} from "../../lib/validations/eval-certification";
import { ContextualStageToolbar } from "../../pages/workbench/components/ContextualStageToolbar";
import { CertificationDrawer } from "./CertificationDrawer";

const REPO_ROOT = process.cwd();

function buildSimCases(overrideCaseId?: string): EvalBenchmarkRunDto["simCases"] {
  const milestones: Array<"M1" | "M2" | "M3" | "M4" | "M5"> = ["M1", "M2", "M3", "M4", "M5"];
  const securityMap: Record<string, string> = {
    "UC-03": "M1_SECRET_INTERCEPT",
    "UC-09": "M2_CSP_XSS_ZERO_SPLIT_BRAIN",
    "UC-14": "M3_RCE_TOOL_JAIL",
    "UC-20": "M4_ZERO_FLOAT_LEAK",
    "UC-23": "M5_ZERO_OFFLINE_404",
    "UC-25": "M5_SEALED_TAMPER_GUARD",
  };
  return Array.from({ length: 25 }, (_, idx) => {
    const num = String(idx + 1).padStart(2, "0");
    const caseId = `UC-${num}`;
    const milestone = milestones[Math.floor(idx / 5)]!;
    const securityGateKey = securityMap[caseId] ?? null;
    const isFailed = overrideCaseId === caseId;
    return {
      caseId,
      milestone,
      titleKey: `evalCertification.caseTitle_${caseId}`,
      mandatorySecurityGate: Boolean(securityGateKey),
      securityGateKey,
      status: isFailed ? "FAIL" : "PASS",
      durationMs: 120 + idx * 15,
      evidenceSummary: isFailed
        ? "Mandatory security assertion failed during verification."
        : `Verified deterministic assertion for ${caseId}.`,
    };
  });
}

function buildMockRun(overrides: Partial<EvalBenchmarkRunDto> = {}): EvalBenchmarkRunDto {
  const simCases = overrides.simCases ?? buildSimCases();
  return {
    runId: "run-eval-001",
    projectId: "proj-1",
    tier: "pr",
    offlineEnforced: true,
    sealedManifestSha256: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
    manifestPinSource: "compiled_binary",
    tamperDetected: false,
    tamperReason: null,
    verdict: "PASS",
    durationMs: 1450,
    estimatedCostUsd: 0,
    simCasesPassed: 25,
    simCasesBlocked: 0,
    simCasesGap: 0,
    simCasesTotal: 25,
    securityGatesPassed: true,
    securityBlockers: [],
    milestones: [
      { milestone: "M1", passed: 5, total: 5, blocked: 0, gap: 0, securityGatePassed: true, status: "PASS" },
      { milestone: "M2", passed: 5, total: 5, blocked: 0, gap: 0, securityGatePassed: true, status: "PASS" },
      { milestone: "M3", passed: 5, total: 5, blocked: 0, gap: 0, securityGatePassed: true, status: "PASS" },
      { milestone: "M4", passed: 5, total: 5, blocked: 0, gap: 0, securityGatePassed: true, status: "PASS" },
      { milestone: "M5", passed: 5, total: 5, blocked: 0, gap: 0, securityGatePassed: true, status: "PASS" },
    ],
    simCases,
    ablation: {
      briefsEvaluated: 14,
      dataStorytellingBriefsCount: 4,
      baselineMeanScore: 71.2,
      withSkillMeanScore: 89.6,
      liftPercent: 18.4,
      regressionsCount: 0,
      designCraftSemverPassed: true,
      designCraftVersion: "v1.4.0",
    },
    calibration: {
      providers: [
        { providerId: "local_fts5", casesEvaluated: 20, accuracyPercent: 95.0, floatLeaksCount: 0, passed: true },
        { providerId: "gemini_decision_jev", casesEvaluated: 20, accuracyPercent: 97.5, floatLeaksCount: 0, passed: true },
        { providerId: "deterministic_mcda", casesEvaluated: 20, accuracyPercent: 100.0, floatLeaksCount: 0, passed: true },
      ],
      totalFloatLeaks: 0,
      passed: true,
    },
    faultInjection: {
      injectedCount: 22,
      caughtCount: 21,
      catchRatePercent: 95.5,
      builderScorerIsolated: true,
      passed: true,
    },
    ledgerPath: ".cowok-design/evals/gate-history.jsonl",
    triggeredBy: "workbench",
    createdAt: 1764050000000,
    ...overrides,
  };
}

function buildMockHistory(run: EvalBenchmarkRunDto | null = buildMockRun()): EvalHistoryResponse {
  return {
    ok: true,
    requestId: "req-hist-1",
    data: {
      sealedIntegrityOk: run ? !run.tamperDetected : true,
      sealedManifestSha256:
        run?.sealedManifestSha256 ?? "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
      manifestPinSource: "compiled_binary",
      tamperReason: run?.tamperReason ?? null,
      latestRun: run,
      runs: run ? [run] : [],
      totalRuns: run ? 1 : 0,
    },
  };
}

afterEach(() => {
  cleanup();
  setGlobalLocale("vi-VN");
});

describe("Story 13.1 — Release Certification Zod Contracts, SDK & Drawer UI", () => {
  it("parses valid EvalRunResponse & EvalHistoryResponse and rejects malformed payloads", () => {
    const validRunResp: EvalRunResponse = {
      ok: true,
      requestId: "req-run-1",
      data: buildMockRun(),
    };
    const parsedRun = EvalRunResponseSchema.parse(validRunResp);
    expect(parsedRun.data.simCases).toHaveLength(25);
    expect(parsedRun.data.ablation.briefsEvaluated).toBeGreaterThanOrEqual(12);

    const parsedHist = EvalHistoryResponseSchema.parse(buildMockHistory());
    expect(parsedHist.data.sealedIntegrityOk).toBe(true);

    expect(() =>
      EvalRunResponseSchema.parse({
        ok: true,
        requestId: "bad",
        data: { ...buildMockRun(), simCases: [{ ...buildSimCases()[0]!, caseId: "UC-99" }] },
      })
    ).toThrow();
  });

  it("registers Story 13.1 /api/eval/run and /api/eval/history routes in Vite BFF router", () => {
    expect(matchBffRoute("POST", "/api/eval/run")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/eval/run")).not.toBeNull();
    expect(matchBffRoute("GET", "/api/eval/history")).not.toBeNull();
    expect(matchBffRoute("GET", "/api/v1/eval/history")).not.toBeNull();
  });

  it("handles 409 conflict, 422 tamper, and flat auth error payloads (401/403/404) via EvalCertificationApiClient", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            error: { code: "EVAL_ALREADY_RUNNING", message: "Another evaluation is active" },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            error: {
              code: "SEALED_EVAL_TAMPER_ERROR",
              message: "WRITABLE_SEALED_FILE",
              details: { tamperReason: "WRITABLE_SEALED_FILE" },
            },
          }),
          { status: 422, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ code: "AUTH_FORBIDDEN", message: "Insufficient role for eval" }),
          { status: 403, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ code: "PROJECT_NOT_FOUND", message: "Project not found" }),
          { status: 404, headers: { "Content-Type": "application/json" } }
        )
      );

    const client = new EvalCertificationApiClient({
      getSessionToken: () => "tok-test",
      fetchImpl: fetchMock,
    });

    await expect(client.runEval({ tier: "pr" })).rejects.toMatchObject({
      name: "EvalCertificationApiError",
      status: 409,
      code: "EVAL_ALREADY_RUNNING",
      isAlreadyRunning: true,
    });

    await expect(client.runEval({ tier: "stage" })).rejects.toMatchObject({
      name: "EvalCertificationApiError",
      status: 422,
      code: "SEALED_EVAL_TAMPER_ERROR",
      isTamperError: true,
    });

    await expect(client.getEvalHistory()).rejects.toMatchObject({
      name: "EvalCertificationApiError",
      status: 403,
      code: "AUTH_FORBIDDEN",
      message: "Insufficient role for eval",
    });

    await expect(client.getEvalHistory()).rejects.toMatchObject({
      name: "EvalCertificationApiError",
      status: 404,
      code: "PROJECT_NOT_FOUND",
      message: "Project not found",
    });
  });

  it("loads history, runs tiers, filters M1..M5 & security cases, and never leaks raw enums or story IDs", async () => {
    setGlobalLocale("en-US");
    const prRun = buildMockRun({ runId: "run-pr-1", tier: "pr" });
    const stageRun = buildMockRun({
      runId: "run-stage-2",
      tier: "stage",
      offlineEnforced: false,
      ablation: { ...prRun.ablation, liftPercent: 21.5 },
    });

    const client = {
      getEvalHistory: vi.fn().mockResolvedValue(buildMockHistory(prRun)),
      runEval: vi.fn().mockResolvedValue({ ok: true as const, requestId: "req-2", data: stageRun }),
    };

    render(
      <CertificationDrawer isOpen onClose={vi.fn()} projectId="proj-1" locale="en-US" client={client} />
    );

    await waitFor(() => {
      expect(screen.getByTestId("eval-case-row-UC-25")).toBeTruthy();
    });

    // Switch tier to stage and run
    fireEvent.click(screen.getByTestId("eval-tier-stage"));
    fireEvent.click(screen.getByTestId("eval-run-trigger-btn"));

    await waitFor(() => {
      expect(client.runEval).toHaveBeenCalledWith({
        tier: "stage",
        projectId: "proj-1",
        runSimulatedUsers: true,
      });
    });

    // Filter by M3 milestone pill -> only UC-11..UC-15
    fireEvent.click(screen.getByTestId("eval-milestone-pill-M3"));
    expect(screen.getByTestId("eval-case-row-UC-14")).toBeTruthy();
    expect(screen.queryByTestId("eval-case-row-UC-01")).toBeNull();

    // Reset to ALL and filter security-only -> 6 mandatory security gates
    fireEvent.click(screen.getByTestId("eval-milestone-pill-ALL"));
    fireEvent.click(screen.getByTestId("eval-security-only-toggle"));
    expect(screen.getByTestId("eval-case-row-UC-03")).toBeTruthy();
    expect(screen.getByTestId("eval-case-row-UC-14")).toBeTruthy();
    expect(screen.queryByTestId("eval-case-row-UC-01")).toBeNull();

    // Switch to Ablation & Calibration tab and verify zero raw provider enums
    fireEvent.click(screen.getByTestId("eval-tab-ablation"));
    const ablationPanel = screen.getByTestId("eval-ablation-tab-panel");
    expect(ablationPanel.textContent).toContain("Local Context Search");
    expect(ablationPanel.textContent).not.toContain("local_fts5");
    expect(ablationPanel.textContent).not.toContain("gemini_decision_jev");

    // Switch to History tab
    fireEvent.click(screen.getByTestId("eval-tab-history"));
    expect(screen.getByTestId("eval-history-item-run-stage-2")).toBeTruthy();

    // Verify ZERO raw backend enum strings or internal IDs in drawer text
    const drawerText = screen.getByTestId("certification-drawer").textContent || "";
    expect(drawerText).not.toMatch(
      /SEALED_EVAL_TAMPER_ERROR|EVAL_ALREADY_RUNNING|Story 13\.1|V33|MPSC|compiled_binary|ci_env_override|chmod 444/
    );
  });

  it("renders non-masking security gate callout for Rust-formatted securityBlockers without generic fallback collapse", async () => {
    setGlobalLocale("en-US");
    const maskedFailRun = buildMockRun({
      verdict: "FAIL",
      simCasesPassed: 23,
      securityGatesPassed: false,
      securityBlockers: [
        "UC-03:M1_SECRET_INTERCEPT",
        "UC-20:M4_ZERO_FLOAT_LEAK",
        "UC-25:M5_SEALED_TAMPER_GUARD:SEALED_HASH_MISMATCH",
      ],
      simCases: buildSimCases("UC-03"),
    });
    const client = {
      getEvalHistory: vi.fn().mockResolvedValue(buildMockHistory(maskedFailRun)),
      runEval: vi.fn(),
    };

    render(<CertificationDrawer isOpen onClose={vi.fn()} locale="en-US" client={client} />);

    const banner = await screen.findByTestId("eval-security-nonmasking-banner");
    expect(banner.textContent).toContain("23/25");
    expect(banner.textContent).toContain("Block secret keys before storage or logging");
    expect(banner.textContent).toContain("Zero raw probability leaks from decision router");
    expect(banner.textContent).toContain("Protect sealed evaluation standards from tampering");
    expect(banner.textContent).not.toContain("Mandatory safety and integrity gate");
    expect(banner.textContent).not.toMatch(/M1_SECRET_INTERCEPT|M4_ZERO_FLOAT_LEAK|SEALED_HASH_MISMATCH/);
    expect(screen.getByTestId("eval-verdict-badge").textContent).toBe("Not Certified");
  });

  it("never leaks raw Rust internal jargon tokens (V33, MPSC, chmod 0444, localStorage, CDP) into rendered DOM", async () => {
    setGlobalLocale("en-US");
    const rawRustCases = buildSimCases().map((item) => {
      if (item.caseId === "UC-01") {
        return { ...item, evidenceSummary: "SQLite WAL schema version V33 verified via Axum IPC" };
      }
      if (item.caseId === "UC-09") {
        return { ...item, evidenceSummary: "Strict CSP/XSS fail-closed sanitizer & zero localStorage split-brain verified" };
      }
      if (item.caseId === "UC-11") {
        return { ...item, evidenceSummary: "Pooled CDP Visual QC & independent Rust scorer fault catch 100.0%" };
      }
      if (item.caseId === "UC-16") {
        return { ...item, evidenceSummary: "Single MPSC SQLite write actor serialization verified" };
      }
      if (item.caseId === "UC-25") {
        return { ...item, evidenceSummary: "Out-of-band pinned SHA-256 + chmod 0444 + zero-symlink sealed guard verified" };
      }
      return item;
    });
    const run = buildMockRun({ simCases: rawRustCases });
    const client = {
      getEvalHistory: vi.fn().mockResolvedValue(buildMockHistory(run)),
      runEval: vi.fn(),
    };

    render(<CertificationDrawer isOpen onClose={vi.fn()} locale="en-US" client={client} />);
    await screen.findByTestId("eval-case-row-UC-25");

    const tableText = screen.getByTestId("eval-sim-cases-table").textContent || "";
    expect(tableText).not.toMatch(/\bV33\b|\bMPSC\b|chmod 0444|localStorage|\bCDP\b|SQLite WAL|\bRust\b|\bAxum\b|\bIPC\b/);
  });

  it("matches localized case titles in search input across English, Vietnamese, and Japanese", async () => {
    const client = {
      getEvalHistory: vi.fn().mockResolvedValue(buildMockHistory()),
      runEval: vi.fn(),
    };

    setGlobalLocale("en-US");
    const { rerender } = render(
      <CertificationDrawer isOpen onClose={vi.fn()} locale="en-US" client={client} />
    );
    await screen.findByTestId("eval-case-row-UC-03");

    const searchInput = screen.getByTestId("eval-case-search-input");
    fireEvent.change(searchInput, { target: { value: "credential redaction" } });
    expect(screen.getByTestId("eval-case-row-UC-03")).toBeTruthy();
    expect(screen.queryByTestId("eval-case-row-UC-01")).toBeNull();

    setGlobalLocale("vi-VN");
    rerender(<CertificationDrawer isOpen onClose={vi.fn()} locale="vi-VN" client={client} />);
    fireEvent.change(searchInput, { target: { value: "khóa bí mật" } });
    expect(screen.getByTestId("eval-case-row-UC-03")).toBeTruthy();
    expect(screen.queryByTestId("eval-case-row-UC-01")).toBeNull();

    setGlobalLocale("ja-JP");
    rerender(<CertificationDrawer isOpen onClose={vi.fn()} locale="ja-JP" client={client} />);
    fireEvent.change(searchInput, { target: { value: "秘密鍵" } });
    expect(screen.getByTestId("eval-case-row-UC-03")).toBeTruthy();
    expect(screen.queryByTestId("eval-case-row-UC-01")).toBeNull();

    // Changing locale while open must NOT re-fetch history (F-09)
    expect(client.getEvalHistory).toHaveBeenCalledTimes(1);
  });

  it("updates verdict immediately on 422 tamper, clears 409 on refresh, and splits live vs historical tamper pill", async () => {
    setGlobalLocale("en-US");
    const passingRun = buildMockRun({ runId: "run-pass-1", verdict: "PASS", tamperDetected: false });
    const tamperedRun = buildMockRun({
      runId: "run-tamper-2",
      verdict: "SEALED_EVAL_TAMPER_ERROR",
      tamperDetected: true,
      tamperReason: "WRITABLE_SEALED_FILE",
    });

    const client = {
      getEvalHistory: vi
        .fn()
        .mockResolvedValueOnce(buildMockHistory(passingRun))
        .mockResolvedValueOnce(buildMockHistory(passingRun))
        .mockResolvedValueOnce({
          ok: true as const,
          requestId: "req-hist-tamper",
          data: {
            sealedIntegrityOk: false,
            sealedManifestSha256: tamperedRun.sealedManifestSha256,
            manifestPinSource: "compiled_binary" as const,
            tamperReason: "WRITABLE_SEALED_FILE",
            latestRun: tamperedRun,
            runs: [tamperedRun, passingRun],
            totalRuns: 2,
          },
        }),
      runEval: vi
        .fn()
        .mockRejectedValueOnce(new EvalCertificationApiError(409, "EVAL_ALREADY_RUNNING", "Busy"))
        .mockRejectedValueOnce(
          new EvalCertificationApiError(422, "SEALED_EVAL_TAMPER_ERROR", "WRITABLE_SEALED_FILE", {
            tamperReason: "WRITABLE_SEALED_FILE",
          })
        ),
    };

    render(<CertificationDrawer isOpen onClose={vi.fn()} locale="en-US" client={client} />);
    await screen.findByTestId("eval-case-row-UC-01");

    // 409 conflict banner appears, then clears cleanly on Refresh (F-09)
    fireEvent.click(screen.getByTestId("eval-run-trigger-btn"));
    const conflictBanner = await screen.findByTestId("eval-conflict-banner");
    expect(conflictBanner.textContent).toContain("already in progress");
    expect(conflictBanner.textContent).not.toContain("EVAL_ALREADY_RUNNING");

    fireEvent.click(screen.getByTestId("eval-refresh-btn"));
    await waitFor(() => {
      expect(screen.queryByTestId("eval-conflict-banner")).toBeNull();
    });

    // 422 tamper must immediately update verdict badge away from green "Certified Pass" (F-05)
    fireEvent.click(screen.getByTestId("eval-run-trigger-btn"));
    const tamperBanner = await screen.findByTestId("eval-tamper-banner");
    expect(tamperBanner.textContent).toContain("writable instead of strictly read-only");
    expect(tamperBanner.textContent).not.toContain("SEALED_EVAL_TAMPER_ERROR");
    expect(tamperBanner.textContent).not.toContain("WRITABLE_SEALED_FILE");
    expect(screen.getByTestId("eval-verdict-badge").textContent).toBe("Locked by Integrity Guard");

    cleanup();

    // Selecting an older tampered run while live sealedIntegrityOk === true keeps header pill healthy (F-05)
    const liveHealthyWithHistoricalTamperClient = {
      getEvalHistory: vi.fn().mockResolvedValue({
        ok: true as const,
        requestId: "req-hist-live-ok",
        data: {
          sealedIntegrityOk: true,
          sealedManifestSha256: passingRun.sealedManifestSha256,
          manifestPinSource: "compiled_binary" as const,
          tamperReason: null,
          latestRun: passingRun,
          runs: [passingRun, tamperedRun],
          totalRuns: 2,
        },
      }),
      runEval: vi.fn(),
    };

    render(
      <CertificationDrawer
        isOpen
        onClose={vi.fn()}
        locale="en-US"
        client={liveHealthyWithHistoricalTamperClient}
      />
    );
    await screen.findByTestId("eval-case-row-UC-01");

    fireEvent.click(screen.getByTestId("eval-tab-history"));
    fireEvent.click(screen.getByTestId("eval-history-item-run-tamper-2"));

    expect(screen.getByTestId("eval-sealed-integrity-pill").textContent).toContain(
      "Sealed Standards Verified"
    );
    expect(screen.getByTestId("eval-tamper-banner")).toBeTruthy();
    expect(screen.getByTestId("eval-verdict-badge").textContent).toBe("Locked by Integrity Guard");
  });

  it("clears isLoadingHistory when handleRunEval supersedes an in-flight loadHistory (F-11)", async () => {
    setGlobalLocale("en-US");
    const runResult = buildMockRun({ runId: "run-supersede-1", verdict: "PASS" });
    const deferred: { resolve?: (value: ReturnType<typeof buildMockHistory>) => void } = {};
    const initialHistoryPromise = new Promise<ReturnType<typeof buildMockHistory>>((resolve) => {
      deferred.resolve = resolve;
    });

    const client = {
      getEvalHistory: vi.fn().mockImplementation(() => initialHistoryPromise),
      runEval: vi.fn().mockResolvedValue({
        ok: true as const,
        requestId: "req-run-supersede",
        data: runResult,
      }),
    };

    render(<CertificationDrawer isOpen onClose={vi.fn()} locale="en-US" client={client} />);

    // Initial history load is still pending -> refresh button is disabled
    const refreshBtn = screen.getByTestId("eval-refresh-btn") as HTMLButtonElement;
    expect(refreshBtn.disabled).toBe(true);

    // Triggering Run supersedes the pending history load and must re-enable refreshBtn
    fireEvent.click(screen.getByTestId("eval-run-trigger-btn"));
    await screen.findByTestId("eval-case-row-UC-01");
    expect(refreshBtn.disabled).toBe(false);

    // Even after the stale initial history promise finally settles, refreshBtn stays enabled
    deferred.resolve?.(buildMockHistory(buildMockRun({ runId: "run-stale-0" })));
    await waitFor(() => {
      expect(refreshBtn.disabled).toBe(false);
    });
  });

  it("wires nav-certification-btn in ContextualStageToolbar across html, md, and code tabs", () => {
    setGlobalLocale("en-US");
    const onOpenCert = vi.fn();
    const { rerender } = render(
      <ContextualStageToolbar
        projectId="proj-1"
        activeTabId="tab-userflow"
        activeFormatTab="dag"
        onOpenCertification={onOpenCert}
      />
    );
    fireEvent.click(screen.getByTestId("nav-certification-btn"));
    expect(onOpenCert).toHaveBeenCalledTimes(1);

    rerender(
      <ContextualStageToolbar
        projectId="proj-1"
        activeTabId="tab-prd"
        activeFormatTab="md"
        onOpenCertification={onOpenCert}
      />
    );
    expect(screen.getByTestId("nav-certification-btn")).toBeTruthy();

    rerender(
      <ContextualStageToolbar
        projectId="proj-1"
        activeTabId="tab-code"
        activeFormatTab="code"
        onOpenCertification={onOpenCert}
      />
    );
    expect(screen.getByTestId("nav-certification-btn")).toBeTruthy();
  });

  it("renders concise CTA label, supports resizing RightDrawer, and toggles M1–M5 domain guide", async () => {
    setGlobalLocale("vi-VN");
    const runResult = buildMockRun({ runId: "run-ux-polish-1", verdict: "PASS" });
    const client = {
      getEvalHistory: vi.fn().mockResolvedValue(buildMockHistory(runResult)),
      runEval: vi.fn().mockResolvedValue({ ok: true as const, requestId: "req-ux", data: runResult }),
    };

    render(<CertificationDrawer isOpen onClose={vi.fn()} locale="vi-VN" client={client} />);
    await screen.findByTestId("eval-case-row-UC-01");

    // 1. Concise CTA label without redundant tier suffix
    const runBtn = screen.getByTestId("eval-run-trigger-btn");
    expect(runBtn.textContent?.trim()).toBe("Chạy kiểm định");

    // 2. Resizable drawer handle adjusts width via keyboard & resets on Home
    const drawer = screen.getByTestId("certification-drawer");
    const handle = screen.getByTestId("certification-drawer-resize-handle");
    expect(drawer.style.width).toBe("520px");
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(drawer.style.width).toBe("552px");
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(drawer.style.width).toBe("520px");
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    fireEvent.keyDown(handle, { key: "Home" });
    expect(drawer.style.width).toBe("520px");

    // 3. M1–M5 info button toggles expandable domain guide panel
    expect(screen.queryByTestId("eval-milestone-guide")).toBeNull();
    fireEvent.click(screen.getByTestId("eval-milestone-info-toggle"));
    const guide = screen.getByTestId("eval-milestone-guide");
    expect(guide.textContent).toContain("M1");
    expect(guide.textContent).toContain("M5");
    expect(guide.textContent).toContain("Khởi tạo dự án");
    fireEvent.click(screen.getByTestId("eval-milestone-guide-close"));
    expect(screen.queryByTestId("eval-milestone-guide")).toBeNull();
  });

  it("enforces zero Buffer.*, zero toLocaleString(), zero hardcoded hex/rgb, and zero Tailwind dark: across Story 13.1 frontend modules", () => {
    const featureDir = path.join(REPO_ROOT, "src/features/eval-certification");
    const featureFiles = fs
      .readdirSync(featureDir)
      .filter((name) => (name.endsWith(".ts") || name.endsWith(".tsx")) && !name.endsWith(".test.tsx"))
      .map((name) => path.join(featureDir, name));

    const guardedFiles = [
      ...featureFiles,
      path.join(REPO_ROOT, "src/api/eval-certification.ts"),
      path.join(REPO_ROOT, "src/lib/validations/eval-certification.ts"),
      path.join(REPO_ROOT, "src/pages/workbench/components/ContextualStageToolbar.tsx"),
    ];

    for (const filePath of guardedFiles) {
      const src = fs.readFileSync(filePath, "utf8");
      expect(src, `${filePath} must not use Buffer.*`).not.toMatch(/\bBuffer\./);
      expect(src, `${filePath} must not use .toLocaleString()`).not.toMatch(/\.toLocaleString\s*\(/);
      expect(src, `${filePath} must not use Tailwind dark:`).not.toMatch(/\bdark:/);
      expect(src, `${filePath} must not hardcode hex colors`).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
      expect(src, `${filePath} must not hardcode rgb/rgba colors`).not.toMatch(/\brgba?\s*\(/i);
    }
  });
});
