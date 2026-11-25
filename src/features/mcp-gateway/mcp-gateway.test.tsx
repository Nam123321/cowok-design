// @vitest-environment jsdom
// @story 11.2
// @implements AC-01, AC-03, AC-04 (Vitest Unit & Component Tests for Chip-Grid McpToolPermissionTable, Localized ExternalActivityTable, HostInstallerList & Zod Contracts)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { ExternalActivityTable } from "./ExternalActivityTable";
import { HostInstallerList } from "./HostInstallerList";
import { McpToolPermissionTable } from "./McpToolPermissionTable";
import { useMcpGatewayController } from "./useMcpGatewayController";
import { apiMcpGateway } from "../../api/mcp-gateway";
import { GET as getExternalActivityBff } from "../../app/api/v1/mcp/external-activity/route";
import { issueBffLoopbackSession } from "../../lib/api-guard";
import {
  McpExternalActivityResponseSchema,
  McpHostInstallRequestSchema,
  McpJsonRpcRequestSchema,
  McpToolToggleRequestSchema,
  type McpExternalActivityData,
  type McpExternalActivityDto,
  type McpHostStatusDto,
  type McpToolPolicyDto,
} from "../../lib/validations/mcp-gateway";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const SAMPLE_HOSTS: McpHostStatusDto[] = [
  {
    hostId: "cursor",
    displayName: "Cursor",
    configFormat: "json",
    defaultRelativePath: "~/.cursor/mcp.json",
    detected: true,
    installed: true,
    latestBackupId: "bkp-cursor-1",
    latestAfterSha256: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    lastInstalledAt: 1_760_000_000_000,
  },
  {
    hostId: "codex",
    displayName: "Codex",
    configFormat: "toml",
    defaultRelativePath: "~/.codex/config.toml",
    detected: false,
    installed: false,
    latestBackupId: null,
    latestAfterSha256: null,
    lastInstalledAt: null,
  },
];

const SAMPLE_POLICIES: McpToolPolicyDto[] = [
  {
    toolName: "get_app_state",
    permissionTier: "READ_ONLY",
    enabled: true,
    mcpCallable: true,
    description: "Read workspace canvas state.",
    lockedHumanOnly: false,
    updatedAt: 1_760_000_000_000,
  },
  {
    toolName: "banner_solver",
    permissionTier: "INSPECT",
    enabled: true,
    mcpCallable: true,
    description: "Compute collision-free banner grid layouts across presets G1-G15.",
    lockedHumanOnly: false,
    updatedAt: 1_760_000_000_000,
  },
  {
    toolName: "ephemeral_tool_forge",
    permissionTier: "HUMAN_ONLY",
    enabled: false,
    mcpCallable: false,
    description: "Create isolated helper scripts (requires direct in-app confirmation).",
    lockedHumanOnly: true,
    updatedAt: 1_760_000_000_000,
  },
];

const SAMPLE_ACTIVITIES: McpExternalActivityDto[] = [
  {
    id: "act-1",
    projectId: "proj-1",
    actorKind: "MCP_CLIENT",
    hostClient: "cursor",
    methodName: "tools/call",
    toolName: "get_app_state",
    permissionTier: "READ_ONLY",
    status: "OK",
    jsonrpcCode: null,
    latencyMs: 4,
    idempotencyKey: null,
    summary: "get_app_state v1 (2 nodes)",
    createdAt: 1_760_000_000_100,
  },
  {
    id: "act-2",
    projectId: "proj-1",
    actorKind: "MCP_CLIENT",
    hostClient: "cursor",
    methodName: "tools/call",
    toolName: "ephemeral_tool_forge",
    permissionTier: "HUMAN_ONLY",
    status: "DENIED_HUMAN_ONLY",
    jsonrpcCode: -32003,
    latencyMs: 1,
    idempotencyKey: null,
    summary: "Blocked privileged tool 'ephemeral_tool_forge' from external MCP client",
    createdAt: 1_760_000_000_200,
  },
];

describe("Story 11.2 Open MCP Gateway UI & Contracts", () => {
  it("renders HostInstallerList with 1-click install, SHA-256 backup pill, and rollback actions", () => {
    const onInstall = vi.fn();
    const onRollback = vi.fn();
    render(
      <HostInstallerList
        hosts={SAMPLE_HOSTS}
        serverUrl="http://127.0.0.1:4171/api/v1/mcp/rpc"
        busyHostId={null}
        loading={false}
        onInstall={onInstall}
        onRollback={onRollback}
      />
    );

    const section = document.querySelector("[data-ff-id='mcp-host-installer-list']");
    expect(section).not.toBeNull();
    expect(section?.textContent).not.toContain("O_NOFOLLOW");
    expect(screen.getByTestId("mcp-backup-pill-cursor").textContent).toContain("abcdef1234");

    fireEvent.click(screen.getByTestId("mcp-install-btn-codex"));
    expect(onInstall).toHaveBeenCalledWith("codex");

    fireEvent.click(screen.getByTestId("mcp-rollback-btn-cursor"));
    expect(onRollback).toHaveBeenCalledWith("cursor", "bkp-cursor-1");
  });

  it("renders Chip-Grid McpToolPermissionTable with single localized lock badge, zero dev jargon, and interactive toggles", () => {
    const onToggle = vi.fn();
    render(
      <McpToolPermissionTable
        policies={SAMPLE_POLICIES}
        busyToolName={null}
        onToggle={onToggle}
      />
    );

    const section = document.querySelector("[data-ff-id='mcp-tool-permission-matrix']");
    expect(section).not.toBeNull();
    expect(section?.querySelector("table")).toBeNull();

    // HUMAN_ONLY card has ONE localized lock badge, zero duplicate "HUMAN_ONLY" text, zero "Story 08." jargon, and no switch
    const lockedCard = screen.getByTestId("mcp-policy-row-ephemeral_tool_forge");
    const lockPill = screen.getByTestId("mcp-human-only-lock-ephemeral_tool_forge");
    expect(lockPill).toBeTruthy();
    expect(screen.queryByTestId("mcp-tool-toggle-ephemeral_tool_forge")).toBeNull();
    expect(lockedCard.textContent).not.toContain("Story 08.");
    expect(lockedCard.textContent).not.toContain("HUMAN_ONLY");
    expect(lockedCard.textContent).not.toContain("EC-LEARN-001");

    // INSPECT card toggles cleanly
    fireEvent.click(screen.getByTestId("mcp-tool-toggle-banner_solver"));
    expect(onToggle).toHaveBeenCalledWith(SAMPLE_POLICIES[1], false);
  });

  it("renders ExternalActivityTable with localized filter pills, localized actor labels, and data-status selectors", () => {
    const onFilterChange = vi.fn();
    const onRefresh = vi.fn();
    render(
      <ExternalActivityTable
        activities={SAMPLE_ACTIVITIES}
        loading={false}
        statusFilter="ALL"
        onFilterChange={onFilterChange}
        onRefresh={onRefresh}
      />
    );

    const section = document.querySelector("[data-ff-id='mcp-external-activity-table']");
    expect(section).not.toBeNull();
    const deniedRow = screen.getByTestId("mcp-activity-row-act-2");
    const statusPill = deniedRow.querySelector("[data-status='DENIED_HUMAN_ONLY']");
    expect(statusPill).not.toBeNull();
    expect(statusPill?.textContent).not.toBe("DENIED_HUMAN_ONLY");

    const filterBtn = screen.getByTestId("mcp-activity-filter-DENIED_HUMAN_ONLY");
    expect(filterBtn.textContent).not.toBe("DENIED_HUMAN_ONLY");
    fireEvent.click(filterBtn);
    expect(onFilterChange).toHaveBeenCalledWith("DENIED_HUMAN_ONLY");

    fireEvent.click(screen.getByTestId("mcp-activity-refresh-btn"));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("validates strict boundary Zod schemas for install, toggle, RPC, and activity responses", () => {
    expect(McpHostInstallRequestSchema.parse({ hostId: "cursor", action: "install" }).hostId).toBe("cursor");
    expect(() => McpHostInstallRequestSchema.parse({ hostId: "evil_host" })).toThrow();
    expect(McpToolToggleRequestSchema.parse({ toolName: "banner_solver", enabled: false }).enabled).toBe(false);
    expect(McpJsonRpcRequestSchema.parse({ jsonrpc: "2.0", id: 1, method: "tools/list" }).method).toBe("tools/list");
    expect(
      McpExternalActivityResponseSchema.parse({
        ok: true,
        requestId: "req-11-2",
        data: {
          policies: SAMPLE_POLICIES,
          activities: SAMPLE_ACTIVITIES,
          hosts: SAMPLE_HOSTS,
          backups: [],
          serverUrl: "http://127.0.0.1:4171/api/v1/mcp/rpc",
          generatedAt: "2026-11-24T00:00:00Z",
        },
      }).ok
    ).toBe(true);
  });

  it("validates GET /api/v1/mcp/external-activity query params via McpExternalActivityQuerySchema (MEDIUM-07)", async () => {
    const sess = issueBffLoopbackSession({
      tenantId: "tenant-alpha",
      userId: "operator-1",
      role: "editor",
    });

    for (const badQuery of [
      "?limit=999",
      "?limit=0",
      "?status=NOT_A_STATUS",
      "?unexpected=1",
      "?limit=10&limit=20",
    ]) {
      const req = new Request(`http://127.0.0.1:5173/api/v1/mcp/external-activity${badQuery}`, {
        method: "GET",
        headers: {
          host: "127.0.0.1:5173",
          "x-cowok-session-token": sess.sessionToken,
        },
      });
      const resp = await getExternalActivityBff(req);
      expect(resp.status).toBe(400);
      const body = (await resp.json()) as { error?: { code?: string } };
      expect(body.error?.code).toBe("VALIDATION_ERROR");
    }
  });

  it("discards stale out-of-order refresh() responses and surfaces localized messages in useMcpGatewayController (MEDIUM-08)", async () => {
    let resolveFirst!: (val: McpExternalActivityData) => void;
    let resolveSecond!: (val: McpExternalActivityData) => void;
    const firstPromise = new Promise<McpExternalActivityData>((res) => {
      resolveFirst = res;
    });
    const secondPromise = new Promise<McpExternalActivityData>((res) => {
      resolveSecond = res;
    });

    const getSpy = vi
      .spyOn(apiMcpGateway, "getExternalActivity")
      .mockReturnValueOnce(firstPromise)
      .mockReturnValueOnce(secondPromise);

    const { result } = renderHook(() => useMcpGatewayController(true));
    expect(getSpy).toHaveBeenCalledTimes(1);

    // Trigger a newer refresh (e.g., user switches status filter) before the first completes
    act(() => {
      result.current.setStatusFilter("DENIED_HUMAN_ONLY");
    });
    await waitFor(() => expect(getSpy).toHaveBeenCalledTimes(2));

    const freshData: McpExternalActivityData = {
      policies: SAMPLE_POLICIES,
      activities: [SAMPLE_ACTIVITIES[1]],
      hosts: SAMPLE_HOSTS,
      backups: [],
      serverUrl: "http://127.0.0.1:4171/api/v1/mcp/rpc",
      generatedAt: "2026-11-24T00:00:02Z",
    };
    const staleData: McpExternalActivityData = {
      policies: SAMPLE_POLICIES,
      activities: [SAMPLE_ACTIVITIES[0]],
      hosts: SAMPLE_HOSTS,
      backups: [],
      serverUrl: "http://127.0.0.1:4171/api/v1/mcp/rpc",
      generatedAt: "2026-11-24T00:00:01Z",
    };

    // Newer second request resolves first
    await act(async () => {
      resolveSecond(freshData);
      await secondPromise;
    });
    expect(result.current.activities).toHaveLength(1);
    expect(result.current.activities[0]?.id).toBe("act-2");

    // Stale first request resolves later -> MUST be discarded!
    await act(async () => {
      resolveFirst(staleData);
      await firstPromise;
    });
    expect(result.current.activities).toHaveLength(1);
    expect(result.current.activities[0]?.id).toBe("act-2");

    // Attempting to toggle a HUMAN_ONLY tool sets localized error without calling API
    const toggleSpy = vi.spyOn(apiMcpGateway, "toggleToolPolicy");
    await act(async () => {
      await result.current.toggleTool(SAMPLE_POLICIES[2], true);
    });
    expect(toggleSpy).not.toHaveBeenCalled();
    expect(result.current.error).toBeTruthy();
  });
});
