// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";

export const MAX_SPLIT_EXPORT_BYTES = 25 * 1024 * 1024;
const SHA256_HEX_LEN = 64;
export const SAFE_RELATIVE_INSTALL_ACTION_REGEX = /^\/(?!\/)[a-zA-Z0-9/_?=&-]+$/;

const SafeInstallActionSchema = z
  .string()
  .min(1)
  .regex(
    SAFE_RELATIVE_INSTALL_ACTION_REGEX,
    "installAction must be a safe relative application path"
  );

export const ExportAudienceModeSchema = z.enum([
  "SHARE_PRESENT",
  "DEVELOPER_HANDOFF",
]);

export const ExportFormatSchema = z.enum([
  "OFFLINE_HTML",
  "VECTOR_PDF",
  "PLUGIN_PPTX",
  "DEV_HANDOFF_BUNDLE",
]);

export const QcGateSummarySchema = z.object({
  status: z.string().min(1),
  p0Count: z.number().int().nonnegative(),
  p1Count: z.number().int().nonnegative(),
  latestRunId: z.string().nullable().optional(),
});

export const HtmlBundleExportRequestSchema = z.object({
  projectId: z.string().min(1),
  title: z.string().min(1).max(160).default("Presentation"),
  html: z.string().default(""),
  inlineCss: z.string().optional(),
  casAssets: z.record(z.string(), z.string()).optional(),
  enablePresenter: z.boolean().default(true),
  overrideQcWarning: z.boolean().default(false),
});

export const HtmlBundleExportDataSchema = z.object({
  exportId: z.string().min(1),
  projectId: z.string().min(1),
  audienceMode: ExportAudienceModeSchema,
  format: z.literal("OFFLINE_HTML"),
  fileName: z.string().min(1),
  sha256: z.string().length(SHA256_HEX_LEN),
  byteSize: z.number().int().nonnegative(),
  externalCdnRefsCount: z.number().int().nonnegative(),
  strippedExternalRefsCount: z.number().int().nonnegative(),
  presenterEnabled: z.boolean(),
  slideCount: z.number().int().positive(),
  svgCount: z.number().int().nonnegative(),
  brokenSvgCount: z.number().int().nonnegative(),
  inlineAssetsCount: z.number().int().nonnegative(),
  brokenCasAssetsCount: z.number().int().nonnegative(),
  cspHeader: z.string().min(1),
  qcGateStatus: z.string().min(1),
  qcOverrideAcknowledged: z.boolean(),
  warnings: z.array(z.string()),
  htmlContent: z.string(),
  createdAt: z.number().int(),
});

export const HtmlBundleExportResponseSchema = z.object({
  ok: z.literal(true),
  data: HtmlBundleExportDataSchema,
  requestId: z.string(),
});

export const VectorPdfSlideSchema = z.object({
  heading: z.string().min(1),
  bodyLines: z.array(z.string()).default([]),
  footer: z.string().optional(),
});

export const VectorPdfExportRequestSchema = z.object({
  projectId: z.string().min(1),
  title: z.string().min(1).max(160).default("Presentation"),
  slides: z.array(VectorPdfSlideSchema).optional(),
  html: z.string().optional(),
  overrideQcWarning: z.boolean().default(false),
});

export const VectorPdfExportDataSchema = z.object({
  exportId: z.string().min(1),
  projectId: z.string().min(1),
  audienceMode: ExportAudienceModeSchema,
  format: z.literal("VECTOR_PDF"),
  fileName: z.string().min(1),
  pdfBase64: z.string().min(1),
  sha256: z.string().length(SHA256_HEX_LEN),
  byteSize: z.number().int().positive(),
  pageCount: z.number().int().positive(),
  widthPt: z.number().positive(),
  heightPt: z.number().positive(),
  engine: z.string().min(1),
  qcGateStatus: z.string().min(1),
  qcOverrideAcknowledged: z.boolean(),
  warnings: z.array(z.string()),
  createdAt: z.number().int(),
});

export const VectorPdfExportResponseSchema = z.object({
  ok: z.literal(true),
  data: VectorPdfExportDataSchema,
  requestId: z.string(),
});

export const PptxExportRequestSchema = z.object({
  projectId: z.string().min(1),
  title: z.string().min(1).max(160).default("Presentation"),
  slideCount: z.number().int().positive().optional(),
  overrideQcWarning: z.boolean().default(false),
});

export const PptxExportDataSchema = z.object({
  exportId: z.string().min(1),
  projectId: z.string().min(1),
  audienceMode: ExportAudienceModeSchema,
  format: z.literal("PLUGIN_PPTX"),
  pluginInstalled: z.boolean(),
  pluginId: z.string().min(1),
  pluginStatus: z.string().min(1),
  installAction: SafeInstallActionSchema,
  message: z.string().min(1),
  fileName: z.string().nullable().optional(),
  sha256: z.string().nullable().optional(),
  byteSize: z.number().int().nonnegative(),
  qcGateStatus: z.string().min(1),
  qcOverrideAcknowledged: z.boolean(),
  createdAt: z.number().int(),
});

export const PptxExportResponseSchema = z.object({
  ok: z.literal(true),
  pluginInstalled: z.boolean(),
  pluginId: z.string().min(1),
  installAction: SafeInstallActionSchema,
  message: z.string().min(1),
  data: PptxExportDataSchema,
  requestId: z.string(),
});

export const DeveloperHandoffRequestSchema = z.object({
  projectId: z.string().min(1),
  projectName: z.string().min(1).max(160).default("Presentation"),
  specMarkdown: z.string().optional(),
  designTokensCss: z.string().optional(),
  designTokensJson: z.string().optional(),
  screenTsx: z.string().optional(),
  htmlSource: z.string().optional(),
  e2eSpecTs: z.string().optional(),
  overrideQcWarning: z.boolean().default(false),
});

export const DeveloperHandoffFileSchema = z.object({
  path: z.string().min(1),
  mimeType: z.string().min(1),
  byteSize: z.number().int().nonnegative(),
  sha256: z.string().length(SHA256_HEX_LEN),
  content: z.string(),
});

export const DeveloperHandoffDataSchema = z.object({
  exportId: z.string().min(1),
  projectId: z.string().min(1),
  audienceMode: z.literal("DEVELOPER_HANDOFF"),
  format: z.literal("DEV_HANDOFF_BUNDLE"),
  fileName: z.string().min(1),
  sha256: z.string().length(SHA256_HEX_LEN),
  byteSize: z.number().int().nonnegative(),
  files: z.array(DeveloperHandoffFileSchema),
  qcGateStatus: z.string().min(1),
  qcOverrideAcknowledged: z.boolean(),
  createdAt: z.number().int(),
});

export const DeveloperHandoffResponseSchema = z.object({
  ok: z.literal(true),
  data: DeveloperHandoffDataSchema,
  requestId: z.string(),
});

export const ExportHistoryItemSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().min(1),
  audienceMode: ExportAudienceModeSchema,
  format: ExportFormatSchema,
  fileName: z.string().min(1),
  artifactSha256: z.string(),
  byteSize: z.number().int().nonnegative(),
  externalCdnRefsCount: z.number().int().nonnegative(),
  qcGateStatus: z.string().min(1),
  qcOverrideAcknowledged: z.boolean(),
  pluginStatus: z.string().min(1),
  createdAt: z.number().int(),
});

export const ExportHistoryDataSchema = z.object({
  projectId: z.string().min(1),
  qcGate: QcGateSummarySchema,
  pptxPluginInstalled: z.boolean(),
  items: z.array(ExportHistoryItemSchema),
});

export const ExportHistoryResponseSchema = z.object({
  ok: z.literal(true),
  data: ExportHistoryDataSchema,
  requestId: z.string(),
});

export const SplitExportErrorEnvelopeSchema = z.object({
  error: z.string().min(1),
  message: z.string().min(1),
  p0Count: z.number().int().nonnegative().optional(),
  p1Count: z.number().int().nonnegative().optional(),
  qcRunId: z.string().optional(),
  limitBytes: z.number().int().nonnegative().optional(),
  actualBytes: z.number().int().nonnegative().optional(),
  requestId: z.string().optional(),
});

export type ExportAudienceMode = z.infer<typeof ExportAudienceModeSchema>;
export type ExportFormat = z.infer<typeof ExportFormatSchema>;
export type QcGateSummary = z.infer<typeof QcGateSummarySchema>;
export type HtmlBundleExportRequest = z.input<typeof HtmlBundleExportRequestSchema>;
export type HtmlBundleExportData = z.infer<typeof HtmlBundleExportDataSchema>;
export type HtmlBundleExportResponse = z.infer<typeof HtmlBundleExportResponseSchema>;
export type VectorPdfSlide = z.input<typeof VectorPdfSlideSchema>;
export type VectorPdfExportRequest = z.input<typeof VectorPdfExportRequestSchema>;
export type VectorPdfExportData = z.infer<typeof VectorPdfExportDataSchema>;
export type VectorPdfExportResponse = z.infer<typeof VectorPdfExportResponseSchema>;
export type PptxExportRequest = z.input<typeof PptxExportRequestSchema>;
export type PptxExportData = z.infer<typeof PptxExportDataSchema>;
export type PptxExportResponse = z.infer<typeof PptxExportResponseSchema>;
export type DeveloperHandoffRequest = z.input<typeof DeveloperHandoffRequestSchema>;
export type DeveloperHandoffFile = z.infer<typeof DeveloperHandoffFileSchema>;
export type DeveloperHandoffData = z.infer<typeof DeveloperHandoffDataSchema>;
export type DeveloperHandoffResponse = z.infer<typeof DeveloperHandoffResponseSchema>;
export type ExportHistoryItem = z.infer<typeof ExportHistoryItemSchema>;
export type ExportHistoryData = z.infer<typeof ExportHistoryDataSchema>;
export type ExportHistoryResponse = z.infer<typeof ExportHistoryResponseSchema>;
