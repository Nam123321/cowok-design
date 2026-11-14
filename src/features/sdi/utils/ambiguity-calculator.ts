import { AMBIGUITY_THRESHOLD, ClarificationVariant } from "../../../lib/validations/sdi";

export const BASE_AMBIGUITY = 0.2;
export const VAGUE_PENALTY = 0.35;
export const SCOPE_PENALTY = 0.25;
export const SPECIFIC_BONUS = 0.2;
export const BASE_RISK = 0.15;
export const HIGH_RISK_PENALTY = 0.4;
export const MAX_SCORE = 1.0;
export const MIN_SCORE = 0.0;

const VAGUE_KEYWORDS = ["đẹp", "fix", "sửa", "change", "update", "tối ưu", "cải tiến", "giao diện"];
const SPECIFIC_KEYWORDS = ["node", "screen", "button", "header", "table", "drawer", "input", "auth"];

export interface ScoringResult {
  ambiguityScore: number;
  riskScore: number;
  isAmbiguous: boolean;
}

export function computeAmbiguityAndRisk(rawPrompt: string, targetNodeId?: string): ScoringResult {
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

  const boundedAmbiguity = Math.round(Math.min(MAX_SCORE, Math.max(MIN_SCORE, ambiguity)) * 100) / 100;
  const boundedRisk = Math.round(Math.min(MAX_SCORE, Math.max(MIN_SCORE, risk)) * 100) / 100;

  return {
    ambiguityScore: boundedAmbiguity,
    riskScore: boundedRisk,
    isAmbiguous: boundedAmbiguity >= AMBIGUITY_THRESHOLD,
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
