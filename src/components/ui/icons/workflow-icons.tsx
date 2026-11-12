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

export const useWorkflowIconTranslation = useTranslation;

export function WorkflowIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3.7" y="5.2" width="5.4" height="4.2" rx="1.7" /><rect x="14.9" y="5.2" width="5.4" height="4.2" rx="1.7" /><rect x="9.3" y="14.8" width="5.4" height="4.2" rx="1.7" /><path d="M6.4 9.4v1.6c0 1.1.9 2 2 2h7.2c1.1 0 2-.9 2-2V9.4M12 13v1.8" /></svg>;
}

export function WorkspaceIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3.8" y="4.5" width="16.4" height="15" rx="3" /><path d="M3.8 9.2h16.4M8.4 9.2v10.3" /></svg>;
}

export function ApprovalIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M12 3.3 5.3 6v4.8c0 4.3 2.4 7.3 6.7 9.8 4.3-2.5 6.7-5.5 6.7-9.8V6z" /><path d="m8.8 12.1 2.1 2.1 4.5-4.5" /></svg>;
}

export function ConnectorIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M8.5 8.4 6.8 6.7a2.8 2.8 0 0 0-4 4l2.4 2.4a2.8 2.8 0 0 0 4 0l1-1m7 1.5 1.7 1.7a2.8 2.8 0 0 0 4-4l-2.4-2.4a2.8 2.8 0 0 0-4 0l-1 1M8.5 15.5l7-7" /></svg>;
}

export function SharedCowokIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="8.2" cy="7.3" r="2.4" /><circle cx="16.2" cy="8.2" r="2.1" /><path d="M3.9 19.7c.6-3.4 2.1-5.1 4.6-5.1 2.1 0 3.6 1.1 4.4 3.3M12.3 19.5c.5-2.7 1.8-4.1 4.1-4.1 2.2 0 3.5 1.4 4 4.1M10.1 10.1c1.4 1.1 2.8 1.3 4.2.5" /></svg>;
}

export function ActivityIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M3.5 13h4l2-5 3.2 9 2.1-5H20.5" /></svg>;
}

export function ExecutionPipelineIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="5" cy="12" r="1.8" /><rect x="9.1" y="9.2" width="5.8" height="5.6" rx="1.8" /><circle cx="19" cy="12" r="1.8" /><path d="M6.8 12h2.3M14.9 12h2.3" /></svg>;
}

export function HandoffIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="6.8" cy="7" r="2.2" /><circle cx="17.2" cy="17" r="2.2" /><path d="M9.1 8.2c3 1 4.5 2.2 5.6 4.7M14.6 6.2H18v3.4M9.4 17.8H6v-3.4" /></svg>;
}

export function IntegrationIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3.8" y="7" width="6.4" height="10" rx="2.2" /><rect x="13.8" y="7" width="6.4" height="10" rx="2.2" /><path d="M10.2 10h3.6M10.2 14h3.6" /></svg>;
}

export function AutomationIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="5" y="5" width="14" height="14" rx="3.3" /><path d="m10.2 9.1 4.7 2.9-4.7 2.9zM8 3.7h8" /></svg>;
}

export function UserIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="12" cy="8" r="4" /><path d="M5 20c0-4 4-6 7-6s7 2 7 6" /></svg>;
}

export function UsersIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" /></svg>;
}

export function TeamIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="8.3" cy="8" r="2.3" /><circle cx="15.6" cy="8.6" r="2" /><path d="M3.7 19.6c.6-3.4 2.3-5.1 4.9-5.1 2.7 0 4.3 1.7 5 5.1M13.7 15.1c2.8-.2 4.9 1.3 5.7 4.5" /></svg>;
}

export function BellIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M18 16H6c1.5-1.5 2-3.5 2-6a4 4 0 0 1 8 0c0 2.5.5 4.5 2 6zM10 19c.5 1 1 1.5 2 1.5s1.5-.5 2-1.5" /></svg>;
}

export function LockIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect width="18" height="11" x="3" y="11" rx="2" ry="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" /></svg>;
}

export function CalendarIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="5" y="5" width="14" height="15" rx="2" /><path d="M8 3v4M16 3v4M5 9h14" /></svg>;
}

export const WORKFLOW_ICON_COMPONENTS = {
  workflow: WorkflowIcon,
  workspace: WorkspaceIcon,
  canvas: WorkspaceIcon,
  approval: ApprovalIcon,
  connector: ConnectorIcon,
  plug: ConnectorIcon,
  "shared-cowok": SharedCowokIcon,
  activity: ActivityIcon,
  pipeline: ExecutionPipelineIcon,
  "execution-pipeline": ExecutionPipelineIcon,
  handoff: HandoffIcon,
  integration: IntegrationIcon,
  automation: AutomationIcon,
  automations: AutomationIcon,
  user: UserIcon,
  users: UsersIcon,
  team: TeamIcon,
  bell: BellIcon,
  lock: LockIcon,
  calendar: CalendarIcon,
};
