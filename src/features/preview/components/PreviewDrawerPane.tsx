import React, { useState } from "react";
import {
  FixtureVariant,
  ViewportPreset,
} from "../../../lib/validations/screen-fixture";
import { FixtureVariantSelector } from "./FixtureVariantSelector";
import { SandboxedLivePreview } from "./SandboxedLivePreview";
import { ViewportSwitcher } from "./ViewportSwitcher";

interface PreviewDrawerPaneProps {
  nodeId: string;
  screenKey: string;
  title: string;
  routePath: string;
  initialPreset?: ViewportPreset;
  initialVariant?: FixtureVariant;
}

export const PreviewDrawerPane: React.FC<PreviewDrawerPaneProps> = ({
  nodeId,
  screenKey,
  title,
  routePath,
  initialPreset = "desktop-1440",
  initialVariant = "DEFAULT",
}) => {
  const [viewportPreset, setViewportPreset] = useState<ViewportPreset>(initialPreset);
  const [variant, setVariant] = useState<FixtureVariant>(initialVariant);

  return (
    <div className="flex flex-col gap-4 w-full">
      <div className="flex flex-wrap items-center justify-between gap-2 p-2 bg-muted/40 rounded-lg border border-border">
        <ViewportSwitcher
          currentPreset={viewportPreset}
          onPresetChange={setViewportPreset}
        />
        <FixtureVariantSelector
          currentVariant={variant}
          onVariantChange={setVariant}
        />
      </div>

      <SandboxedLivePreview
        nodeId={nodeId}
        screenKey={screenKey}
        title={title}
        routePath={routePath}
        viewportPreset={viewportPreset}
        variant={variant}
      />
    </div>
  );
};
