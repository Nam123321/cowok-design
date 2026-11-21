// @vitest-environment jsdom
// @story 02.4
// @implements AC-01, AC-02, AC-04, AC-05
import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ssotSdk from "../../src/api/ssot-registry";
import { SsotToolbarStrip } from "../../src/features/ssot-registry";

vi.mock("../../src/api/ssot-registry", () => ({
  fetchSsotComponents: vi.fn(),
  mutateAppShellLock: vi.fn(),
  exportSsotBundle: vi.fn(),
  routeDesignSkills: vi.fn(),
}));

const FORBIDDEN_INTERNAL_JARGON = [
  "fencingToken",
  "OXC",
  "SQLite",
  "prefixSha256",
  "quarantined_diff_json",
  "idempotency_records",
];

describe("Story 02.4 UI: AppShellLockPanel, SsotExportButton & PatternRegistryDrawer (E2E-UX-01)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders concise unlocked state, acquires/releases shell lock, exports with toast, and opens pattern drawer cleanly", async () => {
    vi.mocked(ssotSdk.fetchSsotComponents).mockResolvedValue({
      ok: true,
      data: {
        ok: true,
        requestId: "req-1",
        data: {
          patterns: [
            {
              id: "pat-1",
              projectId: "proj-1",
              portalId: "DeveloperPortal",
              patternSlug: "metric-card",
              componentName: "MetricCard",
              sourcePath: "src/components/MetricCard.tsx",
              astSignatureSha256: "a".repeat(64),
              dedupMarkerSha256: "b".repeat(64),
              sourceRevision: 1,
              lastSyncSource: "oxc_reverse_sync",
              tokensSnapshotJson: "{}",
              docMarkdown: "### MetricCard",
              driftStatus: "quarantined_drift",
              quarantinedDiffJson: "{}",
              revision: 1,
              createdAt: 1700000000000,
              updatedAt: 1700000000000,
            },
          ],
          lock: null,
          nextCursor: null,
          quarantinedCount: 1,
        },
      },
    });

    vi.mocked(ssotSdk.mutateAppShellLock).mockResolvedValueOnce({
      ok: true,
      data: {
        ok: true,
        requestId: "req-2",
        data: {
          lock: {
            id: "lock-1",
            projectId: "proj-1",
            portalId: "DeveloperPortal",
            lockOwnerId: "workbench-operator",
            fencingToken: 1,
            lockedSlots: ["TopBar", "LeftNav"],
            expiresAt: Date.now() + 30_000,
            isExpired: false,
            revision: 1,
            createdAt: 1700000000000,
            updatedAt: 1700000000000,
          },
        },
      },
    });

    vi.mocked(ssotSdk.exportSsotBundle).mockResolvedValueOnce({
      ok: true,
      data: {
        ok: true,
        requestId: "req-3",
        data: {
          exportId: "exp-1",
          revision: 4,
          portalId: "DeveloperPortal",
          generatedAt: 1700000001000,
          markdownPath: ".cowok-design/exports/generations/rev-4/ssot-report.md",
          markdownSha256: "c".repeat(64),
          jsonPath: ".cowok-design/exports/generations/rev-4/ssot-bundle.json",
          jsonSha256: "d".repeat(64),
        },
      },
    });

    const { container } = render(<SsotToolbarStrip projectId="proj-1" />);

    const lockPanel = await waitFor(() =>
      container.querySelector('[data-ff-id="appshell-lock-panel"]')
    );
    expect(lockPanel).not.toBeNull();
    expect(["Unlocked", "Chưa khóa"]).toContain(screen.getByTestId("appshell-lock-badge").textContent);

    // E2E-UX-01 <= 16 words & zero internal jargon
    const badgeWords = (screen.getByTestId("appshell-lock-badge").textContent ?? "")
      .trim()
      .split(/\s+/);
    expect(badgeWords.length).toBeLessThanOrEqual(16);

    fireEvent.click(screen.getByTestId("appshell-lock-toggle"));
    await waitFor(() => {
      expect(["Shell locked", "Đã khóa khung"]).toContain(
        screen.getByTestId("appshell-lock-badge").textContent
      );
    });

    fireEvent.click(screen.getByTestId("ssot-export-trigger"));
    await waitFor(() => {
      expect(screen.getByTestId("ssot-export-toast").textContent).toContain("SSOT exported cleanly");
    });

    fireEvent.click(screen.getByTestId("ssot-patterns-trigger"));
    const drawer = await waitFor(() => screen.getByTestId("pattern-registry-drawer"));
    expect(drawer.getAttribute("data-ff-id")).toBe("pattern-registry-drawer");
    expect(screen.getByTestId("pattern-item-metric-card").textContent).toContain("MetricCard");
    expect(screen.getByTestId("pattern-quarantine-badge").textContent).toMatch(/pending review|chờ duyệt/);

    const renderedText = container.textContent ?? "";
    for (const jargon of FORBIDDEN_INTERNAL_JARGON) {
      expect(renderedText).not.toContain(jargon);
    }

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByTestId("pattern-registry-drawer")).toBeNull();
    });
  });

  it("handles 409 LOCK_FENCING_CONFLICT gracefully with plain refresh prompt", async () => {
    vi.mocked(ssotSdk.fetchSsotComponents).mockResolvedValue({
      ok: true,
      data: {
        ok: true,
        requestId: "req-1",
        data: {
          patterns: [],
          lock: null,
          nextCursor: null,
          quarantinedCount: 0,
        },
      },
    });
    vi.mocked(ssotSdk.mutateAppShellLock).mockResolvedValueOnce({
      ok: false,
      status: 409,
      error: {
        code: "LOCK_FENCING_CONFLICT",
        message: "Conflict",
        requestId: "req-c",
        details: {
          currentRevision: 2,
          currentFencingToken: 2,
          expiresAt: Date.now() + 30_000,
          isExpired: false,
        },
      },
    });

    render(<SsotToolbarStrip projectId="proj-1" />);
    await waitFor(() => {
      expect(screen.getByTestId("appshell-lock-toggle").hasAttribute("disabled")).toBe(false);
    });
    fireEvent.click(screen.getByTestId("appshell-lock-toggle"));
    await waitFor(() => {
      expect(["Locked elsewhere", "Đang khóa ở phiên khác"]).toContain(
        screen.getByTestId("appshell-lock-badge").textContent
      );
      expect(["Refresh", "Làm mới"]).toContain(
        screen.getByTestId("appshell-lock-toggle").textContent
      );
    });
  });
});
