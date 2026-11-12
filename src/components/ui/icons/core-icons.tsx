// @story 04.1
import * as React from "react";
import { useTranslation } from "../../../hooks/useTranslation";

export type CowokIconProps = React.SVGProps<SVGSVGElement> & { size?: number; strokeWidth?: number };

const base = {
  fill: "none",
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const useCoreIconTranslation = useTranslation;

export function PinIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.5c0-1.5-1.5-3-1.5-3v-6a4.5 4.5 0 0 0-9 0v6c0 0-1.5 1.5-1.5 3V17z" /><line x1="9" y1="5" x2="15" y2="5" /></svg>;
}

export function PinOffIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><line x1="2" y1="2" x2="22" y2="22" /><line x1="12" y1="17" x2="12" y2="22" /><path d="M9 9v3H5v1.5c0 1.5 1.5 3 1.5 3h6.5" /><path d="M15 9.34V5h4h-3.34" /><line x1="9" y1="5" x2="15" y2="5" /></svg>;
}

export function SettingsIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="12" cy="12" r="3" /><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1" /></svg>;
}

export function GlobeIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 9h18" /><path d="M6 6.5h.01M9 6.5h.01M12 6.5h.01" /></svg>;
}

export function DeployIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M 12 3 C 13.5 6 14.5 10 14.5 16 H 9.5 C 9.5 10 10.5 6 12 3 Z" /><circle cx="12" cy="9.5" r="1.5" /><path d="M 9.5 12 L 6 15.5 V 18 H 9.5" /><path d="M 14.5 12 L 18 15.5 V 18 H 14.5" /><path d="M 10.5 16 C 10.5 18.5 12 20.5 12 20.5 C 12 20.5 13.5 18.5 13.5 16" /><path d="M 12 16 V 19" /></svg>;
}

export function PlusIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>;
}

export function Trash2Icon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M10 11v6" /><path d="M14 11v6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" /><path d="M3 6h18" /><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>;
}

export function MessageSquareIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></svg>;
}

export function ChatIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M4 5h16v10H9l-5 5z" /><path d="M8 9h8M8 12h5" /></svg>;
}

export function SendIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z" /><path d="m21.854 2.147-10.94 10.939" /></svg>;
}

export function SearchIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="m21 21-4.34-4.34" /><circle cx="11" cy="11" r="8" /></svg>;
}

export function MapIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M8 6L21 3 21 18 8 21 3 18 3 3 8 6z" /><path d="M8 6v15" /><path d="M15 3v15" /></svg>;
}

export function CheckCircleIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M21.801 10A10 10 0 1 1 17 3.335" /><path d="m9 11 3 3L22 4" /></svg>;
}

export function ShieldAlertIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" /><path d="M12 8v4" /><path d="M12 16h.01" /></svg>;
}

export function HelpCircleIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><circle cx="12" cy="12" r="10" /><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" /><line x1="12" y1="17" x2="12.01" y2="17" /></svg>;
}

export function EditIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>;
}

export function CopyTextIcon({ size = 24, strokeWidth = 1.8, ...props }: CowokIconProps) {
  return <svg viewBox="0 0 24 24" width={size} height={size} {...base} strokeWidth={strokeWidth} {...props}><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>;
}

export const CORE_ICON_COMPONENTS = {
  pin: PinIcon,
  "pin-off": PinOffIcon,
  pinned: PinIcon,
  settings: SettingsIcon,
  setting: SettingsIcon,
  general: SettingsIcon,
  globe: GlobeIcon,
  language: GlobeIcon,
  deploy: DeployIcon,
  rocket: DeployIcon,
  plus: PlusIcon,
  add: PlusIcon,
  trash: Trash2Icon,
  delete: Trash2Icon,
  remove: Trash2Icon,
  message: MessageSquareIcon,
  "message-square": MessageSquareIcon,
  chat: ChatIcon,
  send: SendIcon,
  search: SearchIcon,
  map: MapIcon,
  minimap: MapIcon,
  radar: MapIcon,
  check: CheckCircleIcon,
  "check-circle": CheckCircleIcon,
  success: CheckCircleIcon,
  alert: ShieldAlertIcon,
  warning: ShieldAlertIcon,
  error: ShieldAlertIcon,
  info: HelpCircleIcon,
  help: HelpCircleIcon,
  edit: EditIcon,
  copy: CopyTextIcon,
};
