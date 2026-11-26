// @story 13.1
// @implements AC-04, AC-05 (Non-Tech Localized Copy Mappers — Zero Raw Enum/Code Leakage)
import { EvalCertificationApiError } from "../../api/eval-certification";
import type {
  DecisionProviderId,
  EvalMilestoneId,
  EvalTier,
  EvalVerdict,
  ManifestPinSource,
  SimCaseResult,
  SimCaseStatus,
} from "../../lib/validations/eval-certification";

export type TranslateFn = (
  key: string,
  defaultVal?: string,
  vars?: Record<string, string | number>
) => string;

const MS_PER_SECOND = 1_000;
const SHA_PREVIEW_LEN = 12;

export function formatEvalDuration(
  durationMs: number,
  formatNumber: (val: number, decimals?: number) => string,
  t: TranslateFn
): string {
  if (durationMs < MS_PER_SECOND) {
    return t("evalCertification.durationMsUnit", "{{value}} ms", {
      value: formatNumber(durationMs, 0),
    });
  }
  return t("evalCertification.durationSecUnit", "{{value}} s", {
    value: formatNumber(durationMs / MS_PER_SECOND, 1),
  });
}

export function formatShaPreview(sha256: string): string {
  const clean = sha256.trim();
  if (clean.length <= SHA_PREVIEW_LEN) return clean;
  return `${clean.slice(0, SHA_PREVIEW_LEN)}…`;
}

export function getTierLabel(tier: EvalTier, t: TranslateFn): string {
  if (tier === "pr") return t("evalCertification.tierPrTitle", "Kiểm tra nhanh");
  if (tier === "stage") return t("evalCertification.tierStageTitle", "Đánh giá đối chứng");
  return t("evalCertification.tierReleaseTitle", "Chứng nhận phát hành");
}

export function getTierSubtitle(tier: EvalTier, t: TranslateFn): string {
  if (tier === "pr") return t("evalCertification.tierPrSubtitle", "Ngoại tuyến · <15 giây · $0");
  if (tier === "stage") {
    return t("evalCertification.tierStageSubtitle", "Đối chứng 12+ kịch bản & SemVer");
  }
  return t("evalCertification.tierReleaseSubtitle", "Đầy đủ 25 hành trình & niêm phong");
}

export function getVerdictLabel(verdict: EvalVerdict, t: TranslateFn): string {
  if (verdict === "PASS") return t("evalCertification.verdictPass", "Đạt chứng nhận");
  if (verdict === "SEALED_EVAL_TAMPER_ERROR") {
    return t("evalCertification.verdictTamper", "Khóa do thay đổi tiêu chuẩn");
  }
  return t("evalCertification.verdictFail", "Chưa đạt chuẩn");
}

export function getSimCaseStatusLabel(status: SimCaseStatus, t: TranslateFn): string {
  if (status === "PASS") return t("evalCertification.statusPass", "Đạt");
  if (status === "BLOCKED") return t("evalCertification.statusBlocked", "Bị chặn");
  if (status === "GAP") return t("evalCertification.statusGap", "Thiếu bước");
  return t("evalCertification.statusFail", "Chưa đạt");
}

export function getMilestoneLabel(milestone: EvalMilestoneId, t: TranslateFn): string {
  const map: Record<EvalMilestoneId, [string, string]> = {
    M1: ["evalCertification.milestoneM1", "Nền tảng & Bảo mật khóa"],
    M2: ["evalCertification.milestoneM2", "Không gian Thiết kế & Trình bày"],
    M3: ["evalCertification.milestoneM3", "Tiện ích & An toàn Thực thi"],
    M4: ["evalCertification.milestoneM4", "Định tuyến & Đấu trường Thiết kế"],
    M5: ["evalCertification.milestoneM5", "Đóng gói & Phát hành Độc lập"],
  };
  const [key, fallback] = map[milestone];
  return t(key, fallback);
}

export function getMilestoneDescription(milestone: EvalMilestoneId, t: TranslateFn): string {
  const map: Record<EvalMilestoneId, [string, string]> = {
    M1: [
      "evalCertification.milestoneDesc_M1",
      "Khởi tạo dự án, giới hạn ngữ cảnh, tự động che khóa bí mật, cách ly dự án và chuẩn token giao diện.",
    ],
    M2: [
      "evalCertification.milestoneDesc_M2",
      "Luồng người dùng, cập nhật nguyên tử, nhánh khôi phục, bộ trình chiếu 16:9 và xuất bản gốc.",
    ],
    M3: [
      "evalCertification.milestoneDesc_M3",
      "Kiểm tra đối chứng kỹ năng, an toàn xem trước, cô lập công cụ thực thi và rà soát chất lượng độc lập.",
    ],
    M4: [
      "evalCertification.milestoneDesc_M4",
      "Lưu trữ an toàn, khóa ghi chống xung đột, cổng tiện ích theo chính sách và hiệu chuẩn mô hình định tuyến.",
    ],
    M5: [
      "evalCertification.milestoneDesc_M5",
      "Đồng bộ CLI, ứng dụng desktop, bộ cài đa nền tảng, chạy ngoại tuyến nhanh và bảo vệ bộ tiêu chuẩn niêm phong.",
    ],
  };
  const [key, fallback] = map[milestone];
  return t(key, fallback);
}

export function getPinSourceLabel(source: ManifestPinSource, t: TranslateFn): string {
  if (source === "ci_env_override") {
    return t("evalCertification.pinSourceCi", "Khóa đối chiếu môi trường kiểm định");
  }
  return t("evalCertification.pinSourceBinary", "Khóa đối chiếu tích hợp sẵn");
}

export function getProviderLabel(providerId: DecisionProviderId, t: TranslateFn): string {
  if (providerId === "local_fts5") {
    return t("evalCertification.providerLocalSearch", "Tìm kiếm Ngữ cảnh Cục bộ");
  }
  if (providerId === "gemini_decision_jev") {
    return t("evalCertification.providerDecisionModel", "Mô hình Quyết định Định danh");
  }
  return t("evalCertification.providerDeterministicMcda", "Chấm điểm Đa tiêu chí Tất định");
}

const SECURITY_GATE_COPY_MAP: Record<string, [string, string]> = {
  M1_SECRET_INTERCEPT: [
    "evalCertification.secGateM1Secret",
    "Chặn rò rỉ khóa bí mật trước khi lưu trữ",
  ],
  M2_CSP_XSS_ZERO_SPLIT_BRAIN: [
    "evalCertification.secGateM2Xss",
    "Làm sạch nội dung xem trước & đồng bộ trạng thái duy nhất",
  ],
  M3_RCE_TOOL_JAIL: [
    "evalCertification.secGateM3ToolJail",
    "Cách ly thực thi công cụ & chặn truy cập ngoài phạm vi",
  ],
  M4_ZERO_FLOAT_LEAK: [
    "evalCertification.secGateM4FloatLeak",
    "Không rò rỉ điểm xác suất thô từ bộ định tuyến",
  ],
  M5_ZERO_OFFLINE_404: [
    "evalCertification.secGateM5Offline",
    "Bản xuất ngoại tuyến không lỗi thiếu tài nguyên",
  ],
  M5_SEALED_TAMPER_GUARD: [
    "evalCertification.secGateM5Sealed",
    "Bảo vệ nguyên vẹn bộ tiêu chuẩn niêm phong",
  ],
};

export function getSecurityGateLabel(gateKey: string, t: TranslateFn): string {
  const normalized = gateKey.trim().toUpperCase();
  const segments = normalized
    .split(":")
    .map((part) => part.trim())
    .filter(Boolean);
  for (const candidate of [normalized, ...segments]) {
    const exact = SECURITY_GATE_COPY_MAP[candidate];
    if (exact) return t(exact[0], exact[1]);
  }
  const partial = Object.entries(SECURITY_GATE_COPY_MAP).find(([key]) =>
    normalized.includes(key)
  );
  if (partial) return t(partial[1][0], partial[1][1]);
  return t("evalCertification.secGateGeneric", "Cổng an toàn & toàn vẹn bắt buộc");
}

export function getSimCaseEvidenceSummary(
  item: SimCaseResult,
  t: TranslateFn,
  formatNumber: (val: number, decimals?: number) => string
): string {
  const duration = formatEvalDuration(item.durationMs, formatNumber, t);
  if (item.status === "BLOCKED") {
    return t(
      "evalCertification.evidenceBlocked",
      "Bị chặn bởi điều kiện tiên quyết hoặc chốt an toàn đang bật."
    );
  }
  if (item.status === "GAP") {
    return t(
      "evalCertification.evidenceGap",
      "Phát hiện thiếu bước kiểm chứng trong hành trình này."
    );
  }
  if (item.mandatorySecurityGate && item.securityGateKey) {
    const gate = getSecurityGateLabel(item.securityGateKey, t);
    if (item.status === "PASS") {
      return t(
        "evalCertification.evidenceSecurityPass",
        "Đạt cổng bảo mật bắt buộc: {{gate}} ({{duration}}).",
        { gate, duration }
      );
    }
    return t(
      "evalCertification.evidenceSecurityFail",
      "Chưa đạt cổng bảo mật bắt buộc: {{gate}}.",
      { gate }
    );
  }
  if (item.status === "PASS") {
    return t(
      "evalCertification.evidencePass",
      "Đã kiểm chứng tất định cho {{milestone}} trong {{duration}}.",
      { milestone: getMilestoneLabel(item.milestone, t), duration }
    );
  }
  return t(
    "evalCertification.evidenceFail",
    "Chưa đạt tiêu chuẩn kiểm chứng yêu cầu ({{duration}}).",
    { duration }
  );
}

export function getTamperReasonMessage(reason: string | null | undefined, t: TranslateFn): string {
  const normalized = (reason || "").trim().toUpperCase();
  if (normalized.includes("SYMLINK")) {
    return t(
      "evalCertification.tamperReasonSymlink",
      "Phát hiện liên kết ngoài phạm vi trong thư mục tiêu chuẩn niêm phong."
    );
  }
  if (normalized.includes("WRITABLE")) {
    return t(
      "evalCertification.tamperReasonWritable",
      "Phát hiện tệp tiêu chuẩn đang mở quyền chỉnh sửa thay vì chỉ đọc."
    );
  }
  if (normalized.includes("UNEXPECTED")) {
    return t(
      "evalCertification.tamperReasonUnexpected",
      "Phát hiện tệp lạ nằm trong thư mục tiêu chuẩn niêm phong."
    );
  }
  if (normalized.includes("SHA") || normalized.includes("HASH") || normalized.includes("MISMATCH")) {
    return t(
      "evalCertification.tamperReasonHash",
      "Dấu vân tay toàn vẹn của bộ tiêu chuẩn không khớp với khóa gốc."
    );
  }
  return t(
    "evalCertification.tamperBannerDesc",
    "Phát hiện thay đổi trái phép trên bộ tiêu chuẩn niêm phong. Chứng nhận đã tự động khóa an toàn để bảo vệ tính khách quan."
  );
}

export function getSimCaseTitle(simCase: SimCaseResult, t: TranslateFn): string {
  const caseKey = `evalCertification.caseTitle_${simCase.caseId}`;
  const resolvedById = t(caseKey, "");
  if (resolvedById && resolvedById !== caseKey) return resolvedById;
  if (simCase.titleKey.startsWith("evalCertification.")) {
    const resolvedByKey = t(simCase.titleKey, "");
    if (resolvedByKey && resolvedByKey !== simCase.titleKey) return resolvedByKey;
  }
  return t("evalCertification.caseTitleFallback", "Hành trình người dùng {{id}}", {
    id: simCase.caseId,
  });
}

export function formatEvalErrorMessage(err: unknown, t: TranslateFn): string {
  if (err instanceof EvalCertificationApiError) {
    if (err.isAlreadyRunning) {
      return t(
        "evalCertification.errAlreadyRunning",
        "Đang có một phiên kiểm định chạy song song. Vui lòng đợi hoàn tất."
      );
    }
    if (err.isTamperError) {
      return t(
        "evalCertification.errTamperLocked",
        "Phát hiện thay đổi trái phép trên bộ tiêu chuẩn niêm phong. Chứng nhận bị khóa an toàn."
      );
    }
    if (err.status === 401) {
      return t("evalCertification.errUnauthorized", "Phiên làm việc đã hết hạn. Vui lòng tải lại.");
    }
    if (err.status === 403) {
      return t(
        "evalCertification.errForbidden",
        "Tài khoản hiện tại không có quyền thực hiện thao tác chứng nhận này."
      );
    }
    if (err.status === 404) {
      return t(
        "evalCertification.errNotFound",
        "Không tìm thấy dữ liệu dự án hoặc dịch vụ chứng nhận."
      );
    }
  }
  return t(
    "evalCertification.errGeneric",
    "Không thể hoàn tất yêu cầu chứng nhận lúc này. Vui lòng thử lại."
  );
}
