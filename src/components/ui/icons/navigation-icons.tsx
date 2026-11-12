// @story 04.1
import * as React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CowokIconProps } from "./core-icons";

const base = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const useNavIconTranslation = useTranslation;

export function EyeIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" /><circle cx="12" cy="12" r="3" /></svg>;
}

export function EyeOffIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" /><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" /><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" /><path d="m2 2 20 20" /></svg>;
}

export function SunIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" /></svg>;
}

export function MoonIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401" /></svg>;
}

export function ReplyIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><polyline points="9 17 4 12 9 7" /><path d="M20 18v-2a4 4 0 0 0-4-4H4" /></svg>;
}

export function RefreshIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /></svg>;
}

export function SwitchIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="m16 3 4 4-4 4M20 7H4m4 14-4-4 4-4M4 17h16" /></svg>;
}

export function ChevronDownIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="m6 9 6 6 6-6" /></svg>;
}

export function ChevronRightIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="m9 18 6-6-6-6" /></svg>;
}

export function PanelLeftCloseIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3" y="3" width="18" height="18" rx="2" ry="2" /><line x1="9" y1="3" x2="9" y2="21" /><path d="m14 9-3 3 3 3" /></svg>;
}

export function PanelRightIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3" y="3" width="18" height="18" rx="2" ry="2" /><line x1="15" y1="3" x2="15" y2="21" /></svg>;
}

export function TerminalIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="m4 17 6-6-6-6" /><line x1="12" y1="19" x2="20" y2="19" /></svg>;
}

export function LayersIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M12 2L2 7l10 5 10-5-10-5Z" /><path d="M2 17l10 5 10-5M2 12l10 5 10-5" /></svg>;
}

export function XIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M18 6 6 18M6 6l12 12" /></svg>;
}

export function ArrowDownIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><line x1="12" y1="5" x2="12" y2="19" /><polyline points="19 12 12 19 5 12" /></svg>;
}

export function ExternalLinkIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" /></svg>;
}

export const NAV_ICON_COMPONENTS = {
  eye: EyeIcon,
  preview: EyeIcon,
  "eye-off": EyeOffIcon,
  sun: SunIcon,
  moon: MoonIcon,
  undo: ReplyIcon,
  redo: RefreshIcon,
  refresh: RefreshIcon,
  reply: ReplyIcon,
  switch: SwitchIcon,
  "chevron-down": ChevronDownIcon,
  "chevron-right": ChevronRightIcon,
  "panel-close": PanelLeftCloseIcon,
  "panel-left-close": PanelLeftCloseIcon,
  "panel-right": PanelRightIcon,
  terminal: TerminalIcon,
  code: TerminalIcon,
  layers: LayersIcon,
  presentation: LayersIcon,
  close: XIcon,
  x: XIcon,
  "arrow-down": ArrowDownIcon,
  "external-link": ExternalLinkIcon,
};
