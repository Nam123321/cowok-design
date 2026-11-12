import { z } from "zod";

export const ViewportPresetSchema = z.enum(["desktop-1440", "tablet-768", "mobile-390"]);
export type ViewportPreset = z.infer<typeof ViewportPresetSchema>;

export const FixtureVariantSchema = z.enum(["DEFAULT", "EMPTY", "LOADING", "ERROR"]);
export type FixtureVariant = z.infer<typeof FixtureVariantSchema>;

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
