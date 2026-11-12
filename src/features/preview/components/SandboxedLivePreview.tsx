import React, { useRef } from "react";
import {
  FixtureVariant,
  ViewportPreset,
} from "../../../lib/validations/screen-fixture";
import { usePreviewPostMessage } from "../hooks/usePreviewPostMessage";
import { buildSandboxSrcDoc } from "../utils/sandbox-template";
import { PreviewStageContainer } from "./PreviewStageContainer";

const JSON_INDENT = 2;

interface SandboxedLivePreviewProps {
  nodeId: string;
  screenKey: string;
  title: string;
  routePath: string;
  viewportPreset: ViewportPreset;
  variant: FixtureVariant;
  fixturePayload?: Record<string, string | number | boolean | null>;
}

export const SandboxedLivePreview: React.FC<SandboxedLivePreviewProps> = ({
  nodeId,
  screenKey,
  title,
  routePath,
  viewportPreset,
  variant,
  fixturePayload = {},
}) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const loadError = usePreviewPostMessage({
    iframeRef,
    nodeId,
    screenKey,
    routePath,
    viewportPreset,
    variant,
    fixturePayload,
  });

  const payloadStr = JSON.stringify(fixturePayload, null, JSON_INDENT);
  const srcDoc = buildSandboxSrcDoc(screenKey, title, variant, payloadStr);

  return (
    <div className="relative w-full overflow-hidden flex flex-col items-center justify-center p-4 bg-muted/20 border border-border rounded-xl">
      {loadError && (
        <div className="absolute top-2 left-2 right-2 p-2 bg-destructive/10 text-destructive text-xs rounded border border-destructive/20 z-10">
          Hydration Warning: {loadError}
        </div>
      )}
      <PreviewStageContainer
        iframeRef={iframeRef}
        title={title}
        srcDoc={srcDoc}
        viewportPreset={viewportPreset}
      />
    </div>
  );
};
