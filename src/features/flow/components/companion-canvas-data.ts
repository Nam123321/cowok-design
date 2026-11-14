// @story-04.1
// @implements AC-3
import type { WorkbenchNode } from "../store/flow-workbench-types";
import type { CanvasTabItem } from "./CompanionCanvasTab";

export interface SlidePhaseItem {
  id: string;
  code: string;
  title: string;
  headline: string;
  subtitle: string;
  kpi1: number;
  kpi2: string;
  kpi3: string;
}

export interface ScreenPreviewData {
  eyebrow: string;
  headline: string;
  subtitle: string;
  amount: number;
  statusText: string;
  latency: string;
  actionTitle: string;
  ctaLabel: string;
  kpi1Label?: string;
  kpi1Display?: string;
  kpi2Label?: string;
  kpi3Label?: string;
  actionHint?: string;
}

export interface SynthPreviewLabels {
  kpiRoute?: string;
  kpiState?: string;
  kpiType?: string;
  actionHint?: string;
  ctaOpen?: string;
}

const AMOUNT_SCR_01 = 4850000;
const AMOUNT_SCR_02 = 1284500;
const AMOUNT_SLIDE_01 = 1284500;
const AMOUNT_SLIDE_03 = 960000;

export const DEFAULT_TABS: CanvasTabItem[] = [
  {
    id: "tab-userflow",
    title: "checkout-app.html",
    iconName: "globe",
    format: "dag",
    defaultLens: "userflow",
    filePath: "src/pages/checkout-app.html",
    isClosable: false,
  },
  {
    id: "tab-slide",
    title: "q4-architecture-deck.html",
    iconName: "layers",
    format: "slide-cms",
    defaultLens: "slide-cms",
    filePath: "presentations/q4-architecture-deck.html",
    isClosable: true,
  },
  {
    id: "tab-prd",
    title: "DESIGN.md",
    iconName: "wiki",
    format: "md",
    filePath: "DESIGN.md",
    isClosable: true,
  },
  {
    id: "tab-code",
    title: "CheckoutForm.tsx",
    iconName: "code",
    format: "code",
    filePath: "src/components/CheckoutForm.tsx",
    isClosable: true,
  },
];

export const INITIAL_SLIDES: SlidePhaseItem[] = [
  {
    id: "sld-01",
    code: "SLIDE 01 • HERO",
    title: "Kiến trúc Điều phối cowok-design v5.4",
    headline: "Kiến trúc Điều phối cowok-design v5.4 — Dual-Pane Studio",
    subtitle: "Kết hợp Userflow (2D DAG) + Slide Deck CMS ở khung trái và Live HTML Preview ở khung phải.",
    kpi1: AMOUNT_SLIDE_01,
    kpi2: "99.4% SAFE",
    kpi3: "12.4 ms",
  },
  {
    id: "sld-02",
    code: "SLIDE 02 • ARCHITECTURE",
    title: "Hybrid Project Boundary & OpenDesign Sync",
    headline: "Đồng bộ 2 chiều Folder-Backed Git & Managed Scratch",
    subtitle: "1 Dự án chia sẻ DESIGN.md, tokens.css và đồ thị Userflow xuyên suốt N phiên hội thoại.",
    kpi1: AMOUNT_SCR_01,
    kpi2: "100% SSOT",
    kpi3: "8.2 ms",
  },
  {
    id: "sld-03",
    code: "SLIDE 03 • BENCHMARK",
    title: "Tournament 3 Phương án UI & Zero-Drift AST",
    headline: "So sánh trực tiếp Option A / B / C trên Live HTML Stage",
    subtitle: "Tự động kiểm tra E1..E4 và niêm phong giao diện không lệch token.",
    kpi1: AMOUNT_SLIDE_03,
    kpi2: "9.6 / 10",
    kpi3: "14.0 ms",
  },
];

export const SCREEN_PREVIEW_MAP: Record<string, ScreenPreviewData> = {
  "node-scr-01": {
    eyebrow: "SCR-01 • ENTRY SCREEN (/login)",
    headline: "Đăng nhập Kho bạc & Kiểm tra Danh mục Ủy thác",
    subtitle: "Xác thực danh tính đa nhân tố trước khi mở khóa luồng ký giao dịch.",
    amount: AMOUNT_SCR_01,
    statusText: "3 Lệnh Chờ",
    latency: "9.8 ms",
    actionTitle: "Xác thực Phiên Kho bạc (Passkey / SSO)",
    ctaLabel: "Tiếp tục sang Thanh toán (SCR-02) →",
  },
  "node-scr-02": {
    eyebrow: "SCR-02 • DECISION NODE (/checkout • Option B Winner)",
    headline: "Xác thực Ủy thác Giao dịch Kho bạc Đa Chữ Ký",
    subtitle: "Tối ưu hóa xác thực khóa cứng WebAuthn Passkey ngay trên một mặt phẳng duy nhất.",
    amount: AMOUNT_SCR_02,
    statusText: "99.4% SAFE",
    latency: "12.4 ms",
    actionTitle: "Xác nhận Chữ ký Sinh trắc học (Hardware Passkey)",
    ctaLabel: "Ký & Giải ngân Ngay",
  },
  "node-scr-02b": {
    eyebrow: "SCR-02B • FALLBACK BRANCH (/checkout/verify-3ds)",
    headline: "Xác thực Dự phòng 3DS OTP & Phê duyệt Giám đốc",
    subtitle: "Kích hoạt tự động khi WebAuthn thất bại hoặc giao dịch vượt hạn mức.",
    amount: AMOUNT_SCR_02,
    statusText: "FALLBACK OTP",
    latency: "18.2 ms",
    actionTitle: "Nhập mã 3DS OTP 6 số từ Thiết bị Bảo mật",
    ctaLabel: "Xác nhận OTP & Hội tụ về SCR-03",
  },
  "node-scr-03": {
    eyebrow: "SCR-03 • TERMINAL CONVERGENCE (/checkout/success)",
    headline: "Biên nhận Ủy thác Đã Niêm phong Sổ cái",
    subtitle: "Giao dịch đã hội tụ thành công và được xác thực bởi 3/3 chữ ký phần cứng.",
    amount: AMOUNT_SCR_02,
    statusText: "100% SEALED",
    latency: "6.4 ms",
    actionTitle: "Biên bản Nghiệm thu Giao dịch Hoàn tất",
    ctaLabel: "Tải Biên nhận PDF / JSON",
  },
};

const SCREEN_KEY_TO_PREVIEW_ID: Record<string, string> = {
  "SCR-01": "node-scr-01",
  "SCR-02": "node-scr-02",
  "SCR-02B": "node-scr-02b",
  "SCR-03": "node-scr-03",
  "11111111-1111-4111-8111-111111111101": "node-scr-01",
  "11111111-1111-4111-8111-111111111102": "node-scr-02",
  "11111111-1111-4111-8111-111111111103": "node-scr-02b",
  "11111111-1111-4111-8111-111111111104": "node-scr-03",
};

export function synthesizeFallbackScreenPreview(
  node: Pick<WorkbenchNode, "screenKey" | "title" | "routePath" | "status" | "note" | "nodeType" | "activeSubState">,
  labels?: SynthPreviewLabels
): ScreenPreviewData {
  const route = node.routePath || "/";
  const subState = node.activeSubState || "DEFAULT";
  const nodeType = (node.nodeType || "screen").toUpperCase();
  const statusUpper = (node.status || "draft").toUpperCase();
  return {
    eyebrow: `${node.screenKey} • ${nodeType} (${route})`,
    headline: node.title,
    subtitle: node.note?.trim() || `${nodeType} • ${route} • ${subState}`,
    amount: 0,
    kpi1Label: labels?.kpiRoute || "Route",
    kpi1Display: route,
    kpi2Label: labels?.kpiState || "Node Status",
    statusText: `${statusUpper} • ${subState}`,
    kpi3Label: labels?.kpiType || "Node Type",
    latency: nodeType,
    actionTitle: `${node.title} (${node.screenKey})`,
    actionHint: node.note?.trim() || labels?.actionHint || "Synthesized live preview from WorkbenchNode metadata.",
    ctaLabel: `${labels?.ctaOpen || "Open"} ${route}`,
  };
}

export function resolveScreenPreviewData(
  selectedNode?: WorkbenchNode | null,
  selectedNodeId?: string | null,
  labels?: SynthPreviewLabels
): ScreenPreviewData {
  if (selectedNode) {
    const normalizedKey = selectedNode.screenKey.trim().toUpperCase();
    const mappedId = SCREEN_KEY_TO_PREVIEW_ID[normalizedKey];
    if (mappedId && SCREEN_PREVIEW_MAP[mappedId]) {
      return SCREEN_PREVIEW_MAP[mappedId];
    }
    return synthesizeFallbackScreenPreview(selectedNode, labels);
  }

  if (selectedNodeId) {
    if (SCREEN_PREVIEW_MAP[selectedNodeId]) {
      return SCREEN_PREVIEW_MAP[selectedNodeId];
    }
    const normalized = selectedNodeId.trim().toUpperCase();
    const mappedByKey = SCREEN_KEY_TO_PREVIEW_ID[normalized] || SCREEN_KEY_TO_PREVIEW_ID[selectedNodeId];
    if (mappedByKey && SCREEN_PREVIEW_MAP[mappedByKey]) {
      return SCREEN_PREVIEW_MAP[mappedByKey];
    }
    return synthesizeFallbackScreenPreview(
      {
        screenKey: selectedNodeId,
        title: selectedNodeId,
        routePath: "/",
        status: "draft",
      },
      labels
    );
  }

  return SCREEN_PREVIEW_MAP["node-scr-02"];
}

export function createCanvasTabByKind(
  kind: "app-flow" | "slide-deck" | "md" | "assets" | "code",
  tabsCount: number,
  customTitle?: string
): CanvasTabItem {
  const id = `tab-${Date.now()}`;
  const presetMap: Record<string, Omit<CanvasTabItem, "id">> = {
    "app-flow": { title: customTitle || "multi-sig-flow.html", iconName: "globe", format: "dag", defaultLens: "userflow", isClosable: true },
    "slide-deck": { title: customTitle || "series-a-deck.html", iconName: "layers", format: "slide-cms", defaultLens: "slide-cms", isClosable: true },
    assets: { title: customTitle || "brand-symbol.svg", iconName: "image", format: "assets", filePath: "src/assets/logo/03-symbol/cowok-symbol-color-2048.png", isClosable: true },
    code: { title: customTitle || "tokens.css", iconName: "code", format: "code", isClosable: true },
    md: { title: customTitle || `spec-${tabsCount + 1}.md`, iconName: "wiki", format: "md", isClosable: true },
  };
  return { id, ...(presetMap[kind] || presetMap.md) };
}
