// @vitest-environment jsdom
// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as contextSdk from "../../api/context";
import { ContextMeterWidget } from "./ContextMeterWidget";

vi.mock("../../api/context", () => ({
  assembleContext: vi.fn(),
  getContextTelemetry: vi.fn(),
}));

describe("ContextMeterWidget", () => {
  const projId = "11111111-1111-4111-8111-111111111111";

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders default empty/ready states, runs assembleContext on click, and surfaces 403/422 errors via role=alert", async () => {
    vi.mocked(contextSdk.getContextTelemetry).mockResolvedValue({
      ok: true,
      requestId: projId,
      data: {
        items: [],
        page: { limit: 10, nextCursor: null, order: "created_at_desc_id_desc" },
      },
    });

    vi.mocked(contextSdk.assembleContext).mockResolvedValueOnce({
      ok: true,
      requestId: projId,
      data: {
        telemetryId: projId,
        projectId: projId,
        portalId: "DeveloperPortal",
        horizon: "mid",
        modelProfile: "B",
        adapterMode: "local_reduced",
        reasonCode: "atomic_complete",
        staticPrefixSha256: "b".repeat(64),
        tokenUsage: {
          staticPrefix: 150,
          dynamicTail: 110,
          outline: 90,
          textTotal: 350,
          image: 0,
          ssotDom: 240,
          nonSsotText: 110,
          ssotDomRatio: 0.6857,
        },
        sources: { requested: 3, included: 3, pruned: 0 },
        latencyMs: 9,
        assembledAt: "2026-11-21T01:00:00.000Z",
      },
    });

    render(<ContextMeterWidget projectId={projId} />);

    const trigger = screen.getByTestId("context-meter-trigger");
    fireEvent.click(trigger);

    expect(screen.getByTestId("context-meter-popover")).toBeDefined();
    await waitFor(() => {
      expect(screen.getByText(/Chọn dự án|Select a project/i)).toBeDefined();
    });

    fireEvent.change(screen.getByTestId("context-meter-profile-select"), {
      target: { value: "B" },
    });
    await waitFor(() => {
      expect(screen.getByTestId("context-meter-refresh-btn").hasAttribute("disabled")).toBe(false);
    });

    fireEvent.click(screen.getByTestId("context-meter-refresh-btn"));
    await waitFor(() => {
      expect(screen.getByText("350 / 1500")).toBeDefined();
      expect(screen.getByText(/Mã SHA: bbbbbbbbbb|SHA: bbbbbbbbbb/)).toBeDefined();
    });

    vi.mocked(contextSdk.assembleContext).mockResolvedValueOnce({
      ok: false,
      requestId: projId,
      error: {
        code: "CROSS_PORTAL_LEAK_BLOCKED",
        message: "contextMeter.errors.crossPortalLeak",
        retryable: false,
      },
    });
    fireEvent.click(screen.getByTestId("context-meter-refresh-btn"));
    await waitFor(() => {
      const alert = screen.getByRole("alert");
      expect(alert.textContent).toContain("CROSS_PORTAL_LEAK_BLOCKED");
    });
  });

  it("clears previous snapshot immediately when switching portal, horizon, or projectId", async () => {
    const otherProjId = "22222222-2222-4222-8222-222222222222";
    vi.mocked(contextSdk.getContextTelemetry)
      .mockResolvedValueOnce({
        ok: true,
        requestId: projId,
        data: {
          items: [
            {
              telemetryId: projId,
              projectId: projId,
              portalId: "DeveloperPortal",
              horizon: "mid",
              modelProfile: "A",
              adapterMode: "frontier_full",
              reasonCode: "atomic_complete",
              staticPrefixSha256: "c".repeat(64),
              tokenUsage: {
                outline: 120,
                textTotal: 640,
                image: 0,
                ssotDomRatio: 0.78,
              },
              latencyMs: 6,
              status: "success",
              createdAt: "2026-11-21T01:00:00.000Z",
            },
          ],
          page: { limit: 10, nextCursor: null, order: "created_at_desc_id_desc" },
        },
      })
      .mockResolvedValueOnce({
        ok: true,
        requestId: projId,
        data: { items: [], page: { limit: 10, nextCursor: null, order: "created_at_desc_id_desc" } },
      })
      .mockResolvedValueOnce({
        ok: true,
        requestId: otherProjId,
        data: { items: [], page: { limit: 10, nextCursor: null, order: "created_at_desc_id_desc" } },
      })
      .mockResolvedValueOnce({
        ok: false,
        requestId: otherProjId,
        error: {
          code: "CROSS_PORTAL_LEAK_BLOCKED",
          message: "contextMeter.errors.crossPortalLeak",
          retryable: false,
        },
      });

    const view = render(<ContextMeterWidget projectId={projId} />);
    fireEvent.click(screen.getByTestId("context-meter-trigger"));

    await waitFor(() => {
      expect(screen.getByText("640 / 1500")).toBeDefined();
      expect(screen.getByText(/Mã SHA: cccccccccc|SHA: cccccccccc/)).toBeDefined();
    });

    fireEvent.change(screen.getByTestId("context-meter-horizon-select"), {
      target: { value: "long" },
    });
    await waitFor(() => {
      expect(screen.queryByText("640 / 1500")).toBeNull();
      expect(screen.queryByText(/Mã SHA: cccccccccc|SHA: cccccccccc/)).toBeNull();
      expect(screen.getByText("0 / 1500")).toBeDefined();
    });

    view.rerender(<ContextMeterWidget projectId={otherProjId} />);
    await waitFor(() => {
      expect(screen.getByText("0 / 1500")).toBeDefined();
    });

    fireEvent.change(screen.getByTestId("context-meter-portal-select"), {
      target: { value: "MarketingWeb" },
    });

    await waitFor(() => {
      expect(screen.queryByText("640 / 1500")).toBeNull();
      expect(screen.queryByText(/Mã SHA: cccccccccc|SHA: cccccccccc/)).toBeNull();
      expect(screen.getByText("0 / 1500")).toBeDefined();
      expect(screen.getByRole("alert").textContent).toContain("CROSS_PORTAL_LEAK_BLOCKED");
    });
  });
});
