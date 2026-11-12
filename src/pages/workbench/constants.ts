// @story 04.1
import { WorkbenchNode, WorkbenchTransition } from "../../features/flow/store/flow-workbench-store";
import type { ChatSession } from "./types";

export const DEFAULT_FLOW_PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const COWOK_AI_PROJECT_ID = "22222222-2222-4222-8222-222222222222";
const MARKOPS_PROJECT_ID = "33333333-3333-4333-8333-333333333333";

const POS_SCR_01 = { x: 28, y: 72 };
const POS_SCR_02 = { x: 276, y: 72 };
const POS_SCR_02B = { x: 276, y: 264 };
const POS_SCR_03 = { x: 524, y: 128 };

export const CHAT_DEFAULT_WIDTH = 340;
export const CHAT_COLLAPSED_RAIL_WIDTH = 40;
export const CHAT_MIN_WIDTH = 220;
export const CHAT_MAX_WIDTH = 500;
export const SIDEBAR_WIDTH = 240;
export const MIN_SELECTION_LENGTH = 5;

export const NEW_NODE_BASE_X = 250;
export const NEW_NODE_BASE_Y = 180;
export const NEW_NODE_STEP_X = 160;
export const NEW_NODE_STEP_Y = 100;
export const MODULO_COLS = 3;
export const MODULO_ROWS = 2;

export const DEFAULT_INITIAL_NODES: WorkbenchNode[] = [
  {
    id: "11111111-1111-4111-8111-111111111101",
    screenKey: "SCR-01",
    title: "Màn hình Đăng nhập (Auth)",
    routePath: "/login",
    posX: POS_SCR_01.x,
    posY: POS_SCR_01.y,
    version: 1,
    layoutVersion: 1,
    status: "synced",
    clusterId: "cluster-main",
    note: "Hỗ trợ OTP & Passwordless",
  },
  {
    id: "11111111-1111-4111-8111-111111111102",
    screenKey: "SCR-02",
    title: "Thanh toán Đơn hàng (Checkout)",
    routePath: "/checkout",
    posX: POS_SCR_02.x,
    posY: POS_SCR_02.y,
    version: 1,
    layoutVersion: 1,
    status: "synced",
    clusterId: "cluster-main",
    note: "Tích hợp Stripe & VNPay",
  },
  {
    id: "11111111-1111-4111-8111-111111111103",
    screenKey: "SCR-02B",
    title: "Xác thực 3DS (OTP Challenge)",
    routePath: "/checkout/verify-3ds",
    posX: POS_SCR_02B.x,
    posY: POS_SCR_02B.y,
    version: 1,
    layoutVersion: 1,
    status: "synced",
    clusterId: "cluster-fallback",
    note: "Nhánh rẽ khi ngân hàng yêu cầu 3DS",
  },
  {
    id: "11111111-1111-4111-8111-111111111104",
    screenKey: "SCR-03",
    title: "Hoàn tất Đơn hàng (Success)",
    routePath: "/checkout/success",
    posX: POS_SCR_03.x,
    posY: POS_SCR_03.y,
    version: 1,
    layoutVersion: 1,
    status: "synced",
    clusterId: "cluster-main",
    note: "Hiển thị mã QR và biên nhận",
  },
];

export const DEFAULT_INITIAL_TRANSITIONS: WorkbenchTransition[] = [
  {
    id: "22222222-2222-4222-8222-222222222201",
    sourceNodeId: "11111111-1111-4111-8111-111111111101",
    targetNodeId: "11111111-1111-4111-8111-111111111102",
    triggerEvent: "onClick",
    edgeType: "main",
    conditionLabel: "Next Screen",
    status: "synced",
    version: 1,
  },
  {
    id: "22222222-2222-4222-8222-222222222202",
    sourceNodeId: "11111111-1111-4111-8111-111111111102",
    targetNodeId: "11111111-1111-4111-8111-111111111104",
    triggerEvent: "onSubmit",
    edgeType: "main",
    conditionLabel: "Payment Approved",
    status: "synced",
    version: 1,
  },
  {
    id: "22222222-2222-4222-8222-222222222203",
    sourceNodeId: "11111111-1111-4111-8111-111111111102",
    targetNodeId: "11111111-1111-4111-8111-111111111103",
    triggerEvent: "custom",
    edgeType: "branch",
    conditionLabel: "Requires 3DS OTP",
    status: "synced",
    version: 1,
  },
  {
    id: "22222222-2222-4222-8222-222222222204",
    sourceNodeId: "11111111-1111-4111-8111-111111111103",
    targetNodeId: "11111111-1111-4111-8111-111111111104",
    triggerEvent: "onNavigate",
    edgeType: "main",
    conditionLabel: "OTP Correct",
    status: "synced",
    version: 1,
  },
];

export type ProjectProvenance = "folder-backed" | "scratch";

export interface WorkbenchProject {
  id: string;
  name: string;
  isExpanded?: boolean;
  provenance?: ProjectProvenance;
  localPath?: string;
}

export const DEFAULT_PROJECTS: WorkbenchProject[] = [
  {
    id: DEFAULT_FLOW_PROJECT_ID,
    name: "cowok-design",
    isExpanded: true,
    provenance: "folder-backed",
    localPath: "./projects/uxui-os",
  },
  {
    id: COWOK_AI_PROJECT_ID,
    name: "Cowok.ai",
    isExpanded: true,
    provenance: "folder-backed",
    localPath: "./projects/Cowok-ai",
  },
  {
    id: MARKOPS_PROJECT_ID,
    name: "Markops_platform",
    isExpanded: false,
    provenance: "scratch",
    localPath: "~/.cowok-design/projects/markops",
  },
];

export const DEFAULT_SESSIONS: ChatSession[] = [
  { id: "session-1", title: "Epic 35 — Design System V5", projectId: DEFAULT_FLOW_PROJECT_ID, isPinned: true, updatedAt: "2h" },
  { id: "session-2", title: "Thiết kế Luồng Checkout & 3DS", projectId: DEFAULT_FLOW_PROJECT_ID, isPinned: true, updatedAt: "10:00" },
  { id: "session-3", title: "Tối ưu hóa Token Theme Night", projectId: DEFAULT_FLOW_PROJECT_ID, isPinned: false, updatedAt: "09:30" },
  { id: "session-4", title: "Epic 41, 99, harness 3.3", projectId: COWOK_AI_PROJECT_ID, isPinned: false, updatedAt: "3h" },
  { id: "session-5", title: "Epic 91", projectId: COWOK_AI_PROJECT_ID, isPinned: false, updatedAt: "1d" },
  { id: "session-6", title: "Epic 01, 03", projectId: COWOK_AI_PROJECT_ID, isPinned: false, updatedAt: "2d" },
  { id: "session-7", title: "Pipeline Orchestrator V2", projectId: MARKOPS_PROJECT_ID, isPinned: false, updatedAt: "3d" },
  { id: "session-8", title: "Cấu hình State Hydrated Preview", isPinned: false, updatedAt: "Hôm qua" },
  { id: "session-9", title: "Khảo sát kiến trúc Design Tokens", isPinned: false, updatedAt: "4d" },
];

export function getConditionLabel(edgeType: "main" | "branch" | "fallback"): string {
  if (edgeType === "branch") return "IF Condition";
  if (edgeType === "fallback") return "IF NOT";
  return "Next";
}
