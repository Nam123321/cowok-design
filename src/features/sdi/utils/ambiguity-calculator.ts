// @story 05.1
// @implements Context/Topology-Aware Ambiguity Scoring & Gap-Driven Discovery Engine
import { AMBIGUITY_THRESHOLD, ClarificationVariant } from "../../../lib/validations/sdi";

export const BASE_AMBIGUITY = 0.2;
export const VAGUE_PENALTY = 0.35;
export const SCOPE_PENALTY = 0.25;
export const SPECIFIC_BONUS = 0.2;
export const TOPOLOGY_GROUNDED_BONUS = 0.1;
export const BASE_RISK = 0.15;
export const HIGH_RISK_PENALTY = 0.4;
export const MAX_SCORE = 1.0;
export const MIN_SCORE = 0.0;
const MAX_GAP_QUESTIONS = 3;
const COMPLEX_PROJECT_RISK_BONUS = 0.1;

const VAGUE_KEYWORDS = [
  "đẹp",
  "fix",
  "sửa",
  "change",
  "update",
  "tối ưu",
  "cải tiến",
  "giao diện",
  "make it look better",
  "clean up",
  "redesign",
  "modernize",
  "tùy ý",
  "sao cũng được",
];

const SPECIFIC_KEYWORDS = [
  "node",
  "screen",
  "button",
  "header",
  "table",
  "drawer",
  "input",
  "auth",
  "layout",
  "modal",
  "card",
  "filter",
  "navigation",
  "kpi",
  "substate",
  "viewport",
  "token",
];

export interface TopologyContext {
  nodeCount?: number;
  edgeCount?: number;
  hasActiveSelection?: boolean;
  clusterCount?: number;
  targetComponentType?: string;
  projectComplexity?: "simple" | "medium" | "complex";
}

export interface VisualDiscoveryOption {
  id: string;
  label: string;
  previewBadge?: string;
}

export interface VisualDiscoveryQuestion {
  id: string;
  question: string;
  hint: string;
  options: VisualDiscoveryOption[];
}

export interface ScoringResult {
  ambiguityScore: number;
  riskScore: number;
  isAmbiguous: boolean;
  routingMode: "direct_edit" | "full_plan";
  questions: VisualDiscoveryQuestion[];
}

export function generateGapDiscoveryQuestions(
  rawPrompt: string,
  context?: TopologyContext
): VisualDiscoveryQuestion[] {
  const lower = rawPrompt.toLowerCase();
  const questions: VisualDiscoveryQuestion[] = [];

  // Question 1: Scope & Hierarchy Gap
  if (!lower.includes("screen") && !lower.includes("global") && !lower.includes("node")) {
    questions.push({
      id: "q-scope",
      question: "Phạm vi thay đổi cấu trúc UI/UX mong muốn?",
      hint: "Xác định giới hạn ảnh hưởng của thay đổi lên cây component.",
      options: [
        { id: "opt-local", label: "Cục bộ trên Node đang chọn", previewBadge: "Direct Edit" },
        { id: "opt-cluster", label: "Toàn bộ cụm tính năng (Cluster)", previewBadge: "Flow-Wide" },
        { id: "opt-appshell", label: "Khung điều hướng chung (AppShell)", previewBadge: "Global" },
      ],
    });
  }

  // Question 2: Sub-State & Fixture Requirement
  if (!lower.includes("loading") && !lower.includes("empty") && !lower.includes("error")) {
    questions.push({
      id: "q-substate",
      question: "Cần xử lý các trạng thái phụ (Sub-States) nào?",
      hint: "Chọn trạng thái để đồng bộ qua IframeKeepAlivePool.",
      options: [
        { id: "opt-default-only", label: "Chỉ Default State", previewBadge: "Default" },
        { id: "opt-full-matrix", label: "Đủ 4 Sub-States (Default, Loading, Empty, Error)", previewBadge: "Option 5 Matrix" },
        { id: "opt-error-emphasis", label: "Tập trung kịch bản lỗi (ValidationError)", previewBadge: "Resilience" },
      ],
    });
  }

  // Question 3: Routing & Execution Plan
  if (questions.length < MAX_GAP_QUESTIONS) {
    questions.push({
      id: "q-routing",
      question: "Phương thức thực thi đề xuất?",
      hint: "Lựa chọn áp dụng trực tiếp hay tạo kế hoạch phân rã chi tiết.",
      options: [
        { id: "opt-direct", label: "Cập nhật trực tiếp <16ms (Direct In-Place)", previewBadge: "Fast Path" },
        { id: "opt-plan", label: "Sinh kế hoạch thẩm định đầy đủ (Full Plan Gate)", previewBadge: "Safe Gate" },
      ],
    });
  }

  return questions.slice(0, MAX_GAP_QUESTIONS);
}

export function computeAmbiguityAndRisk(
  rawPrompt: string,
  targetNodeId?: string,
  context?: TopologyContext
): ScoringResult {
  const lower = rawPrompt.toLowerCase();
  let ambiguity = BASE_AMBIGUITY;
  let risk = BASE_RISK;

  const hasVague = VAGUE_KEYWORDS.some((kw) => lower.includes(kw));
  if (hasVague) ambiguity += VAGUE_PENALTY;

  const hasSpecific = SPECIFIC_KEYWORDS.some((kw) => lower.includes(kw)) || Boolean(targetNodeId);
  if (hasSpecific) {
    ambiguity = Math.max(MIN_SCORE, ambiguity - SPECIFIC_BONUS);
  } else {
    ambiguity += SCOPE_PENALTY;
    risk += HIGH_RISK_PENALTY;
  }

  // Topology-aware context grounding
  if (context?.hasActiveSelection || Boolean(targetNodeId)) {
    ambiguity = Math.max(MIN_SCORE, ambiguity - TOPOLOGY_GROUNDED_BONUS);
  }
  if (context?.projectComplexity === "complex") {
    risk += COMPLEX_PROJECT_RISK_BONUS;
  }

  const boundedAmbiguity = Math.round(Math.min(MAX_SCORE, Math.max(MIN_SCORE, ambiguity)) * 100) / 100;
  const boundedRisk = Math.round(Math.min(MAX_SCORE, Math.max(MIN_SCORE, risk)) * 100) / 100;
  const isAmbiguous = boundedAmbiguity >= AMBIGUITY_THRESHOLD;

  const routingMode: "direct_edit" | "full_plan" =
    !isAmbiguous && (Boolean(targetNodeId) || Boolean(context?.hasActiveSelection))
      ? "direct_edit"
      : "full_plan";

  const questions = generateGapDiscoveryQuestions(rawPrompt, context);

  return {
    ambiguityScore: boundedAmbiguity,
    riskScore: boundedRisk,
    isAmbiguous,
    routingMode,
    questions,
  };
}

export function generateClarificationVariants(promptSummary: string): ClarificationVariant[] {
  return [
    {
      id: "var-1-minimal",
      title: "Phương án 1: Cục bộ & Tối giản (Minimal)",
      description: `Áp dụng thay đổi giới hạn ngay trên thành phần hiện tại cho '${promptSummary}'.`,
      pros: ["Thời gian thực thi tức thì (<100ms)", "Không ảnh hưởng đến luồng điều hướng khác"],
      cons: ["Không mở rộng cấu trúc dữ liệu cho tương lai"],
    },
    {
      id: "var-2-balanced",
      title: "Phương án 2: Tiêu chuẩn Cân bằng (Balanced)",
      description: `Điều chỉnh bố cục theo mẫu chuẩn DESIGN.md và đồng bộ trạng thái giao diện cho '${promptSummary}'.`,
      pros: ["Đồng bộ chuẩn token thiết kế 100%", "Bảo toàn khả năng tái sử dụng component"],
      cons: ["Cần kiểm định lại các viewport tablet và mobile"],
    },
    {
      id: "var-3-modular",
      title: "Phương án 3: Mô-đun hóa Toàn diện (Modular)",
      description: `Tách thành component con độc lập kèm quản lý trạng thái riêng cho '${promptSummary}'.`,
      pros: ["Khả năng mở rộng tối đa", "Dễ dàng viết unit test độc lập"],
      cons: ["Gia tăng số lượng file mã nguồn trong dự án"],
    },
  ];
}
