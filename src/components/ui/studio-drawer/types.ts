// @story 05.6, 06.5
// @implements [5.26] StudioDrawerShell, PAT-09 Focused Secondary Workbench, CMP-17
import type { ViewportPreset } from "../../../lib/validations/screen-fixture";

export type StudioPointerMode = "interact" | "select-region";
export type StudioSubState = "DEFAULT" | "EMPTY" | "LOADING" | "ERROR";
export type StudioViewportPreset = ViewportPreset;
export type StudioMobileTab = "control" | "canvas";
export type StudioSpatialCoreAction = "comment" | "edit" | "ask-subchat" | "inspect";

export interface StudioRegionRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface StudioPinnedComment {
  id: string;
  num: number;
  ffId: string;
  label: string;
  text: string;
  x: number;
  y: number;
  kind: "element" | "region";
  regionRect?: StudioRegionRect;
}

export interface StudioSubChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  kind?: "chat" | "comment";
  pinNumber?: number;
  scopeFfId?: string | null;
  syncedToMain?: boolean;
}

export interface StudioSpatialTarget {
  ffId: string;
  label?: string;
  x: number;
  y: number;
  kind?: "element" | "region";
  regionRect?: StudioRegionRect;
}

export interface StudioSpatialDomainAction {
  id: string;
  label: string;
  icon: string;
  onSelect: () => void;
}

export const STUDIO_DRAWER_DEFAULT_VW = 0.78;
export const STUDIO_DRAWER_MAX_VW = 0.85;
export const STUDIO_DRAWER_MOBILE_MAX_VW = 0.96;
export const STUDIO_DRAWER_MIN_PX = 420;
export const STUDIO_MASTER_DEFAULT_PX = 356;
export const STUDIO_MASTER_MIN_PX = 260;
export const STUDIO_MASTER_MAX_PX = 520;
export const STUDIO_SUBCHAT_DEFAULT_PX = 224;
export const STUDIO_SUBCHAT_MIN_PX = 148;
export const STUDIO_SUBCHAT_MAX_PX = 380;
export const STUDIO_SUBCHAT_COLLAPSED_PX = 34;
export const STUDIO_MOBILE_BREAKPOINT_PX = 768;

export const STUDIO_FONT_MEDIUM = 500;
export const STUDIO_FONT_SEMIBOLD = 600;
export const STUDIO_FONT_BOLD = 700;
export const STUDIO_ICON_XS = 11;
export const STUDIO_ICON_SM = 12;
export const STUDIO_ICON_MD = 14;
export const STUDIO_ICON_LG = 16;
export const STUDIO_STROKE_MONO = 1.85;
export const STUDIO_DRAWER_Z = 56;
export const STUDIO_RESIZER_Z = 20;
export const STUDIO_TOAST_Z = 62;
export const STUDIO_MARQUEE_MIN_PX = 16;
