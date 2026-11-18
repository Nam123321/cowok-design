// @story 08.4
// @implements AC-01, AC-04, AC-05
import { z } from "zod";
import { PluginSummaryDtoSchema } from "../../lib/validations/plugins";
import type { PluginSummaryDto } from "../../lib/validations/plugins";
import {
  CronScheduleStatusDtoSchema,
  KnowledgeCardDtoSchema,
  type CronScheduleStatusDto,
  type KnowledgeCardDto,
} from "../../lib/validations/plugins-learn-cron";

const SCHEMA_VERSION_V1 = 1;

export const PLUGIN_CENTER_CARDS = {
  pluginSummaryCard: {
    cardId: "plugin-summary-card",
    schemaVersion: SCHEMA_VERSION_V1,
    designPatternRef: "DataTableView [5.21]",
    payloadSchema: z.object({
      plugin: PluginSummaryDtoSchema,
      showStagingDiff: z.boolean().default(false),
    }),
  },
  learnReviewCard: {
    cardId: "learn-review-card",
    schemaVersion: SCHEMA_VERSION_V1,
    designPatternRef: "RightDrawer [5.25]",
    payloadSchema: z.object({
      card: KnowledgeCardDtoSchema,
    }),
  },
  cronScheduleCard: {
    cardId: "cron-schedule-card",
    schemaVersion: SCHEMA_VERSION_V1,
    designPatternRef: "Metric Line [5.10]",
    payloadSchema: z.object({
      schedule: CronScheduleStatusDtoSchema,
    }),
  },
} as const;

export function validateCardItems<T>(items: T[], validate: (item: T) => boolean, label: string): { valid: T[]; error: string | null } {
  const valid: T[] = [];
  let invalidCount = 0;
  for (const item of items) {
    if (validate(item)) valid.push(item);
    else invalidCount += 1;
  }
  return {
    valid,
    error: invalidCount > 0 ? `${invalidCount} ${label} payload(s) failed contract validation` : null,
  };
}

export function validatePluginCards(plugins: PluginSummaryDto[]): { valid: PluginSummaryDto[]; error: string | null } {
  return validateCardItems(
    plugins,
    (plugin) => PLUGIN_CENTER_CARDS.pluginSummaryCard.payloadSchema.safeParse({ plugin }).success,
    "plugin"
  );
}

export function validateLearnCards(cards: KnowledgeCardDto[]): { valid: KnowledgeCardDto[]; error: string | null } {
  return validateCardItems(
    cards,
    (card) => PLUGIN_CENTER_CARDS.learnReviewCard.payloadSchema.safeParse({ card }).success,
    "LEARN card"
  );
}

export function validateCronSchedules(
  schedulesByPlugin: Record<string, CronScheduleStatusDto | null>
): { valid: Record<string, CronScheduleStatusDto | null>; error: string | null } {
  const valid: Record<string, CronScheduleStatusDto | null> = {};
  let invalidCount = 0;
  for (const [pluginId, schedule] of Object.entries(schedulesByPlugin)) {
    if (schedule === null) {
      valid[pluginId] = null;
    } else if (PLUGIN_CENTER_CARDS.cronScheduleCard.payloadSchema.safeParse({ schedule }).success) {
      valid[pluginId] = schedule;
    } else {
      invalidCount += 1;
    }
  }
  return {
    valid,
    error: invalidCount > 0 ? `${invalidCount} cron schedule payload(s) failed contract validation` : null,
  };
}
