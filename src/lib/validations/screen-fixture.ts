import { z } from "zod";

export const CanonicalViewportPresetSchema = z.enum(["desktop-1440", "tablet-768", "mobile-390"]);
export const ViewportPresetSchema = z.preprocess((val) => {
  if (typeof val !== "string") return val;
  const v = val.trim().toLowerCase();
  if (!v || v === "desktop" || v === "desktop-1440") return "desktop-1440";
  if (v === "tablet" || v === "tablet-768") return "tablet-768";
  if (v === "mobile" || v === "mobile-390") return "mobile-390";
  return val.trim();
}, CanonicalViewportPresetSchema);
export type ViewportPreset = z.infer<typeof CanonicalViewportPresetSchema>;

const DESKTOP_WIDTH = 1440;
const DESKTOP_HEIGHT = 900;
const DESKTOP_SCALE = 0.55;

const TABLET_WIDTH = 768;
const TABLET_HEIGHT = 1024;
const TABLET_SCALE = 0.75;

const MOBILE_WIDTH = 390;
const MOBILE_HEIGHT = 844;
const MOBILE_SCALE = 1.0;

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

export function normalizeViewportPreset(raw?: string | null): ViewportPreset {
  const val = (raw ?? "").trim().toLowerCase();
  if (val === "tablet" || val === "tablet-768") return "tablet-768";
  if (val === "mobile" || val === "mobile-390") return "mobile-390";
  return "desktop-1440";
}

export const FixtureVariantSchema = z.enum(["DEFAULT", "EMPTY", "LOADING", "ERROR"]);
export type FixtureVariant = z.infer<typeof FixtureVariantSchema>;
export type FixtureRef = FixtureVariant;

const FIXTURE_REF_MAP = new Map<string, FixtureRef>([
  ["DEFAULT", "DEFAULT"],
  ["LOADING", "LOADING"],
  ["EMPTY", "EMPTY"],
  ["ERROR", "ERROR"],
]);

export function normalizeFixtureRef(raw?: string | null): FixtureRef {
  const upper = (raw ?? "DEFAULT").trim().toUpperCase();
  return FIXTURE_REF_MAP.get(upper || "DEFAULT") ?? "DEFAULT";
}

export const FixtureRefSchema = z.preprocess(
  (val) => (typeof val === "string" ? (val.trim() ? val.trim().toUpperCase() : "DEFAULT") : val),
  FixtureVariantSchema
);

export const FIXTURE_VARIANTS: { variant: FixtureVariant; label: string; badgeClass: string }[] = [
  { variant: "DEFAULT", label: "Default", badgeClass: "bg-blue-500/10 text-blue-600 border-blue-500/20" },
  { variant: "EMPTY", label: "Empty", badgeClass: "bg-secondary text-secondary-foreground border-border" },
  { variant: "LOADING", label: "Loading", badgeClass: "bg-purple-500/10 text-purple-600 border-purple-500/20" },
  { variant: "ERROR", label: "Error", badgeClass: "bg-red-500/10 text-red-600 border-red-500/20" },
];

export const CreateScreenFixtureSchema = z.object({
  variant: FixtureVariantSchema,
  payloadJson: z.string().default("{}"),
  isDefault: z.boolean().default(false),
});
export type CreateScreenFixture = z.infer<typeof CreateScreenFixtureSchema>;

export const UpdateScreenFixtureSchema = z.object({
  payloadJson: z.string().optional(),
  isDefault: z.boolean().optional(),
});
export type UpdateScreenFixture = z.infer<typeof UpdateScreenFixtureSchema>;

export const ScreenFixtureResponseSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string(),
  nodeId: z.string(),
  variant: FixtureVariantSchema,
  payloadJson: z.string(),
  isDefault: z.boolean(),
  createdAt: z.string(),
  updatedAt: z.string(),
});
export type ScreenFixtureResponse = z.infer<typeof ScreenFixtureResponseSchema>;

export const PreviewHydrationMessageSchema = z.object({
  type: z.literal("FLOW_PREVIEW_HYDRATE"),
  nodeId: z.string(),
  screenKey: z.string(),
  routePath: z.string(),
  variant: FixtureVariantSchema,
  payload: z.record(z.union([z.string(), z.number(), z.boolean(), z.null()])),
  viewport: z.object({
    preset: ViewportPresetSchema,
    width: z.number(),
    height: z.number(),
  }),
});
export type PreviewHydrationMessage = z.infer<typeof PreviewHydrationMessageSchema>;
