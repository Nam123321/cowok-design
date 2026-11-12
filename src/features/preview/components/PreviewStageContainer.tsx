import React, { RefObject } from "react";
import { ViewportPreset } from "../../../lib/validations/screen-fixture";
import { VIEWPORT_PRESETS } from "./ViewportSwitcher";

const DEFAULT_STAGE_HEIGHT = 700;
const SANDBOX_PERMISSIONS = "allow-scripts allow-forms";

interface PreviewStageContainerProps {
  iframeRef: RefObject<HTMLIFrameElement | null>;
  title: string;
  srcDoc: string;
  viewportPreset: ViewportPreset;
}

export const PreviewStageContainer: React.FC<PreviewStageContainerProps> = ({
  iframeRef,
  title,
  srcDoc,
  viewportPreset,
}) => {
  const vpConfig = VIEWPORT_PRESETS[viewportPreset];

  return (
    <div
      className="transition-all duration-300 origin-top flex items-center justify-center shadow-2xl rounded-lg overflow-hidden border border-border/80"
      style={{
        width: `${vpConfig.width}px`,
        height: `${vpConfig.height}px`,
        transform: `scale(${vpConfig.scale})`,
        transformOrigin: "top center",
        maxHeight: `${DEFAULT_STAGE_HEIGHT}px`,
      }}
    >
      <iframe
        ref={iframeRef}
        title={`Sandboxed Preview: ${title}`}
        srcDoc={srcDoc}
        sandbox={SANDBOX_PERMISSIONS}
        className="w-full h-full border-0 bg-background"
      />
    </div>
  );
};
