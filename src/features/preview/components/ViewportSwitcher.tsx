import React from "react";
import { ViewportPreset } from "../../../lib/validations/screen-fixture";

export const DESKTOP_WIDTH = 1440;
export const DESKTOP_HEIGHT = 900;
export const DESKTOP_SCALE = 0.55;

export const TABLET_WIDTH = 768;
export const TABLET_HEIGHT = 1024;
export const TABLET_SCALE = 0.75;

export const MOBILE_WIDTH = 390;
export const MOBILE_HEIGHT = 844;
export const MOBILE_SCALE = 1.0;

export interface ViewportConfig {
  preset: ViewportPreset;
  label: string;
  width: number;
  height: number;
  scale: number;
}

export const VIEWPORT_PRESETS: Record<ViewportPreset, ViewportConfig> = {
  "desktop-1440": {
    preset: "desktop-1440",
    label: "Desktop",
    width: DESKTOP_WIDTH,
    height: DESKTOP_HEIGHT,
    scale: DESKTOP_SCALE,
  },
  "tablet-768": {
    preset: "tablet-768",
    label: "Tablet",
    width: TABLET_WIDTH,
    height: TABLET_HEIGHT,
    scale: TABLET_SCALE,
  },
  "mobile-390": {
    preset: "mobile-390",
    label: "Mobile",
    width: MOBILE_WIDTH,
    height: MOBILE_HEIGHT,
    scale: MOBILE_SCALE,
  },
};

interface ViewportSwitcherProps {
  currentPreset: ViewportPreset;
  onPresetChange: (preset: ViewportPreset) => void;
}

export const ViewportSwitcher: React.FC<ViewportSwitcherProps> = ({
  currentPreset,
  onPresetChange,
}) => {
  const activeConfig = VIEWPORT_PRESETS[currentPreset];

  return (
    <div className="flex items-center gap-2 bg-muted/60 p-1 rounded-lg border border-border">
      {(Object.keys(VIEWPORT_PRESETS) as ViewportPreset[]).map((preset) => {
        const config = VIEWPORT_PRESETS[preset];
        const isActive = currentPreset === preset;
        return (
          <button
            key={preset}
            type="button"
            onClick={() => onPresetChange(preset)}
            className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
              isActive
                ? "bg-background text-foreground shadow-sm font-semibold"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            {config.label}
          </button>
        );
      })}
      <span className="text-[10px] text-muted-foreground ml-1 font-mono">
        {activeConfig.width}x{activeConfig.height}
      </span>
    </div>
  );
};
