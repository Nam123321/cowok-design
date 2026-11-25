// @vitest-environment jsdom
// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, renderHook, act, screen, waitFor } from "@testing-library/react";
import { SplitExportApiClient, SplitExportApiError } from "../../api/split-export";
import { PptxExportDataSchema, PptxExportResponseSchema } from "../../lib/validations/split-export";
import { ExportBundleModal } from "../snapshots/components/ExportBundleModal";
import { ContextualStageToolbar } from "../../pages/workbench/components/ContextualStageToolbar";
import { setGlobalLocale } from "../../hooks/useTranslation";
import { DeveloperHandoffPanel } from "./DeveloperHandoffPanel";
import { QcGateBannerCard } from "./QcGateBannerCard";
import { SharePresentPanel } from "./SharePresentPanel";
import { useSplitExport } from "./useSplitExport";

const REPO_ROOT = path.resolve(__dirname, "../../..");

afterEach(() => {
  cleanup();
  setGlobalLocale("vi-VN");
  vi.restoreAllMocks();
});

describe("Story 07.2: Split Export SDK Zod Validation & Error Typing", () => {
  it("parses valid offline HTML, Vector PDF, PPTX fallback, and Developer Handoff responses with Zod", async () => {
    const mockFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok-072" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      if (url.includes("/api/v1/export/html-bundle")) {
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-1",
            data: {
              exportId: "exp-1",
              projectId: "proj-1",
              audienceMode: "SHARE_PRESENT",
              format: "OFFLINE_HTML",
              fileName: "pitch-offline.html",
              sha256: "a".repeat(64),
              byteSize: 2048,
              externalCdnRefsCount: 0,
              strippedExternalRefsCount: 2,
              presenterEnabled: true,
              slideCount: 3,
              svgCount: 1,
              brokenSvgCount: 0,
              inlineAssetsCount: 1,
              brokenCasAssetsCount: 0,
              cspHeader: "default-src 'self' 'unsafe-inline' data: blob:;",
              qcGateStatus: "PASS",
              qcOverrideAcknowledged: false,
              warnings: [],
              htmlContent: "<!DOCTYPE html><html><body>OK</body></html>",
              createdAt: 1730000000000,
            },
          }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        );
      }
      if (url.includes("/api/v1/export/pptx")) {
        return new Response(
          JSON.stringify({
            ok: true,
            pluginInstalled: false,
            pluginId: "pptx-deck-exporter-plugin",
            installAction: "/plugins?install=pptx-deck-exporter-plugin",
            message: "Install extension to export PPTX",
            requestId: "req-2",
            data: {
              exportId: "exp-2",
              projectId: "proj-1",
              audienceMode: "SHARE_PRESENT",
              format: "PLUGIN_PPTX",
              pluginInstalled: false,
              pluginId: "pptx-deck-exporter-plugin",
              pluginStatus: "NOT_INSTALLED",
              installAction: "/plugins?install=pptx-deck-exporter-plugin",
              message: "Install extension to export PPTX",
              fileName: null,
              sha256: null,
              byteSize: 0,
              qcGateStatus: "PASS",
              qcOverrideAcknowledged: false,
              createdAt: 1730000000001,
            },
          }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({
          error: "QC_GATE_P0_BLOCKED",
          p0Count: 2,
          message: "Export paused due to critical layout issues",
        }),
        { status: 409, headers: { "Content-Type": "application/json" } }
      );
    }) as unknown as typeof fetch;

    const client = new SplitExportApiClient({
      baseUrl: "http://127.0.0.1:4170",
      fetchImpl: mockFetch,
    });

    const htmlRes = await client.exportOfflineHtml({
      projectId: "proj-1",
      title: "Pitch",
      html: "<section><h1>Hello</h1></section>",
    });
    expect(htmlRes.data.externalCdnRefsCount).toBe(0);
    expect(htmlRes.data.presenterEnabled).toBe(true);

    const pptxRes = await client.exportPptx({ projectId: "proj-1", title: "Pitch" });
    expect(pptxRes.pluginInstalled).toBe(false);
    expect(pptxRes.installAction).toBe("/plugins?install=pptx-deck-exporter-plugin");

    let caughtError: unknown = null;
    try {
      await client.exportVectorPdf({ projectId: "proj-1", title: "Pitch" });
    } catch (err) {
      caughtError = err;
    }
    expect(caughtError).toBeInstanceOf(SplitExportApiError);
    expect(caughtError).toMatchObject({
      status: 409,
      code: "QC_GATE_P0_BLOCKED",
      p0Count: 2,
    });
  });
});

describe("Story 07.2: Split Export UI Panels, QC Gate Banner & Tab Switcher", () => {
  it("defaults ExportBundleModal to Share & Present tab when opened for a project and switches cleanly to Developer Handoff", async () => {
    render(
      <ExportBundleModal
        isOpen={true}
        projectId="11111111-1111-4111-8111-111111111111"
        projectTitle="Investor Deck"
        metadata={null}
        isExporting={false}
        onDownloadZip={vi.fn()}
        onCreateGitHubPR={vi.fn()}
        onClose={vi.fn()}
      />
    );

    const modal = screen.getByTestId("export-bundle-modal");
    expect(modal.getAttribute("data-ff-id")).toBe("export-bundle-modal");
    expect(screen.getByTestId("share-present-tab").getAttribute("aria-selected")).toBe("true");
    expect(screen.getByTestId("share-present-panel")).toBeTruthy();

    fireEvent.click(screen.getByTestId("developer-handoff-tab"));
    await waitFor(() => {
      expect(screen.getByTestId("developer-handoff-tab").getAttribute("aria-selected")).toBe("true");
      expect(screen.getByTestId("developer-handoff-panel")).toBeTruthy();
      expect(screen.getByTestId("export-dev-handoff-btn")).toBeTruthy();
    });
  });

  it("renders SharePresentPanel PPTX extension install CTA without crashing when plugin is absent", () => {
    const onNavigate = vi.fn();
    render(
      <SharePresentPanel
        activeFormat={null}
        htmlResult={null}
        pdfResult={null}
        pptxPluginInstalled={false}
        pptxResult={{
          exportId: "exp-pptx",
          projectId: "proj-1",
          audienceMode: "SHARE_PRESENT",
          format: "PLUGIN_PPTX",
          pluginInstalled: false,
          pluginId: "pptx-deck-exporter-plugin",
          pluginStatus: "NOT_INSTALLED",
          installAction: "/plugins?install=pptx-deck-exporter-plugin",
          message: "Install extension",
          fileName: null,
          sha256: null,
          byteSize: 0,
          qcGateStatus: "UNSCANNED",
          qcOverrideAcknowledged: false,
          createdAt: 1730000000000,
        }}
        onExportOfflineHtml={vi.fn()}
        onExportVectorPdf={vi.fn()}
        onExportPptx={vi.fn()}
        onNavigatePluginInstall={onNavigate}
      />
    );

    const cta = screen.getByTestId("export-pptx-plugin-cta");
    expect(cta.getAttribute("data-ff-id")).toBe("export-pptx-plugin-cta");
    fireEvent.click(cta);
    expect(onNavigate).toHaveBeenCalledWith("/plugins?install=pptx-deck-exporter-plugin");
  });

  it("renders QcGateBannerCard with explicit override checkbox and zero raw Story/enum codes", () => {
    const onToggle = vi.fn();
    const { container } = render(
      <QcGateBannerCard
        qcGate={{ status: "FAIL", p0Count: 2, p1Count: 1, latestRunId: "run-1" }}
        overrideQcWarning={false}
        onToggleOverride={onToggle}
      />
    );

    const text = container.textContent || "";
    expect(text).not.toContain("Story 07.1");
    expect(text).not.toContain("QC_GATE_P0_BLOCKED");
    expect(text).not.toContain("UNVERIFIED_NO_BROWSER");

    fireEvent.click(screen.getByTestId("qc-override-checkbox"));
    expect(onToggle).toHaveBeenCalledWith(true);
  });

  it("renders DeveloperHandoffPanel receipt files cleanly", () => {
    render(
      <DeveloperHandoffPanel
        activeFormat={null}
        hasProjectId={true}
        devHandoffResult={{
          exportId: "exp-dev",
          projectId: "proj-1",
          audienceMode: "DEVELOPER_HANDOFF",
          format: "DEV_HANDOFF_BUNDLE",
          fileName: "billing-developer-handoff.json",
          sha256: "b".repeat(64),
          byteSize: 1024,
          files: [
            { path: "SPEC.md", mimeType: "text/markdown", byteSize: 200, sha256: "c".repeat(64), content: "# Spec" },
            { path: "DESIGN_TOKENS.css", mimeType: "text/css", byteSize: 150, sha256: "d".repeat(64), content: ":root{}" },
          ],
          qcGateStatus: "PASS",
          qcOverrideAcknowledged: false,
          createdAt: 1730000000000,
        }}
        onExportDeveloperHandoff={vi.fn()}
      />
    );

    expect(screen.getByTestId("dev-handoff-receipt").textContent).toContain("SPEC.md");
    expect(screen.getByTestId("dev-handoff-receipt").textContent).toContain("DESIGN_TOKENS.css");
  });

  it("enforces zero Buffer.*, zero toLocaleString(), zero hardcoded hex/rgb colors, and zero Tailwind dark: classes across split-export and ContextualStageToolbar", () => {
    const dir = path.join(REPO_ROOT, "src/features/split-export");
    const splitExportFiles = fs
      .readdirSync(dir)
      .filter((name) => (name.endsWith(".ts") || name.endsWith(".tsx")) && !name.endsWith(".test.tsx"))
      .map((name) => path.join(dir, name));
    const guardedFiles = [
      ...splitExportFiles,
      path.join(REPO_ROOT, "src/features/snapshots/components/ExportBundleModal.tsx"),
      path.join(REPO_ROOT, "src/pages/workbench/components/ContextualStageToolbar.tsx"),
    ];

    for (const filePath of guardedFiles) {
      const src = fs.readFileSync(filePath, "utf8");
      expect(src).not.toMatch(/\bBuffer\./);
      expect(src).not.toMatch(/\.toLocaleString\s*\(/);
      expect(src).not.toMatch(/\bdark:/);
      expect(src).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
      expect(src).not.toMatch(/\brgba?\s*\(/i);
    }
  });

  it("formats QcGateBannerCard p0Count and p1Count with useFormatter and interpolates {{count}}", () => {
    setGlobalLocale("en-US");
    const { container } = render(
      <QcGateBannerCard
        qcGate={{ status: "FAIL", p0Count: 1200, p1Count: 1500, latestRunId: "run-fmt" }}
        overrideQcWarning={false}
        onToggleOverride={vi.fn()}
      />
    );

    const text = container.textContent || "";
    expect(text).toContain("1,200");
    expect(text).toContain("1,500");
    expect(text).not.toContain("{{count}}");
  });

  it("adapts ContextualStageToolbar across .html, .md, and .tsx tabs and formats activeTournamentRunCount", () => {
    setGlobalLocale("en-US");
    const onOpenQc = vi.fn();
    const onOpenTournament = vi.fn();

    const { rerender } = render(
      <ContextualStageToolbar
        projectId="11111111-1111-4111-8111-111111111111"
        activeTabId="tab-userflow"
        activeFormatTab="dag"
        onOpenVisualQc={onOpenQc}
        onOpenTournament={onOpenTournament}
        activeTournamentRunCount={1250}
      />
    );

    expect(screen.getByTestId("contextual-stage-toolbar").getAttribute("data-tab-kind")).toBe("html");
    expect(screen.getByTestId("nav-visual-qc-btn")).toBeTruthy();
    const tournamentBtn = screen.getByTestId("nav-tournament-btn");
    expect(screen.getByTestId("tournament-active-run-badge").textContent).toBe("1,250");
    expect(tournamentBtn.getAttribute("aria-label")).toContain("1,250");
    expect(tournamentBtn.getAttribute("aria-label")).not.toContain("{{count}}");

    fireEvent.click(screen.getByTestId("nav-visual-qc-btn"));
    fireEvent.click(tournamentBtn);
    expect(onOpenQc).toHaveBeenCalledTimes(1);
    expect(onOpenTournament).toHaveBeenCalledTimes(1);

    rerender(
      <ContextualStageToolbar
        projectId="11111111-1111-4111-8111-111111111111"
        activeTabId="tab-prd"
        activeFormatTab="md"
        onOpenVisualQc={onOpenQc}
        onOpenTournament={onOpenTournament}
        activeTournamentRunCount={3}
      />
    );
    expect(screen.getByTestId("contextual-stage-toolbar").getAttribute("data-tab-kind")).toBe("md");
    expect(screen.getByTestId("contextual-stage-badge")).toBeTruthy();
    expect(screen.getByTestId("nav-tournament-btn")).toBeTruthy();
    expect(screen.queryByTestId("nav-visual-qc-btn")).toBeNull();

    rerender(
      <ContextualStageToolbar
        projectId="11111111-1111-4111-8111-111111111111"
        activeTabId="tab-code"
        activeFormatTab="code"
        onOpenVisualQc={onOpenQc}
        onOpenTournament={onOpenTournament}
        activeTournamentRunCount={3}
      />
    );
    expect(screen.getByTestId("contextual-stage-toolbar").getAttribute("data-tab-kind")).toBe("code");
    expect(screen.getByTestId("contextual-stage-badge")).toBeTruthy();
    expect(screen.getByTestId("nav-visual-qc-btn")).toBeTruthy();
    expect(screen.queryByTestId("nav-tournament-btn")).toBeNull();
  });

  it("validates PptxExportDataSchema.installAction against strict relative-path regex", () => {
    const baseData = {
      exportId: "exp-pptx",
      projectId: "proj-1",
      audienceMode: "SHARE_PRESENT" as const,
      format: "PLUGIN_PPTX" as const,
      pluginInstalled: false,
      pluginId: "pptx-deck-exporter-plugin",
      pluginStatus: "NOT_INSTALLED",
      installAction: "/plugins?install=pptx-deck-exporter-plugin",
      message: "Install extension",
      fileName: null,
      sha256: null,
      byteSize: 0,
      qcGateStatus: "PASS",
      qcOverrideAcknowledged: false,
      createdAt: 1730000000000,
    };

    expect(PptxExportDataSchema.safeParse(baseData).success).toBe(true);

    for (const badAction of [
      "javascript:alert(1)",
      "https://evil.com/plugins",
      "//evil.com/plugins",
      "/plugins/../../etc/passwd",
      "/plugins?a=1<script>",
    ]) {
      expect(
        PptxExportDataSchema.safeParse({ ...baseData, installAction: badAction }).success,
        `Expected ${badAction} to fail`
      ).toBe(false);
      expect(
        PptxExportResponseSchema.safeParse({
          ok: true,
          pluginInstalled: false,
          pluginId: "pptx-deck-exporter-plugin",
          installAction: badAction,
          message: "Install",
          data: { ...baseData, installAction: badAction },
          requestId: "req-bad",
        }).success
      ).toBe(false);
    }
  });

  it("renders SharePresentPanel with human-readable KB/MB sizes and friendly offline text instead of raw bytes or hardcoded 0 CDN", () => {
    const { rerender } = render(
      <SharePresentPanel
        activeFormat={null}
        htmlResult={{
          exportId: "exp-html",
          projectId: "proj-1",
          audienceMode: "SHARE_PRESENT",
          format: "OFFLINE_HTML",
          fileName: "deck-offline.html",
          sha256: "a".repeat(64),
          byteSize: 2048,
          externalCdnRefsCount: 0,
          strippedExternalRefsCount: 1,
          presenterEnabled: true,
          slideCount: 3,
          svgCount: 1,
          brokenSvgCount: 0,
          inlineAssetsCount: 0,
          brokenCasAssetsCount: 0,
          cspHeader: "default-src 'none';",
          qcGateStatus: "PASS",
          qcOverrideAcknowledged: false,
          warnings: [],
          htmlContent: "<!DOCTYPE html>",
          createdAt: 1730000000000,
        }}
        pdfResult={null}
        pptxResult={null}
        pptxPluginInstalled={false}
        onExportOfflineHtml={vi.fn()}
        onExportVectorPdf={vi.fn()}
        onExportPptx={vi.fn()}
      />
    );

    const receiptText = screen.getByTestId("offline-html-receipt").textContent || "";
    expect(receiptText).toContain("KB");
    expect(receiptText).not.toContain("bytes");
    expect(receiptText).not.toContain("0 CDN");
    expect(receiptText).toContain("Sẵn sàng chạy ngoại tuyến");

    rerender(
      <SharePresentPanel
        activeFormat={null}
        htmlResult={{
          exportId: "exp-html-2",
          projectId: "proj-1",
          audienceMode: "SHARE_PRESENT",
          format: "OFFLINE_HTML",
          fileName: "deck-offline.html",
          sha256: "a".repeat(64),
          byteSize: 2 * 1024 * 1024,
          externalCdnRefsCount: 2,
          strippedExternalRefsCount: 0,
          presenterEnabled: true,
          slideCount: 3,
          svgCount: 0,
          brokenSvgCount: 0,
          inlineAssetsCount: 0,
          brokenCasAssetsCount: 0,
          cspHeader: "default-src 'none';",
          qcGateStatus: "PASS",
          qcOverrideAcknowledged: false,
          warnings: [],
          htmlContent: "<!DOCTYPE html>",
          createdAt: 1730000000000,
        }}
        pdfResult={null}
        pptxResult={null}
        pptxPluginInstalled={false}
        onExportOfflineHtml={vi.fn()}
        onExportVectorPdf={vi.fn()}
        onExportPptx={vi.fn()}
      />
    );

    const warnText = screen.getByTestId("offline-html-receipt").textContent || "";
    expect(warnText).toContain("MB");
    expect(warnText).not.toContain("Sẵn sàng chạy ngoại tuyến");
    expect(warnText).toContain("2");
  });

  it("resets overrideQcWarning & state across project switches and HTML-escapes projectTitle in fallback HTML", async () => {
    const capturedPayloads: Array<{ html?: string }> = [];
    const mockClient = {
      getExportHistory: vi.fn(async () => ({
        ok: true as const,
        requestId: "req-h",
        data: {
          projectId: "proj-a",
          qcGate: { status: "FAIL", p0Count: 1, p1Count: 0, latestRunId: "r1" },
          pptxPluginInstalled: false,
          items: [],
        },
      })),
      exportOfflineHtml: vi.fn(async (payload: { html?: string }) => {
        capturedPayloads.push(payload);
        return {
          ok: true as const,
          requestId: "req-e",
          data: {
            exportId: "exp-1",
            projectId: "proj-a",
            audienceMode: "SHARE_PRESENT" as const,
            format: "OFFLINE_HTML" as const,
            fileName: "deck-offline.html",
            sha256: "a".repeat(64),
            byteSize: 1024,
            externalCdnRefsCount: 0,
            strippedExternalRefsCount: 0,
            presenterEnabled: true,
            slideCount: 1,
            svgCount: 0,
            brokenSvgCount: 0,
            inlineAssetsCount: 0,
            brokenCasAssetsCount: 0,
            cspHeader: "default-src 'none';",
            qcGateStatus: "PASS",
            qcOverrideAcknowledged: true,
            warnings: [],
            htmlContent: "<!DOCTYPE html>",
            createdAt: 1730000000000,
          },
        };
      }),
    } as unknown as SplitExportApiClient;

    const { result, rerender } = renderHook(
      (props: { projectId: string; projectTitle: string; enabled: boolean }) =>
        useSplitExport({
          projectId: props.projectId,
          projectTitle: props.projectTitle,
          previewHtml: "",
          enabled: props.enabled,
          client: mockClient,
          autoDownload: false,
        }),
      {
        initialProps: {
          projectId: "proj-a",
          projectTitle: '<img src=x onerror="alert(1)"> & Pitch',
          enabled: true,
        },
      }
    );

    act(() => {
      result.current.setOverrideQcWarning(true);
    });
    expect(result.current.overrideQcWarning).toBe(true);

    await act(async () => {
      await result.current.runOfflineHtmlExport();
    });

    expect(capturedPayloads[0]?.html).toContain(
      "&lt;img src=x onerror=&quot;alert(1)&quot;&gt; &amp; Pitch"
    );
    expect(capturedPayloads[0]?.html).not.toContain('<img src=x onerror="alert(1)">');
    expect(result.current.htmlResult).not.toBeNull();

    // Switch project -> overrideQcWarning and prior results must reset immediately
    rerender({ projectId: "proj-b", projectTitle: "Project B", enabled: true });
    expect(result.current.overrideQcWarning).toBe(false);
    expect(result.current.htmlResult).toBeNull();
    expect(result.current.errorNotice).toBeNull();
  });
});
