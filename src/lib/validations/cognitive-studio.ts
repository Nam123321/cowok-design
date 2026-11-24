// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z } from "zod";

export const ThemeModeIdSchema = z
  .string()
  .trim()
  .regex(/^[a-z0-9][a-z0-9-]{0,31}$/);

export const BrandModeTokensSchema = z.object({
  modeId: ThemeModeIdSchema,
  label: z.string().min(1).max(48),
  tokens: z.record(z.string().regex(/^--[a-zA-Z0-9-]+$/), z.string().min(1).max(128)),
  wcagAaVerified: z.boolean(),
});

export const FontPairingSchema = z.object({
  heading: z.string().min(1).max(128),
  body: z.string().min(1).max(128),
  mono: z.string().min(1).max(128),
});

export const BrandProposalV1Schema = z.object({
  id: z.string().min(1).max(64),
  brandName: z.string().min(1).max(80),
  rationale: z.string().min(1).max(600),
  inspirationRefs: z.array(z.string().min(1).max(120)).max(6),
  themeModes: z.array(BrandModeTokensSchema).min(1).max(6),
  fontPairing: FontPairingSchema,
  radiusScale: z.enum(["sharp", "balanced", "soft", "pill"]),
  density: z.enum(["airy", "comfortable", "compact"]),
  componentStyleOverrides: z.record(z.string(), z.record(z.string(), z.string())).default({}),
});

export const SketchPageInputSchema = z.object({
  screenId: z.string().min(1).max(128),
  title: z.string().min(1).max(120),
  pageRole: z.string().max(64).default(""),
  html: z.string().min(1).max(262_144),
});

export const OklchContrastReportSchema = z.object({
  originalFgHex: z.string(),
  bgHex: z.string(),
  adjustedFgHex: z.string(),
  originalRatio: z.number(),
  adjustedRatio: z.number(),
  minRatio: z.number(),
  clamped: z.boolean(),
  warning: z.string().nullable().optional(),
});

export const TokenizedPagePreviewSchema = z.object({
  screenId: z.string(),
  title: z.string(),
  pageRole: z.string(),
  modeId: ThemeModeIdSchema,
  renderedHtml: z.string(),
  contentSha256: z.string(),
  wcagAaVerified: z.boolean(),
  contrastReports: z.array(OklchContrastReportSchema),
  renderTimeMicros: z.number().nonnegative(),
});

export const BrandPreviewBundleSchema = z.object({
  proposalId: z.string(),
  activeModeId: ThemeModeIdSchema,
  requestedModes: z.array(ThemeModeIdSchema).min(1).max(6),
  totalSketchPages: z.number().int().nonnegative(),
  cappedToRepresentative: z.boolean(),
  pages: z.array(TokenizedPagePreviewSchema).max(3),
  totalRenderTimeMicros: z.number().nonnegative(),
});

export const GenerateBrandProposalsRequestSchema = z.object({
  projectId: z.string().uuid(),
  referenceApps: z.array(z.string().min(1).max(120)).max(6).optional(),
  themeModes: z.array(ThemeModeIdSchema).min(1).max(6),
  locale: z.enum(["vi", "en"]).optional(),
  activeModeId: ThemeModeIdSchema.optional(),
  selectedProposalId: z.string().min(1).max(64).optional(),
  sketchPages: z.array(SketchPageInputSchema).max(64).optional(),
});

export const BrandProposalsResponseDataSchema = z.object({
  projectId: z.string().uuid(),
  requestedModes: z.array(ThemeModeIdSchema).min(1).max(6),
  activeModeId: ThemeModeIdSchema,
  selectedProposalId: z.string(),
  proposals: z.array(BrandProposalV1Schema).min(1).max(6),
  previewBundle: BrandPreviewBundleSchema,
});

export const BrandProposalsResponseSchema = z.object({
  ok: z.literal(true),
  data: BrandProposalsResponseDataSchema,
  requestId: z.string(),
});

export const PopularComponentAuditItemSchema = z.object({
  componentKind: z.string().min(1).max(64),
  category: z.enum(["feedback", "overlay", "navigation", "input", "display", "deck"]),
  status: z.enum(["detected_in_sketch", "suggested_contextual", "optional", "not_needed"]),
  reason: z.string().max(240).optional(),
  selected: z.boolean(),
  occurrenceCount: z.number().int().nonnegative().default(0),
});

export const ScanComponentsRequestSchema = z.object({
  projectId: z.string().uuid(),
  portalId: z.string().min(1).max(48).optional(),
  surface: z.enum(["fe", "deck"]).optional(),
  sketchHtmls: z.array(z.string().max(262_144)).max(32).optional(),
  selectedKinds: z.array(z.string().min(1).max(64)).max(64).optional(),
});

export const ScanComponentsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    projectId: z.string().uuid(),
    portalId: z.string(),
    extractedKinds: z.array(z.string()),
    auditItems: z.array(PopularComponentAuditItemSchema),
    detectedCount: z.number().int().nonnegative(),
    suggestedCount: z.number().int().nonnegative(),
    notNeededCount: z.number().int().nonnegative(),
    syncedToRegistryCount: z.number().int().nonnegative(),
  }),
  requestId: z.string(),
});

export const FeDataContractDraftSchema = z.object({
  projectId: z.string().uuid(),
  screenId: z.string().min(1).max(128),
  boundaryNotice: z.literal("FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"),
  viewModelFields: z.array(
    z.object({
      ffId: z.string(),
      fieldName: z.string(),
      inferredType: z.enum([
        "string",
        "number",
        "currency",
        "boolean",
        "date",
        "enum",
        "array",
        "chart_series",
      ]),
      sampleValue: z.unknown(),
    })
  ),
  actionInputs: z.array(
    z.object({
      actionId: z.string(),
      triggerFfId: z.string(),
      requiredFields: z.array(z.string()),
    })
  ),
  subStateFixtures: z.record(z.enum(["DEFAULT", "EMPTY", "LOADING", "ERROR"]), z.record(z.unknown())),
});

export const FeDataContractQuerySchema = z.object({
  projectId: z.string().uuid(),
  screenId: z.string().min(1).max(128).optional(),
});

export const FeDataContractResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    draft: FeDataContractDraftSchema,
    draftTsSource: z.string(),
    diskPath: z.string().optional(),
  }),
  requestId: z.string(),
});

export const RevealSpecsFolderRequestSchema = z.object({
  projectId: z.string().uuid(),
  fileName: z.string().min(1).max(96).optional(),
  dryRun: z.boolean().optional(),
});
export type RevealSpecsFolderRequest = z.infer<typeof RevealSpecsFolderRequestSchema>;

export const RevealSpecsFolderResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    opened: z.boolean(),
    relativePath: z.string(),
    specsDir: z.string(),
    mode: z.string(),
  }),
  requestId: z.string(),
});
export type RevealSpecsFolderResponse = z.infer<typeof RevealSpecsFolderResponseSchema>;

export const HiddenSpecKeySchema = z.enum([
  "01_PRODUCT_BRIEF",
  "02_BRAND_IDENTITY",
  "03_UX_FLOW",
  "04_DESIGN_TOKENS",
  "05_SCREEN_SPECS",
  "06_DATA_CONTRACT",
  "07_COMPONENT_REGISTRY",
  "08_INTERACTION_STATES",
  "09_QA_ACCEPTANCE",
]);

export const HiddenSpecDocumentSchema = z.object({
  specKey: HiddenSpecKeySchema,
  title: z.string(),
  revision: z.number().int().nonnegative(),
  contentSha256: z.string(),
  contentMarkdown: z.string(),
  updatedAt: z.number().int().nonnegative(),
});

export const HiddenSpecsQuerySchema = z.object({
  projectId: z.string().uuid(),
});

export const HiddenSpecsListResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    projectId: z.string().uuid(),
    specs: z.array(HiddenSpecDocumentSchema),
  }),
  requestId: z.string(),
});

export const PutHiddenSpecRequestSchema = z.object({
  projectId: z.string().uuid(),
  specKey: HiddenSpecKeySchema,
  contentMarkdown: z.string().max(262_144),
  expectedRevision: z.number().int().nonnegative(),
});

export const PutHiddenSpecResponseSchema = z.object({
  ok: z.literal(true),
  data: HiddenSpecDocumentSchema,
  requestId: z.string(),
});

export const AdaptiveEntryModeSchema = z.enum([
  "GREENFIELD",
  "ADD_FEATURE",
  "REVAMP",
  "MICRO_POLISH",
  "DECK_FROM_APP",
]);

export const HeritageDesignPackSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string(),
  name: z.string().min(1).max(80),
  sourceProjectId: z.string().uuid(),
  entryMode: AdaptiveEntryModeSchema,
  brandIdentity: z.record(z.unknown()).default({}),
  componentRegistry: z.record(z.unknown()).default({}),
  presentationTheme: z.record(z.unknown()).default({}),
  themeModes: z.array(ThemeModeIdSchema).min(1).max(6),
  createdAt: z.number().int().nonnegative(),
});

export const HeritagePacksListResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    packs: z.array(HeritageDesignPackSchema),
  }),
  requestId: z.string(),
});

export const CreateHeritagePackRequestSchema = z.object({
  sourceProjectId: z.string().uuid(),
  name: z.string().trim().min(1).max(80),
  entryMode: AdaptiveEntryModeSchema.default("GREENFIELD"),
  brandIdentity: z.record(z.unknown()).default({}),
  componentRegistry: z.record(z.unknown()).default({}),
  presentationTheme: z.record(z.unknown()).default({}),
  themeModes: z.array(ThemeModeIdSchema).min(1).max(6),
});

export const CreateHeritagePackResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    pack: HeritageDesignPackSchema,
    filePath: z.string(),
  }),
  requestId: z.string(),
});

export const CognitiveStudioErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "CSS_SECURITY_BLOCKED",
  "UNAUTHORIZED",
  "FORBIDDEN",
  "PROJECT_NOT_FOUND",
  "PROJECT_TENANT_MISMATCH",
  "PAYLOAD_TOO_LARGE",
  "STALE_MAPPING",
  "STORAGE_BUSY",
  "FS_JAIL_ERROR",
  "INTERNAL_ERROR",
]);

export const CognitiveStudioErrorResponseSchema = z.object({
  error: z.object({
    code: CognitiveStudioErrorCodeSchema,
    message: z.string(),
    field: z.string().optional(),
    requestId: z.string(),
    details: z.record(z.unknown()).optional(),
  }),
});

export type BrandModeTokens = z.infer<typeof BrandModeTokensSchema>;
export type BrandProposalV1 = z.infer<typeof BrandProposalV1Schema>;
export type TokenizedPagePreview = z.infer<typeof TokenizedPagePreviewSchema>;
export type BrandPreviewBundle = z.infer<typeof BrandPreviewBundleSchema>;
export type GenerateBrandProposalsRequest = z.infer<typeof GenerateBrandProposalsRequestSchema>;
export type BrandProposalsResponse = z.infer<typeof BrandProposalsResponseSchema>;
export type PopularComponentAuditItem = z.infer<typeof PopularComponentAuditItemSchema>;
export type ScanComponentsRequest = z.infer<typeof ScanComponentsRequestSchema>;
export type ScanComponentsResponse = z.infer<typeof ScanComponentsResponseSchema>;
export type FeDataContractDraft = z.infer<typeof FeDataContractDraftSchema>;
export type FeDataContractResponse = z.infer<typeof FeDataContractResponseSchema>;
export type HiddenSpecKey = z.infer<typeof HiddenSpecKeySchema>;
export type HiddenSpecDocument = z.infer<typeof HiddenSpecDocumentSchema>;
export type HiddenSpecsListResponse = z.infer<typeof HiddenSpecsListResponseSchema>;
export type PutHiddenSpecRequest = z.infer<typeof PutHiddenSpecRequestSchema>;
export type PutHiddenSpecResponse = z.infer<typeof PutHiddenSpecResponseSchema>;
export type AdaptiveEntryMode = z.infer<typeof AdaptiveEntryModeSchema>;
export type HeritageDesignPack = z.infer<typeof HeritageDesignPackSchema>;
export type HeritagePacksListResponse = z.infer<typeof HeritagePacksListResponseSchema>;
export type CreateHeritagePackRequest = z.infer<typeof CreateHeritagePackRequestSchema>;
export type CreateHeritagePackResponse = z.infer<typeof CreateHeritagePackResponseSchema>;
