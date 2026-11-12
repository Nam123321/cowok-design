// @story 04.1
import { useState, useCallback, useEffect } from "react";
import {
  CHAT_DEFAULT_WIDTH,
  CHAT_COLLAPSED_RAIL_WIDTH,
  CHAT_MIN_WIDTH,
  CHAT_MAX_WIDTH,
  SIDEBAR_WIDTH,
} from "../constants";

function clampChatWidth(rawClientX: number): number {
  return Math.max(Math.min(rawClientX - SIDEBAR_WIDTH, CHAT_MAX_WIDTH), CHAT_MIN_WIDTH);
}

function useSplitterDragEffect(
  isDragging: boolean,
  onStopDrag: () => void,
  onResize: (clientX: number) => void
) {
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (isDragging) onResize(e.clientX);
    };
    const handleMouseUp = () => {
      if (isDragging) onStopDrag();
    };
    if (isDragging) {
      window.addEventListener("mousemove", handleMouseMove);
      window.addEventListener("mouseup", handleMouseUp);
    }
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging, onStopDrag, onResize]);
}

export function useWorkbenchSplitter(
  initialWidth: number = CHAT_DEFAULT_WIDTH,
  initialCollapsed: boolean = false
) {
  const [expandedWidth, setExpandedWidth] = useState(initialWidth);
  const [isChatShortened, setIsChatShortened] = useState(initialCollapsed);
  const [isDraggingSplitter, setIsDraggingSplitter] = useState(false);

  const chatWidth = isChatShortened ? CHAT_COLLAPSED_RAIL_WIDTH : expandedWidth;

  const handleToggleShortenChat = useCallback(() => {
    setIsChatShortened((prev) => !prev);
  }, []);

  const handleSplitterMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (isChatShortened) return;
      e.preventDefault();
      setIsDraggingSplitter(true);
    },
    [isChatShortened]
  );

  const handleResize = useCallback((clientX: number) => {
    const newWidth = clampChatWidth(clientX);
    setExpandedWidth(newWidth);
  }, []);

  const handleStopDrag = useCallback(() => setIsDraggingSplitter(false), []);

  useSplitterDragEffect(isDraggingSplitter, handleStopDrag, handleResize);

  return {
    chatWidth,
    isChatShortened,
    isDraggingSplitter,
    handleToggleShortenChat,
    handleSplitterMouseDown,
  };
}
