// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";

export const PortalIdSchema = z.enum([
  "SuperAdmin",
  "TenantAdmin",
  "SalesApp",
  "MarketingWeb",
  "DeveloperPortal",
  "Presentation",
]);
export type PortalId = z.infer<typeof PortalIdSchema>;

export const TaskHorizonSchema = z.enum(["short", "mid", "long"]);
export type TaskHorizon = z.infer<typeof TaskHorizonSchema>;

export const ContextModelProfileSchema = z.enum(["A", "B", "C"]);
export type ContextModelProfile = z.infer<typeof ContextModelProfileSchema>;

export const ContextAdapterModeSchema = z.enum([
  "frontier_full",
  "local_reduced",
  "offline_fts5",
]);
export type ContextAdapterMode = z.infer<typeof ContextAdapterModeSchema>;

export const ContextReasonCodeSchema = z.enum([
  "atomic_complete",
  "pruned_by_budget",
  "cross_portal_leak_blocked",
  "atomic_block_over_budget",
]);
export type ContextReasonCode = z.infer<typeof ContextReasonCodeSchema>;
