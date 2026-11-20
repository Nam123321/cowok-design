// @story 06.4
// @implements AC-03 (Auto-collapse Left CMS Structure Pane on Inline Split Fallback and restore on close)
import { useEffect, useRef } from "react";

export function usePresenterInlineCollapse(
  isInlineFallbackOpen: boolean,
  isLeftStructureHidden: boolean,
  setIsLeftStructureHidden: (hidden: boolean) => void
): void {
  const isLeftHiddenRef = useRef(isLeftStructureHidden);
  isLeftHiddenRef.current = isLeftStructureHidden;
  const setLeftHiddenRef = useRef(setIsLeftStructureHidden);
  setLeftHiddenRef.current = setIsLeftStructureHidden;
  const wasLeftOpenRef = useRef(false);
  const prevFallbackRef = useRef(isInlineFallbackOpen);

  useEffect(() => {
    const wasOpen = prevFallbackRef.current;
    prevFallbackRef.current = isInlineFallbackOpen;
    if (wasOpen !== isInlineFallbackOpen && typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("cowok:presenter-inline-fallback", { detail: { open: isInlineFallbackOpen } }));
    }
    if (!wasOpen && isInlineFallbackOpen) {
      if (!isLeftHiddenRef.current) {
        wasLeftOpenRef.current = true;
        setLeftHiddenRef.current(true);
      }
      return;
    }
    if (wasOpen && !isInlineFallbackOpen && wasLeftOpenRef.current) {
      wasLeftOpenRef.current = false;
      setLeftHiddenRef.current(false);
    }
  }, [isInlineFallbackOpen]);

  useEffect(() => {
    return () => {
      if (prevFallbackRef.current && typeof window !== "undefined") {
        window.dispatchEvent(new CustomEvent("cowok:presenter-inline-fallback", { detail: { open: false } }));
      }
      if (wasLeftOpenRef.current) {
        wasLeftOpenRef.current = false;
        setLeftHiddenRef.current(false);
      }
    };
  }, []);
}
