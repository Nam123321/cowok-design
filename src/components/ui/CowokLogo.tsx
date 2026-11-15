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
const DEFAULT_SYMBOL_SIZE = 24;
const DEFAULT_WORDMARK_HEIGHT = 20;
const DEFAULT_HORIZONTAL_HEIGHT = 26;
const LOCKUP_FONT_WEIGHT = 600;

export const CowokSymbolVector: React.FC<{
  size?: number | string;
  className?: string;
  style?: React.CSSProperties;
}> = ({ size = DEFAULT_SYMBOL_SIZE, className = "", style = {} }) => {
  const gradId = React.useId().replace(/:/g, "");
  return (
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
        <linearGradient id={gradId} x1="12" y1="24" x2="36" y2="24" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="var(--human-accent, #0057FF)" />
          <stop offset="50%" stopColor="var(--accent-cyan, #00D4FF)" />
          <stop offset="100%" stopColor="var(--agent-accent, #FF9A2F)" />
        </linearGradient>
      </defs>
      {/* Asymmetric Dynamic Bridge */}
      <path
        d="M14 24C18 16 30 16 34 24C30 32 18 32 14 24Z"
        stroke={`url(#${gradId})`}
        strokeWidth="3.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity="0.85"
      />
      {/* Left Human Node */}
      <circle cx="12" cy="24" r="5.5" fill="var(--human-accent, #0057FF)" />
      <circle cx="12" cy="24" r="2.5" fill="var(--text-inverse, #FFFFFF)" />
      {/* Right Agent Node */}
      <circle cx="36" cy="24" r="5.5" fill="var(--agent-accent, #FF9A2F)" />
      <circle cx="36" cy="24" r="2.5" fill="var(--text-inverse, #FFFFFF)" />
      {/* Shared Center State Node */}
      <circle cx="24" cy="24" r="3.2" fill="var(--cw-navy-deep, #081E3A)" stroke="var(--text-inverse, #FFFFFF)" strokeWidth="1.5" />
    </svg>
  );
};

const BASE_IMG_STYLE: React.CSSProperties = {
  display: "inline-block",
  objectFit: "contain",
  verticalAlign: "middle",
  flexShrink: 0,
  imageRendering: "-webkit-optimize-contrast",
};

const LOCKUP_BADGE_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  fontWeight: LOCKUP_FONT_WEIGHT,
  fontSize: "12px",
  letterSpacing: "+0.02em",
  color: "var(--human-accent)",
  backgroundColor: "var(--human-soft)",
  padding: "1px 5px",
  borderRadius: "4px",
  border: "1px solid var(--human-border)",
};

const LockupBadgeLogo: React.FC<{
  height: number | string;
  className: string;
  style: React.CSSProperties;
  badgeText: string;
}> = ({ height, className, style, badgeText }) => (
  <div
    className={`cowok-lockup-badge ${className}`}
    style={{ display: "inline-flex", alignItems: "center", gap: "8px", lineHeight: 1, ...style }}
  >
    <img src={symbolColor} alt="cowok symbol" height={height} width={height} style={BASE_IMG_STYLE} />
    <div style={{ display: "inline-flex", alignItems: "baseline", gap: "4px" }}>
      <span style={{ fontFamily: "var(--font-sans)", fontWeight: LOCKUP_FONT_WEIGHT, fontSize: "15px", letterSpacing: "-0.032em", color: "var(--cw-text-primary)" }}>
        cowok
      </span>
      {badgeText && <span style={LOCKUP_BADGE_STYLE}>-{badgeText}</span>}
    </div>
  </div>
);

function resolveBrandAsset(variant: LogoVariant, isDark: boolean, size?: number | string, height?: number | string) {
  if (variant === "wordmark") {
    return {
      pxHeight: height || size || DEFAULT_WORDMARK_HEIGHT,
      src: isDark ? wordmarkWhite : wordmarkNavy,
      alt: "cowok wordmark",
      variantClass: "cowok-logo-wordmark",
    };
  }
  return {
    pxHeight: height || size || DEFAULT_HORIZONTAL_HEIGHT,
    src: isDark ? horizontalDark : horizontalLight,
    alt: "cowok logo",
    variantClass: "cowok-logo cowok-logo-horizontal",
  };
}

export const CowokLogo: React.FC<CowokLogoProps> = ({
  variant = "horizontal",
  theme = "warm",
  size,
  height,
  className = "",
  style = {},
  badgeText = "design",
}) => {
  const { t } = useTranslation();
  const isDark = theme === "night" || theme === "night-low" || theme === "monochrome";

  if (variant === "lockup-badge") {
    return <LockupBadgeLogo height={height || size || DEFAULT_SYMBOL_SIZE} className={className} style={style} badgeText={badgeText} />;
  }
  if (variant === "symbol") {
    const pxSize = size || height || DEFAULT_SYMBOL_SIZE;
    return <img src={symbolColor} alt={t("brand.symbolAlt", "cowok symbol")} width={pxSize} height={pxSize} className={`cowok-logo cowok-logo-symbol ${className}`} style={{ ...BASE_IMG_STYLE, ...style }} />;
  }

  const { pxHeight, src, alt, variantClass } = resolveBrandAsset(variant, isDark, size, height);
  return <img src={src} alt={alt} height={pxHeight} className={`${variantClass} ${className}`} style={{ ...BASE_IMG_STYLE, height: pxHeight, ...style }} />;
};
