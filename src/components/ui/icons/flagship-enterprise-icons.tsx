// @story 04.1
// Domain path glyphs split from flagship-domain-icons.tsx for modular icon registry cleanliness
import type * as React from "react";
import type { CowokIconProps } from "./core-icons";

const svgBase = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

function makePathIcon(paths: readonly string[]): React.ComponentType<CowokIconProps> {
  return function FlagshipPathIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
    return (
      <svg viewBox="0 0 24 24" width={size} height={size} {...svgBase} strokeWidth={strokeWidth} {...props}>
        {paths.map((d, idx) => (
          <path key={idx} d={d} />
        ))}
      </svg>
    );
  };
}

export function makeAgentPersonaIcon(badgeChar: string): React.ComponentType<CowokIconProps> {
  return function FlagshipAgentIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
    return (
      <svg viewBox="0 0 24 24" width={size} height={size} {...svgBase} strokeWidth={strokeWidth} {...props}>
        <rect x="4" y="4" width="16" height="16" rx="4" />
        <text x="12" y="15.5" textAnchor="middle" fontSize="9" fontWeight="600" fill="currentColor" stroke="none">
          {badgeChar}
        </text>
      </svg>
    );
  };
}

export const DatabaseGlyph = makePathIcon(["M12 3c4.4 0 8 1.3 8 3s-3.6 3-8 3-8-1.3-8-3 3.6-3 8-3Z", "M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6", "M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"]);
export const FileGlyph = makePathIcon(["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z", "M14 2v6h6"]);
export const FileTextGlyph = makePathIcon(["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z", "M14 2v6h6", "M8 13h8", "M8 17h6"]);
export const AnalyticsGlyph = makePathIcon(["M3 3v18h18", "M7 15v-4", "M12 15V7", "M17 15v-6"]);
export const TrendGlyph = makePathIcon(["M3 17l6-6 4 4 8-8", "M15 7h6v6"]);
export const BoxGlyph = makePathIcon(["M12 2l8 4.5v11L12 22l-8-4.5v-11L12 2Z", "M12 12l8-4.5", "M12 12v10", "M12 12L4 7.5"]);
export const LinkGlyph = makePathIcon(["M10 13a5 5 0 0 0 7.5.5l2-2a5 5 0 0 0-7-7l-1.5 1.5", "M14 11a5 5 0 0 0-7.5-.5l-2 2a5 5 0 0 0 7 7l1.5-1.5"]);
export const KeyGlyph = makePathIcon(["M15 7a4 4 0 1 1-7.5 2l-4.5 4.5V17h3.5v-2H9v-2.5"]);
export const ServerGlyph = makePathIcon(["M4 4h16v6H4z", "M4 14h16v6H4z", "M8 7h.01", "M8 17h.01"]);
export const DownloadGlyph = makePathIcon(["M12 3v12", "M7 10l5 5 5-5", "M4 19h16"]);
export const UploadGlyph = makePathIcon(["M12 15V3", "M7 8l5-5 5 5", "M4 19h16"]);
export const DashboardGlyph = makePathIcon(["M3 3h7v7H3z", "M14 3h7v4h-7z", "M14 11h7v10h-7z", "M3 14h7v7H3z"]);
export const KanbanGlyph = makePathIcon(["M4 4h4v14H4z", "M10 4h4v9h-4z", "M16 4h4v16h-4z"]);
export const HashGlyph = makePathIcon(["M4 9h16", "M4 15h16", "M10 3L8 21", "M16 3l-2 18"]);
export const MonitorGlyph = makePathIcon(["M3 4h18v12H3z", "M8 20h8", "M12 16v4"]);
export const LightbulbGlyph = makePathIcon(["M9 18h6", "M10 21h4", "M12 3a6 6 0 0 0-4 10.5c.8.8 1.2 1.7 1.3 2.5h5.4c.1-.8.5-1.7 1.3-2.5A6 6 0 0 0 12 3Z"]);
export const MoreHorizGlyph = makePathIcon(["M5 12h.01", "M12 12h.01", "M19 12h.01"]);
export const ForwardGlyph = makePathIcon(["M15 10l5 5-5 5", "M4 4v7a4 4 0 0 0 4 4h12"]);
export const LogOutGlyph = makePathIcon(["M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4", "M16 17l5-5-5-5", "M21 12H9"]);
export const TaskGlyph = makePathIcon(["M9 11l2 2 4-4", "M4 5h16v14H4z"]);
export const DataGridGlyph = makePathIcon(["M3 3h18v18H3z", "M3 9h18", "M3 15h18", "M9 3v18", "M15 3v18"]);
export const ImageGlyph = makePathIcon(["M3 4h18v16H3z", "M8 10a1.5 1.5 0 1 0 0 .01", "M21 16l-5-5-6 6"]);
export const SmileGlyph = makePathIcon(["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z", "M8 14s1.5 2 4 2 4-2 4-2", "M9 9h.01", "M15 9h.01"]);
