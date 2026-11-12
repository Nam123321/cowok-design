import { useEffect, useState, RefObject } from "react";
import {
  FixtureVariant,
  ViewportPreset,
} from "../../../lib/validations/screen-fixture";
import { MESSAGE_TYPE } from "../utils/sandbox-template";
import { VIEWPORT_PRESETS } from "../components/ViewportSwitcher";

interface UsePreviewPostMessageParams {
  iframeRef: RefObject<HTMLIFrameElement | null>;
  nodeId: string;
  screenKey: string;
  routePath: string;
  viewportPreset: ViewportPreset;
  variant: FixtureVariant;
  fixturePayload: Record<string, string | number | boolean | null>;
}

export function usePreviewPostMessage({
  iframeRef,
  nodeId,
  screenKey,
  routePath,
  viewportPreset,
  variant,
  fixturePayload,
}: UsePreviewPostMessageParams): string | null {
  const [loadError, setLoadError] = useState<string | null>(null);
  const vpConfig = VIEWPORT_PRESETS[viewportPreset];

  useEffect(() => {
    try {
      setLoadError(null);
      if (iframeRef.current?.contentWindow) {
        iframeRef.current.contentWindow.postMessage(
          {
            type: MESSAGE_TYPE,
            nodeId,
            screenKey,
            routePath,
            variant,
            payload: fixturePayload,
            viewport: {
              preset: viewportPreset,
              width: vpConfig.width,
              height: vpConfig.height,
            },
          },
          "*"
        );
      }
    } catch (err) {
      setLoadError(err instanceof Error ? err.message : "Hydration error");
    }
  }, [iframeRef, nodeId, screenKey, routePath, viewportPreset, variant, fixturePayload, vpConfig]);

  return loadError;
}
