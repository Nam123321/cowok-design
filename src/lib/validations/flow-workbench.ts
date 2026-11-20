// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
import { z } from "zod";
import {
  FixtureRefSchema,
  ViewportPresetSchema,
  type FixtureRef,
  type ViewportPreset,
} from "./screen-fixture";

export { FixtureRefSchema, ViewportPresetSchema };
export type { FixtureRef, ViewportPreset };

export const SemanticNodeTypeSchema = z.enum(["screen", "modal", "condition", "action"]);
export type SemanticNodeType = z.infer<typeof SemanticNodeTypeSchema>;

export const EdgeFlowTypeSchema = z.enum(["main", "if_else", "if_not", "loopback"]);
export type EdgeFlowType = z.infer<typeof EdgeFlowTypeSchema>;

export const TriggerEventSchema = z.enum(["onClick", "onSubmit", "onNavigate", "onTimeout", "custom"]);

const FiniteCoordSchema = z
  .number()
  .finite("Coordinate must be finite")
  .min(-50000, "Coordinate below -50000")
  .max(50000, "Coordinate above 50000");

const BoundedFixtureJsonSchema = z
  .string()
  .refine((v) => new TextEncoder().encode(v).length <= 32768, "fixtureJson exceeds 32768 bytes");

const TimestampStringSchema = z
  .union([z.string().min(1), z.number().int()])
  .transform((v) => String(v));

export const CreateFlowNodeSchema = z.object({
  id: z.string().uuid().optional(),
  portalId: z.string().min(1).max(64).default("default"),
  screenKey: z.string().trim().min(1, "screenKey is required").max(120),
  title: z.string().trim().min(1, "title is required").max(100),
  nodeType: SemanticNodeTypeSchema.default("screen"),
  routePath: z.string().min(1).max(255).default("/"),
  componentEntryPath: z.string().max(500).default(""),
  fixtureRef: FixtureRefSchema.default("DEFAULT"),
  fixtureJson: BoundedFixtureJsonSchema.default("{}"),
  note: z.string().max(4000).default(""),
  viewportPreset: ViewportPresetSchema.default("desktop-1440"),
  posX: FiniteCoordSchema.default(0.0),
  posY: FiniteCoordSchema.default(0.0),
  portalGroup: z.string().trim().optional(),
  sharedAppShellId: z.string().trim().nullable().optional(),
  miniSkeletonHtml: z.string().nullable().optional(),
  ctaPillsJson: z.string().default("[]"),
  revision: z.number().int().min(1).default(1),
});
export type CreateFlowNodeInput = z.input<typeof CreateFlowNodeSchema>;
export type CreateFlowNodePayload = z.infer<typeof CreateFlowNodeSchema>;

export const UpdateFlowNodeSchema = z
  .object({
    title: z.string().trim().min(1).max(100).optional(),
    portalId: z.string().min(1).max(64).optional(),
    nodeType: SemanticNodeTypeSchema.optional(),
    routePath: z.string().min(1).max(255).optional(),
    componentEntryPath: z.string().max(500).optional(),
    fixtureRef: FixtureRefSchema.optional(),
    fixtureJson: BoundedFixtureJsonSchema.optional(),
    note: z.string().max(4000).optional(),
    viewportPreset: ViewportPresetSchema.optional(),
    posX: FiniteCoordSchema.optional(),
    posY: FiniteCoordSchema.optional(),
    expectedVersion: z.number().int().min(1).optional(),
    expectedLayoutVersion: z.number().int().min(1).optional(),
    portalGroup: z.string().trim().nullable().optional(),
    sharedAppShellId: z.string().trim().nullable().optional(),
    miniSkeletonHtml: z.string().nullable().optional(),
    ctaPillsJson: z.string().optional(),
    expectedRevision: z.number().int().min(1).optional(),
  })
  .refine(
    (v) => !(v.expectedVersion !== undefined && v.expectedLayoutVersion !== undefined),
    "Cannot specify both expectedVersion and expectedLayoutVersion"
  );
export type UpdateFlowNodeInput = z.input<typeof UpdateFlowNodeSchema>;
export type UpdateFlowNodePayload = z.infer<typeof UpdateFlowNodeSchema>;

export const FlowNodeResponseSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  portalId: z.string().min(1),
  screenKey: z.string().min(1),
  title: z.string().min(1),
  nodeType: SemanticNodeTypeSchema.default("screen"),
  routePath: z.string(),
  componentEntryPath: z.string(),
  fixtureRef: FixtureRefSchema.default("DEFAULT"),
  fixtureJson: z.string(),
  note: z.string().default(""),
  viewportPreset: ViewportPresetSchema.default("desktop-1440"),
  posX: FiniteCoordSchema,
  posY: FiniteCoordSchema,
  portalGroup: z.string().nullable().optional().default(null),
  sharedAppShellId: z.string().nullable().optional().default(null),
  miniSkeletonHtml: z.string().nullable().optional().default(null),
  ctaPillsJson: z.string().default("[]"),
  version: z.number().int().min(1).default(1),
  layoutVersion: z.number().int().min(1).default(1),
  revision: z.number().int().min(1).default(1),
  createdAt: TimestampStringSchema,
  updatedAt: TimestampStringSchema,
});
export type FlowNodeResponse = z.infer<typeof FlowNodeResponseSchema>;
export const GetFlowNodesResponseSchema = z.array(FlowNodeResponseSchema);

export const BatchNodePositionItemSchema = z.object({
  nodeId: z.string().uuid(),
  posX: FiniteCoordSchema,
  posY: FiniteCoordSchema,
  expectedLayoutVersion: z.number().int().min(1),
});

export const BatchUpdateNodePositionsSchema = z.object({
  positions: z.array(BatchNodePositionItemSchema).max(250),
});
export type BatchUpdateNodePositionsInput = z.input<typeof BatchUpdateNodePositionsSchema>;

export const BatchUpdateNodePositionsResponseSchema = z.object({
  layoutVersion: z.number().int().min(1),
  nodes: z.array(FlowNodeResponseSchema),
});

export const DeleteFlowNodeQuerySchema = z.object({
  expectedVersion: z.coerce.number().int().min(1).optional(),
  expectedGraphVersion: z.coerce.number().int().min(1).optional(),
});

export const DeleteFlowNodeResponseSchema = z.object({
  success: z.boolean(),
  id: z.string().uuid(),
  graphVersion: z.number().int().min(1),
  cascadeDeletedEdgeIds: z.array(z.string().uuid()).default([]),
});

export const CreateFlowTransitionSchema = z
  .object({
    id: z.string().uuid().optional(),
    sourceNodeId: z.string().uuid("sourceNodeId must be a valid UUID"),
    targetNodeId: z.string().uuid("targetNodeId must be a valid UUID"),
    flowType: EdgeFlowTypeSchema.default("main"),
    triggerEvent: TriggerEventSchema.default("onClick"),
    guardCondition: z.string().max(1000).nullable().optional(),
    sourceCtaFfId: z.string().trim().min(1).nullable().optional(),
    maxLoopIterations: z.number().int().min(1).max(10).default(3),
    allowConfirmedLoopback: z.boolean().optional(),
    revision: z.number().int().min(1).optional(),
  })
  .refine((v) => v.sourceNodeId !== v.targetNodeId, {
    message: "Self-loop transitions (sourceNodeId === targetNodeId) are forbidden",
    path: ["targetNodeId"],
  });
export type CreateFlowTransitionInput = z.input<typeof CreateFlowTransitionSchema>;
export type CreateFlowTransitionPayload = z.infer<typeof CreateFlowTransitionSchema>;

export const UpdateFlowTransitionSchema = z.object({
  flowType: EdgeFlowTypeSchema.optional(),
  triggerEvent: TriggerEventSchema.optional(),
  guardCondition: z.string().max(1000).nullable().optional(),
  reversed: z.boolean().optional(),
  expectedVersion: z.number().int().min(1).optional(),
  sourceCtaFfId: z.string().trim().min(1).nullable().optional(),
  maxLoopIterations: z.number().int().min(1).max(10).optional(),
  allowConfirmedLoopback: z.boolean().optional(),
  expectedRevision: z.number().int().min(1).optional(),
});
export type UpdateFlowTransitionInput = z.input<typeof UpdateFlowTransitionSchema>;
export type UpdateFlowTransitionPayload = z.infer<typeof UpdateFlowTransitionSchema>;

export const FlowTransitionResponseSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  sourceNodeId: z.string().uuid(),
  targetNodeId: z.string().uuid(),
  flowType: EdgeFlowTypeSchema.default("main"),
  triggerEvent: z.string().min(1),
  guardCondition: z.string().nullable().optional().default(null),
  sourceCtaFfId: z.string().nullable().optional().default(null),
  maxLoopIterations: z.number().int().min(1).max(10).default(3),
  version: z.number().int().min(1).default(1),
  revision: z.number().int().min(1).default(1),
  createdAt: TimestampStringSchema,
  updatedAt: TimestampStringSchema,
});
export type FlowTransitionResponse = z.infer<typeof FlowTransitionResponseSchema>;
export const GetFlowTransitionsResponseSchema = z.array(FlowTransitionResponseSchema);

export const DeleteTransitionQuerySchema = z.object({
  expectedVersion: z.coerce.number().int().min(1).optional(),
});

export const DeleteTransitionResponseSchema = z.object({
  success: z.boolean(),
  id: z.string().uuid(),
  graphVersion: z.number().int().min(1),
});

export const FlowGraphSnapshotSchema = z.object({
  graphId: z.string().min(1),
  graphVersion: z.number().int().min(1),
  layoutVersion: z.number().int().min(1),
  revision: z.number().int().min(1).default(1),
  nodes: z.array(FlowNodeResponseSchema),
  transitions: z.array(FlowTransitionResponseSchema),
});
export type FlowGraphSnapshot = z.infer<typeof FlowGraphSnapshotSchema>;

export const DeployDraftSchema = z.object({
  draftId: z.string().uuid("draftId must be a fixed UUIDv4 per draft session"),
  expectedGraphVersion: z.number().int().min(1),
  createdNodes: z.array(CreateFlowNodeSchema).max(250).default([]),
  updatedNodes: z
    .array(z.object({ nodeId: z.string().uuid(), patch: UpdateFlowNodeSchema }))
    .max(250)
    .default([]),
  deletedNodes: z
    .array(z.object({ nodeId: z.string().uuid(), expectedVersion: z.number().int().min(1) }))
    .max(250)
    .default([]),
  updatedNodePositions: z.array(BatchNodePositionItemSchema).max(250).default([]),
  createdTransitions: z.array(CreateFlowTransitionSchema).max(500).default([]),
  updatedTransitions: z
    .array(z.object({ transitionId: z.string().uuid(), patch: UpdateFlowTransitionSchema }))
    .max(500)
    .default([]),
  deletedTransitions: z
    .array(z.object({ transitionId: z.string().uuid(), expectedVersion: z.number().int().min(1) }))
    .max(500)
    .default([]),
  allowConfirmedLoopback: z.boolean().optional(),
  expectedRevision: z.number().int().min(1).optional(),
});
export type DeployDraftInput = z.input<typeof DeployDraftSchema>;
export const DeployDraftResponseSchema = FlowGraphSnapshotSchema;

const RectBoxSchema = z.object({
  left: z.number().finite(),
  top: z.number().finite(),
  width: z.number().finite().nonnegative(),
  height: z.number().finite().nonnegative(),
});

export const SpatialMarqueeSelectionSchema = z.object({
  x: z.number().finite(),
  y: z.number().finite(),
  width: z.number().finite().min(16),
  height: z.number().finite().min(16),
  elementFfIds: z.array(z.string().min(1)),
});
export type SpatialMarqueeSelection = z.infer<typeof SpatialMarqueeSelectionSchema>;

export const HostToIframeMessageSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("INIT_PORT"),
    epoch: z.number().int().min(1),
    bootSecret: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    artifactSha256: z.string().max(128).optional(),
  }),
  z.object({
    type: z.literal("APPLY_HTML"),
    epoch: z.number().int().min(1),
    bootSecret: z.string().min(16).max(128).optional(),
    html: z.string().max(1_048_576),
  }),
  z.object({
    type: z.literal("FLOW_PREVIEW_HYDRATE"),
    epoch: z.number().int().min(1),
    bootSecret: z.string().min(16).max(128).optional(),
    variant: z.enum(["DEFAULT", "EMPTY", "LOADING", "ERROR"]).optional(),
    artifactSha256: z.string().max(128).optional(),
    fixtureJson: z.string().max(32768),
  }),
  z.object({
    type: z.literal("PING"),
    epoch: z.number().int().min(1),
    bootSecret: z.string().min(16).max(128).optional(),
  }),
]);
export type HostToIframeMessage = z.infer<typeof HostToIframeMessageSchema>;

export const ElementInspectMetadataSchema = z.object({
  tagName: z.string().max(64).optional(),
  textContent: z.string().max(500).optional(),
  slotKind: z.string().max(64).optional(),
  cssVariables: z.record(z.string().max(128), z.string().max(256)).optional(),
  overflowPx: z.number().finite().min(0).max(50_000).optional(),
});
export type ElementInspectMetadata = z.infer<typeof ElementInspectMetadataSchema>;

export const IframeToHostMessageSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("READY"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
  }),
  z.object({
    type: z.literal("PORT_ACK"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    artifactSha256: z.string().max(128),
    domAnchorHash: z.string().max(128),
  }),
  z.object({
    type: z.literal("ELEMENT_CLICKED"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    selector: z.string().max(256),
    elementFfId: z.string().max(200).optional(),
    tagName: z.string().max(64),
    domHash: z.string().max(64).optional(),
    rect: RectBoxSchema.optional(),
    metadata: ElementInspectMetadataSchema.optional(),
    altKey: z.boolean().optional(),
    shiftKey: z.boolean().optional(),
    metaKey: z.boolean().optional(),
    ctrlKey: z.boolean().optional(),
  }),
  z.object({
    type: z.literal("ELEMENT_CONTEXT_MENU"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    selector: z.string().max(256),
    elementFfId: z.string().max(200).optional(),
    tagName: z.string().max(64),
    domHash: z.string().max(64).optional(),
    rect: RectBoxSchema.optional(),
    clientX: z.number().finite().optional(),
    clientY: z.number().finite().optional(),
    metadata: ElementInspectMetadataSchema.optional(),
  }),
  z.object({
    type: z.literal("ELEMENT_RECTS"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    rects: z.record(z.string().max(200), RectBoxSchema),
  }),
  z.object({
    type: z.literal("TEXT_QUOTE"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    text: z.string().min(1).max(2000),
    rect: RectBoxSchema.optional(),
  }),
  z.object({
    type: z.literal("HEIGHT_CHANGED"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    height: z.number().finite().min(0).max(100_000),
  }),
  z.object({
    type: z.literal("RUNTIME_ERROR"),
    epoch: z.number().int().min(1),
    bootSecretAck: z.string().min(16).max(128).optional(),
    channelSecret: z.string().min(16).max(128).optional(),
    seq: z.number().int().nonnegative().optional(),
    message: z.string().max(1024),
  }),
]);
export type IframeToHostMessage = z.infer<typeof IframeToHostMessageSchema>;

export const CreateFlowNodePayloadSchema = CreateFlowNodeSchema;
export const CreateFlowTransitionPayloadSchema = CreateFlowTransitionSchema;
export const DeployFlowDraftPayloadSchema = DeployDraftSchema;
export const BatchUpdateNodePositionsPayloadSchema = BatchUpdateNodePositionsSchema;
export const FlowGraphSnapshotDTOSchema = FlowGraphSnapshotSchema;
export const FlowNodeResponseDTOSchema = FlowNodeResponseSchema;
export const FlowTransitionResponseDTOSchema = FlowTransitionResponseSchema;
export const UpdateFlowNodePayloadSchema = UpdateFlowNodeSchema;
export const UpdateFlowTransitionPayloadSchema = UpdateFlowTransitionSchema;
