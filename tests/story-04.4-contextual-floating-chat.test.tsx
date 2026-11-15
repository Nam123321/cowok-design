/** @vitest-environment jsdom */
// @story 04.4
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import {
  ContextualNodeFloatingChat,
  FLOATING_CHAT_LAYOUT_STORAGE_KEY,
  MIN_WIDTH,
  MAX_WIDTH,
  MIN_HEIGHT,
  MAX_HEIGHT,
  getEffectiveMaxHeight,
} from "../src/features/flow/components/ContextualNodeFloatingChat";
import { NodeQueueBadge } from "../src/features/flow/components/NodeQueueBadge";
import { formatStructuredPayload } from "../src/features/flow/components/FloatingChatStreamPanel";
import type { WorkbenchNode } from "../src/features/flow/store/flow-workbench-store";

describe("Story 04.4: Contextual Floating Chat Drag/Resize Persistence & Canonical ai-agent Iconology", () => {
  const mockNodeA: WorkbenchNode = {
    id: "node-scr-01",
    screenKey: "SCR-01",
    title: "Màn hình Đăng nhập",
    posX: 100,
    posY: 100,
    version: 1,
    status: "synced",
    activeSubState: "Default",
    nodeType: "screen",
  };

  const mockNodeB: WorkbenchNode = {
    id: "node-scr-02",
    screenKey: "SCR-02",
    title: "Màn hình Giỏ hàng",
    posX: 400,
    posY: 100,
    version: 1,
    status: "synced",
    activeSubState: "Default",
    nodeType: "screen",
  };

  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("exports FLOATING_CHAT_LAYOUT_STORAGE_KEY and dimensional clamping constants", () => {
    expect(FLOATING_CHAT_LAYOUT_STORAGE_KEY).toBe("cowok:floating-chat:layout:v1");
    expect(MIN_WIDTH).toBe(320);
    expect(MAX_WIDTH).toBe(640);
    expect(MIN_HEIGHT).toBe(280);
    expect(MAX_HEIGHT).toBe(680);
    expect(getEffectiveMaxHeight()).toBeLessThanOrEqual(MAX_HEIGHT);
  });

  it("follows anchorX/anchorY props when user has NOT dragged (hasCustomPosition is false)", () => {
    const { rerender } = render(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeA]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={150}
        anchorY={220}
      />
    );

    const chatWin = screen.getByTestId("contextual-node-floating-chat");
    expect(chatWin.style.left).toBe("150px");
    expect(chatWin.style.top).toBe("220px");

    // Switching node or updating anchor props moves position because hasCustomPosition is false
    rerender(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeB]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={380}
        anchorY={410}
      />
    );

    expect(chatWin.style.left).toBe("380px");
    expect(chatWin.style.top).toBe("410px");
  });

  it("persists position to localStorage on drag, prevents anchor props from overwriting, and restores on remount", () => {
    const { unmount, rerender } = render(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeA]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={100}
        anchorY={100}
      />
    );

    const header = document.querySelector(".fc-drag-header");
    expect(header).toBeTruthy();

    // Simulate drag start and move within viewport bounds (maxY = 768 - 480 = 288)
    fireEvent.mouseDown(header!, { clientX: 100, clientY: 100 });
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 250, clientY: 200 }));
      window.dispatchEvent(new MouseEvent("mouseup", { clientX: 250, clientY: 200 }));
    });

    const chatWin = screen.getByTestId("contextual-node-floating-chat");
    expect(chatWin.style.left).toBe("250px");
    expect(chatWin.style.top).toBe("200px");

    // Verify localStorage was updated with hasCustomPosition: true
    const savedRaw = localStorage.getItem(FLOATING_CHAT_LAYOUT_STORAGE_KEY);
    expect(savedRaw).toBeTruthy();
    const saved = JSON.parse(savedRaw!);
    expect(saved.x).toBe(250);
    expect(saved.y).toBe(200);
    expect(saved.hasCustomPosition).toBe(true);

    // Rerender with different anchor props: position MUST NOT change
    rerender(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeB]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={999}
        anchorY={888}
      />
    );
    expect(chatWin.style.left).toBe("250px");
    expect(chatWin.style.top).toBe("200px");

    // Unmount and remount with different anchor props: restores saved position from localStorage
    unmount();

    render(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeB]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={50}
        anchorY={50}
      />
    );

    const remountedWin = screen.getByTestId("contextual-node-floating-chat");
    expect(remountedWin.style.left).toBe("250px");
    expect(remountedWin.style.top).toBe("200px");
  });

  it("supports horizontal, vertical, and corner resize with strict bounds clamping and localStorage persistence", () => {
    render(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeA]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
        anchorX={100}
        anchorY={100}
      />
    );

    const chatWin = screen.getByTestId("contextual-node-floating-chat");
    expect(chatWin.style.maxWidth).toBe(`${MAX_WIDTH}px`);
    expect(chatWin.style.maxHeight).toBe(`${getEffectiveMaxHeight()}px`);

    // 1. Horizontal resize handle (resize-e)
    const resizeE = screen.getByTestId("floating-chat-resize-e");
    expect(resizeE).toBeDefined();

    // Drag right edge far beyond MAX_WIDTH -> must clamp to MAX_WIDTH (640)
    fireEvent.mouseDown(resizeE, { clientX: 200, clientY: 200 });
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 1200, clientY: 200 }));
      window.dispatchEvent(new MouseEvent("mouseup", { clientX: 1200, clientY: 200 }));
    });

    expect(chatWin.style.width).toBe(`${MAX_WIDTH}px`);

    // 2. Vertical resize handle (resize-s)
    const resizeS = screen.getByTestId("floating-chat-resize-s");
    expect(resizeS).toBeDefined();

    // Drag bottom edge far beyond MAX_HEIGHT -> must clamp to getEffectiveMaxHeight() (<= 680)
    fireEvent.mouseDown(resizeS, { clientX: 200, clientY: 200 });
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 200, clientY: 1500 }));
      window.dispatchEvent(new MouseEvent("mouseup", { clientX: 200, clientY: 1500 }));
    });

    const effMaxH = getEffectiveMaxHeight();
    expect(chatWin.style.height).toBe(`${effMaxH}px`);

    // 3. Corner resize handle (resize-se)
    const resizeSE = screen.getByTestId("floating-chat-resize-se");
    expect(resizeSE).toBeDefined();

    // Drag corner down and right
    fireEvent.mouseDown(resizeSE, { clientX: 500, clientY: 500 });
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 400, clientY: 400 }));
      window.dispatchEvent(new MouseEvent("mouseup", { clientX: 400, clientY: 400 }));
    });

    // Check localStorage persistence
    const saved = JSON.parse(localStorage.getItem(FLOATING_CHAT_LAYOUT_STORAGE_KEY)!);
    expect(saved.width).toBeGreaterThanOrEqual(MIN_WIDTH);
    expect(saved.width).toBeLessThanOrEqual(MAX_WIDTH);
    expect(saved.height).toBeGreaterThanOrEqual(MIN_HEIGHT);
    expect(saved.height).toBeLessThanOrEqual(MAX_HEIGHT);
  });

  it("renders canonical ai-agent icon (zero sparkles icons in floating chat)", () => {
    const { container } = render(
      <ContextualNodeFloatingChat
        projectId="11111111-1111-4111-8111-111111111111"
        sessionId="session-1"
        selectedNodes={[mockNodeA]}
        onDeselectNode={vi.fn()}
        onClearSelection={vi.fn()}
        defaultExpanded
      />
    );

    // Verify canonical ai-agent icon is present
    const aiAgentIcons = container.querySelectorAll('[data-icon-name="ai-agent"]');
    expect(aiAgentIcons.length).toBeGreaterThanOrEqual(1);

    // Verify ZERO sparkles icons are rendered
    const sparklesIcons = container.querySelectorAll('[data-icon-name="sparkles"], .icon-sparkles');
    expect(sparklesIcons.length).toBe(0);
  });

  it("formats structured payload messages without raw JSON blobs", () => {
    const t = (_key: string, fallback: string) => fallback;

    // 1. Raw json payload in assistant message
    const rawJson = JSON.stringify({ prompt: "Tạo form thanh toán", status: "running", fencingToken: 3 });
    const formatted = formatStructuredPayload(rawJson, t);
    expect(formatted).not.toContain("{");
    expect(formatted).not.toContain("}");
    expect(formatted).toContain("Tạo form thanh toán");
    expect(formatted).toContain("Status: running");
    expect(formatted).toContain("#3");

    // 2. Prefixed event payload [started] {...}
    const prefixed = `[started] ${JSON.stringify({ prompt: "Phân tích DAG" })}`;
    const formattedPrefixed = formatStructuredPayload(prefixed, t);
    expect(formattedPrefixed).toContain("[started]");
    expect(formattedPrefixed).not.toContain("{");
    expect(formattedPrefixed).toContain("Phân tích DAG");

    // 3. Plain text payload is preserved
    const plainText = "[chunk] message-chunk-123";
    expect(formatStructuredPayload(plainText, t)).toBe(plainText);
  });

  it("renders NodeQueueBadge with localized formatting and proper testid", () => {
    const { rerender } = render(
      <NodeQueueBadge status="queued" queuePosition={2} fencingToken={4} />
    );

    const queuedBadge = screen.getByTestId("node-queue-badge");
    expect(queuedBadge.textContent).toContain("#2");
    expect(queuedBadge.textContent).toContain("#4");

    rerender(<NodeQueueBadge status="running" fencingToken={5} />);
    const runningBadge = screen.getByTestId("node-running-badge");
    expect(runningBadge.textContent).toMatch(/(RUNNING|ĐANG CHẠY)/);
    expect(runningBadge.textContent).toContain("#5");

    rerender(<NodeQueueBadge status="interrupted" />);
    const interruptedBadge = screen.getByTestId("node-interrupted-badge");
    expect(interruptedBadge.textContent).toContain("INTERRUPTED");
  });
});
