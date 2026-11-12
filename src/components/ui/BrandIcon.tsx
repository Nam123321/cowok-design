// @story 04.1
// cowok-design — Canonical Brand Icon Registry (DESIGN.md v4.3-OS & Iconography v2)
import React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import { COWOK_ICON_REGISTRY, type CowokIconProps, WorkflowIcon } from "./icons";
import { SPECIAL_ICON_RENDERERS } from "./icons/special-renderers";

export type { CowokIconProps };
export const useBrandIconTranslation = useTranslation;

export interface BrandIconProps {
  name: string;
  size?: number | string;
  className?: string;
  color?: string;
  strokeWidth?: number;
  "aria-label"?: string;
  title?: string;
  style?: React.CSSProperties;
}

const DEFAULT_ICON_SIZE = 20;
const DEFAULT_STROKE_WIDTH = 1.8;

export const BrandIcon: React.FC<BrandIconProps> = (props) => {
  const {
    name,
    size = DEFAULT_ICON_SIZE,
    className = "",
    color = "currentColor",
    strokeWidth = DEFAULT_STROKE_WIDTH,
    title,
    style = {},
  } = props;
  const normalizedKey = name.toLowerCase().trim().replace(/^icon-/, "");
  const numSize = typeof size === "number" ? size : parseInt(String(size), 10) || DEFAULT_ICON_SIZE;
  const resolvedLabel = props["aria-label"] || title || name;

  if (SPECIAL_ICON_RENDERERS[normalizedKey]) {
    return (
      <span
        className={`brand-icon icon-${normalizedKey} ${className}`}
        style={{ display: "inline-flex", alignItems: "center", justifyContent: "center", color, ...style }}
        aria-label={resolvedLabel}
        title={title}
      >
        {SPECIAL_ICON_RENDERERS[normalizedKey]({ size: numSize, strokeWidth })}
      </span>
    );
  }

  const Component = COWOK_ICON_REGISTRY[normalizedKey] || WorkflowIcon;
  const iconClass = COWOK_ICON_REGISTRY[normalizedKey] ? `icon-${normalizedKey}` : "icon-fallback";

  return (
    <Component
      size={numSize}
      strokeWidth={strokeWidth}
      className={`brand-icon ${iconClass} ${className}`}
      style={{ color, display: "inline-block", verticalAlign: "middle", ...style }}
      aria-label={resolvedLabel}
    />
  );
};
