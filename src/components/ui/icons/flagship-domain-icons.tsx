// @story 04.1
// Canonical domain & agent flagship icon components aligning 100% of flagship-icon-manifest.json (143 icons)
import type * as React from "react";
import {
  type CowokIconProps,
  CopyTextIcon,
  HelpCircleIcon,
  ShieldAlertIcon,
  Trash2Icon,
  MessageSquareIcon,
  SettingsIcon,
} from "./core-icons";
import { AgentMktBrandIcon, AiAgentIcon, ProposalIcon } from "./agent-icons";
import { LayersIcon, RefreshIcon, ReplyIcon } from "./navigation-icons";
import { BellIcon, CalendarIcon, LockIcon, TeamIcon, UserIcon, UsersIcon, WorkspaceIcon } from "./workflow-icons";

const svgBase = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

function makePathIcon(paths: readonly string[]): React.ComponentType<CowokIconProps> {
  return function FlagshipPathIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
    return (
      <svg viewBox="0 0 24 24" width={size} height={size} {...svgBase} strokeWidth={strokeWidth} {...props}>
        {paths.map((d, idx) => (
          <path key={idx} d={d} />
        ))}
      </svg>
    );
  };
}

function makeAgentPersonaIcon(badgeChar: string): React.ComponentType<CowokIconProps> {
  return function FlagshipAgentIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
    return (
      <svg viewBox="0 0 24 24" width={size} height={size} {...svgBase} strokeWidth={strokeWidth} {...props}>
        <rect x="4" y="4" width="16" height="16" rx="4" />
        <text
          x="12"
          y="15.5"
          textAnchor="middle"
          fontSize="9"
          fontWeight="600"
          fill="currentColor"
          stroke="none"
        >
          {badgeChar}
        </text>
      </svg>
    );
  };
}

const DatabaseGlyph = makePathIcon([
  "M12 3c4.4 0 8 1.3 8 3s-3.6 3-8 3-8-1.3-8-3 3.6-3 8-3Z",
  "M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6",
  "M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6",
]);
const FileGlyph = makePathIcon(["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z", "M14 2v6h6"]);
const FileTextGlyph = makePathIcon([
  "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z",
  "M14 2v6h6",
  "M8 13h8",
  "M8 17h6",
]);
const AnalyticsGlyph = makePathIcon(["M3 3v18h18", "M7 15v-4", "M12 15V7", "M17 15v-6"]);
const TrendGlyph = makePathIcon(["M3 17l6-6 4 4 8-8", "M15 7h6v6"]);
const BoxGlyph = makePathIcon([
  "M12 2l8 4.5v11L12 22l-8-4.5v-11L12 2Z",
  "M12 12l8-4.5",
  "M12 12v10",
  "M12 12L4 7.5",
]);
const LinkGlyph = makePathIcon([
  "M10 13a5 5 0 0 0 7.5.5l2-2a5 5 0 0 0-7-7l-1.5 1.5",
  "M14 11a5 5 0 0 0-7.5-.5l-2 2a5 5 0 0 0 7 7l1.5-1.5",
]);
const KeyGlyph = makePathIcon([
  "M15 7a4 4 0 1 1-7.5 2l-4.5 4.5V17h3.5v-2H9v-2.5",
]);
const ServerGlyph = makePathIcon([
  "M4 4h16v6H4z",
  "M4 14h16v6H4z",
  "M8 7h.01",
  "M8 17h.01",
]);
const DownloadGlyph = makePathIcon(["M12 3v12", "M7 10l5 5 5-5", "M4 19h16"]);
const UploadGlyph = makePathIcon(["M12 15V3", "M7 8l5-5 5 5", "M4 19h16"]);
const DashboardGlyph = makePathIcon(["M3 3h7v7H3z", "M14 3h7v4h-7z", "M14 11h7v10h-7z", "M3 14h7v7H3z"]);
const KanbanGlyph = makePathIcon(["M4 4h4v14H4z", "M10 4h4v9h-4z", "M16 4h4v16h-4z"]);
const HashGlyph = makePathIcon(["M4 9h16", "M4 15h16", "M10 3L8 21", "M16 3l-2 18"]);
const MonitorGlyph = makePathIcon(["M3 4h18v12H3z", "M8 20h8", "M12 16v4"]);
const LightbulbGlyph = makePathIcon([
  "M9 18h6",
  "M10 21h4",
  "M12 3a6 6 0 0 0-4 10.5c.8.8 1.2 1.7 1.3 2.5h5.4c.1-.8.5-1.7 1.3-2.5A6 6 0 0 0 12 3Z",
]);
const MoreHorizGlyph = makePathIcon(["M5 12h.01", "M12 12h.01", "M19 12h.01"]);
const ForwardGlyph = makePathIcon(["M15 10l5 5-5 5", "M4 4v7a4 4 0 0 0 4 4h12"]);
const LogOutGlyph = makePathIcon(["M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4", "M16 17l5-5-5-5", "M21 12H9"]);
const TaskGlyph = makePathIcon(["M9 11l2 2 4-4", "M4 5h16v14H4z"]);
const DataGridGlyph = makePathIcon(["M3 3h18v18H3z", "M3 9h18", "M3 15h18", "M9 3v18", "M15 3v18"]);
const ImageGlyph = makePathIcon(["M3 4h18v16H3z", "M8 10a1.5 1.5 0 1 0 0 .01", "M21 16l-5-5-6 6"]);
const SmileGlyph = makePathIcon(["M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z", "M8 14s1.5 2 4 2 4-2 4-2", "M9 9h.01", "M15 9h.01"]);

export const FLAGSHIP_DOMAIN_ICON_COMPONENTS: Record<string, React.ComponentType<CowokIconProps>> = {
  // Aliases to existing icon components (zero duplication with SPECIAL_ICON_RENDERERS)
  "trash2": Trash2Icon,
  "shield-alert": ShieldAlertIcon,
  "help-circle": HelpCircleIcon,
  "copy-text": CopyTextIcon,
  "agent-mkt-brand": AgentMktBrandIcon,
  "ai-evolution": RefreshIcon,
  "corner-up-left": ReplyIcon,
  notification: BellIcon,
  security: LockIcon,
  member: UserIcon,
  "add-member": UsersIcon,
  "human-staff": TeamIcon,
  department: UsersIcon,
  supervisor: UserIcon,
  role: LockIcon,
  project: WorkspaceIcon,
  onboarding: SettingsIcon,
  "jump-message": MessageSquareIcon,
  topic: MessageSquareIcon,
  "review-required": ShieldAlertIcon,
  proposal: ProposalIcon,
  skill: LayersIcon,
  assets: BoxGlyph,
  "switch-workspace": RefreshIcon,

  // Core / System / Data / Ops flagship icons
  database: DatabaseGlyph,
  "data-desk": DatabaseGlyph,
  "schema-topology": DatabaseGlyph,
  "sql-explain": DatabaseGlyph,
  "hard-drive": ServerGlyph,
  server: ServerGlyph,
  file: FileGlyph,
  "file-text": FileTextGlyph,
  "knowledge-base": FileTextGlyph,
  "wiki-save": FileTextGlyph,
  report: FileTextGlyph,
  "sales-deck": FileTextGlyph,
  analytics: AnalyticsGlyph,
  progress: AnalyticsGlyph,
  status: AnalyticsGlyph,
  "trending-up": TrendGlyph,
  box: BoxGlyph,
  zip: BoxGlyph,
  link: LinkGlyph,
  key: KeyGlyph,
  download: DownloadGlyph,
  upload: UploadGlyph,
  dashboard: DashboardGlyph,
  kanban: KanbanGlyph,
  "data-grid": DataGridGlyph,
  "table-pro": DataGridGlyph,
  hash: HashGlyph,
  monitor: MonitorGlyph,
  lightbulb: LightbulbGlyph,
  "auto-fix": LightbulbGlyph,
  "more-horizontal": MoreHorizGlyph,
  forward: ForwardGlyph,
  "log-out": LogOutGlyph,
  task: TaskGlyph,
  reconciliation: TaskGlyph,
  "image-icon": ImageGlyph,
  smile: SmileGlyph,
  commissions: AnalyticsGlyph,
  vouchers: FileTextGlyph,
  subscriptions: CalendarIcon,
  "ticket-discrepancy": ShieldAlertIcon,
  "talent-pool": UsersIcon,
  "token-chip": BoxGlyph,
  "ai-card": AiAgentIcon,
  github: LinkGlyph,
  lark: MessageSquareIcon,
  slack: HashGlyph,
  telegram: MessageSquareIcon,
  whatsapp: MessageSquareIcon,

  // Specialized agent persona icons
  "agent-accountant": makeAgentPersonaIcon("AC"),
  "agent-devops": makeAgentPersonaIcon("DO"),
  "agent-legal": makeAgentPersonaIcon("LG"),
  "agent-mkt-content": makeAgentPersonaIcon("MC"),
  "agent-mkt-copywriter": makeAgentPersonaIcon("CW"),
  "agent-mkt-designer": makeAgentPersonaIcon("MD"),
  "agent-mkt-ecom": makeAgentPersonaIcon("EC"),
  "agent-mkt-kol": makeAgentPersonaIcon("KL"),
  "agent-mkt-ops": makeAgentPersonaIcon("MO"),
  "agent-mkt-trade": makeAgentPersonaIcon("MT"),
  "agent-qa": makeAgentPersonaIcon("QA"),
  "agent-recruiter": makeAgentPersonaIcon("RC"),
  "agent-sales-admin": makeAgentPersonaIcon("SA"),
  "agent-sales-ops": makeAgentPersonaIcon("SO"),
  "agent-sales-reporter": makeAgentPersonaIcon("SR"),
  "agent-sales": makeAgentPersonaIcon("SL"),
};
