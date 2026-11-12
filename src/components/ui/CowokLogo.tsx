// cowok-design — Canonical Official Brand Mark & Logo (DESIGN.md v4.3-OS & LOGO_GUIDELINE.md)
import React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import horizontalLight from "@/assets/logo/01-horizontal/cowok-logo-horizontal-light-no-tagline.png";
import horizontalDark from "@/assets/logo/01-horizontal/cowok-logo-horizontal-dark-no-tagline.png";
import symbolColor from "@/assets/logo/03-symbol/cowok-symbol-color-2048.png";
import wordmarkNavy from "@/assets/logo/04-wordmark/cowok-wordmark-navy-4096.png";
import wordmarkWhite from "@/assets/logo/04-wordmark/cowok-wordmark-white-4096.png";

export type LogoVariant = "horizontal" | "symbol" | "wordmark" | "lockup-badge";

export interface CowokLogoProps {
  variant?: LogoVariant;
  theme?: "warm" | "dim" | "night" | "monochrome" | "night-low" | string;
  size?: number | string;
  height?: number | string;
  className?: string;
  style?: React.CSSProperties;
  badgeText?: string;
}

/**
 * Option 3 Refined — Human + Agent Canonical Vector Symbol
 * Left Node (Human Blue #0057FF) + Right Node (Agent Orange #FF9A2F)
 * Dynamic Bridge (#0057FF -> #FF9A2F) + Shared Center Node
 */
export const CowokSymbolVector: React.FC<{
  size?: number | string;
  className?: string;
  style?: React.CSSProperties;
}> = ({ size = 24, className = "", style = {} }) => (
  <svg
    viewBox="0 0 48 48"
    width={size}
    height={size}
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={`cowok-symbol-vector ${className}`}
    style={{ display: "inline-block", verticalAlign: "middle", flexShrink: 0, ...style }}
    aria-label="cowok brand symbol"
  >
    <defs>
      <linearGradient id="cowokBridgeGrad" x1="12" y1="24" x2="36" y2="24" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stopColor="#0057FF" />
        <stop offset="50%" stopColor="#00D4FF" />
        <stop offset="100%" stopColor="#FF9A2F" />
      </linearGradient>
    </defs>
    {/* Asymmetric Dynamic Bridge */}
    <path
      d="M14 24C18 16 30 16 34 24C30 32 18 32 14 24Z"
      stroke="url(#cowokBridgeGrad)"
      strokeWidth="3.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      opacity="0.85"
    />
    {/* Left Human Node */}
    <circle cx="12" cy="24" r="5.5" fill="#0057FF" />
    <circle cx="12" cy="24" r="2.5" fill="#FFFFFF" />
    {/* Right Agent Node */}
    <circle cx="36" cy="24" r="5.5" fill="#FF9A2F" />
    <circle cx="36" cy="24" r="2.5" fill="#FFFFFF" />
    {/* Shared Center State Node */}
    <circle cx="24" cy="24" r="3.2" fill="#081E3A" stroke="#FFFFFF" strokeWidth="1.5" />
  </svg>
);

export const CowokLogo: React.FC<CowokLogoProps> = ({
  variant = "horizontal",
  theme = "warm",
  size,
  height,
  className = "",
  style = {},
  badgeText = "design",
}) => {
  const isDark = theme === "night" || theme === "night-low" || theme === "monochrome";

  if (variant === "symbol") {
    const pxSize = size || height || 24;
    return (
      <img
        src={symbolColor}
        alt="cowok symbol"
        width={pxSize}
        height={pxSize}
        className={`cowok-logo cowok-logo-symbol ${className}`}
        style={{
          display: "inline-block",
          objectFit: "contain",
          verticalAlign: "middle",
          flexShrink: 0,
          imageRendering: "-webkit-optimize-contrast",
          ...style,
        }}
      />
    );
  }

  if (variant === "wordmark") {
    const pxHeight = height || size || 20;
    return (
      <img
        src={isDark ? wordmarkWhite : wordmarkNavy}
        alt="cowok wordmark"
        height={pxHeight}
        className={`cowok-logo-wordmark ${className}`}
        style={{
          display: "inline-block",
          objectFit: "contain",
          verticalAlign: "middle",
          height: pxHeight,
          imageRendering: "-webkit-optimize-contrast",
          ...style,
        }}
      />
    );
  }

  if (variant === "lockup-badge") {
    const h = height || size || 24;
    return (
      <div
        className={`cowok-lockup-badge ${className}`}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: "8px",
          lineHeight: 1,
          ...style,
        }}
      >
        <img
          src={symbolColor}
          alt="cowok symbol"
          height={h}
          width={h}
          style={{
            objectFit: "contain",
            imageRendering: "-webkit-optimize-contrast",
            flexShrink: 0,
          }}
        />
        <div style={{ display: "inline-flex", alignItems: "baseline", gap: "4px" }}>
          <span
            style={{
              fontFamily: "'Geist', sans-serif",
              fontWeight: 650,
              fontSize: "15px",
              letterSpacing: "-0.032em",
              color: "var(--cw-text-primary)",
            }}
          >
            cowok
          </span>
          {badgeText && (
            <span
              style={{
                fontFamily: "'Geist Mono', monospace",
                fontWeight: 600,
                fontSize: "10px",
                letterSpacing: "+0.02em",
                color: "var(--human-accent)",
                backgroundColor: "var(--human-soft)",
                padding: "1px 5px",
                borderRadius: "4px",
                border: "1px solid var(--human-border)",
              }}
            >
              -{badgeText}
            </span>
          )}
        </div>
      </div>
    );
  }

  // Default: variant === "horizontal"
  const pxHeight = height || size || 26;
  const src = isDark ? horizontalDark : horizontalLight;

  return (
    <img
      src={src}
      alt="cowok logo"
      height={pxHeight}
      className={`cowok-logo cowok-logo-horizontal ${className}`}
      style={{
        display: "inline-block",
        objectFit: "contain",
        verticalAlign: "middle",
        height: pxHeight,
        imageRendering: "-webkit-optimize-contrast",
        ...style,
      }}
    />
  );
};
