// @story 05.1
// @implements Zero-Tailwind Semantic Pure CSS SDI Prompt Input
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { apiConstraints } from "../../../api/constraints";
import { apiSdiEngine } from "../../../api/sdi-engine";
import { useTranslation } from "../../../hooks/useTranslation";
import { DesignIntentResponse } from "../../../lib/validations/sdi";

const PROMPT_ROWS = 3;
const DISABLED_OPACITY = 0.5;
const ICON_SIZE_WARN = 11;
const ICON_SIZE_SUBMIT = 12;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;

const TEXTAREA_STYLE: React.CSSProperties = {
  width: "100%",
  fontSize: "12px",
  padding: "10px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-canvas)",
  color: "var(--text-primary)",
  resize: "none",
  outline: "none",
  boxSizing: "border-box",
  fontFamily: "var(--font-sans)",
};

const ERROR_BANNER_STYLE: React.CSSProperties = {
  fontSize: "11px",
  color: "var(--state-danger)",
  display: "flex",
  alignItems: "center",
  gap: "4px",
};

const SUBMIT_BASE_STYLE: React.CSSProperties = {
  alignSelf: "flex-end",
  padding: "6px 14px",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  borderRadius: "var(--radius-xs)",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
};

interface SDIPromptInputProps {
  projectId: string;
  targetNodeId?: string;
  onIntentCompiled: (intent: DesignIntentResponse) => void;
}

export const SDIPromptInput: React.FC<SDIPromptInputProps> = ({
  projectId,
  targetNodeId,
  onIntentCompiled,
}) => {
  const { t } = useTranslation();
  const [prompt, setPrompt] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const isDisabled = !prompt.trim() || isLoading;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isDisabled) return;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      await apiConstraints.getDesignContracts(projectId).catch(() => null);
      const intent = await apiSdiEngine.compileIntent(projectId, { rawPrompt: prompt, targetNodeId });
      onIntentCompiled(intent);
      setPrompt("");
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : t("sdi.compileErrorFallback", "Không thể biên dịch Semantic Design Intent."));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        placeholder={t("sdi.promptPlaceholder", "Mô tả thay đổi bằng ngôn ngữ tự nhiên...")}
        rows={PROMPT_ROWS}
        style={TEXTAREA_STYLE}
      />
      {errorMessage && (
        <div role="alert" style={ERROR_BANNER_STYLE}>
          <BrandIcon name="warning" size={ICON_SIZE_WARN} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
          <span>{errorMessage}</span>
        </div>
      )}
      <button type="submit" disabled={isDisabled} style={{ ...SUBMIT_BASE_STYLE, cursor: isDisabled ? "not-allowed" : "pointer", opacity: isDisabled ? DISABLED_OPACITY : 1 }}>
        <BrandIcon name={isLoading ? "loader" : "sparkles"} size={ICON_SIZE_SUBMIT} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>{isLoading ? t("sdi.analyzing", "Đang phân tích...") : t("sdi.compileButton", "Biên Dịch SDI")}</span>
      </button>
    </form>
  );
};
