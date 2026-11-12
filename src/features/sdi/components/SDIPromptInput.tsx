import React, { useState } from "react";
import { apiSdiEngine } from "../../../api/sdi-engine";
import { DesignIntentResponse } from "../../../lib/validations/sdi";

const PROMPT_ROWS = 3;

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
  const [prompt, setPrompt] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!prompt.trim() || isLoading) return;

    setIsLoading(true);
    try {
      const intent = await apiSdiEngine.compileIntent(projectId, {
        rawPrompt: prompt,
        targetNodeId,
      });
      onIntentCompiled(intent);
      setPrompt("");
    } catch (err) {
      console.error("Failed to compile intent:", err);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-2">
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        placeholder="Mô tả thay đổi bằng ngôn ngữ tự nhiên..."
        rows={PROMPT_ROWS}
        className="w-full text-xs p-2.5 rounded-md border border-border bg-background resize-none focus:outline-none focus:ring-1 focus:ring-primary"
      />
      <button
        type="submit"
        disabled={!prompt.trim() || isLoading}
        className="self-end px-3 py-1.5 text-xs font-semibold rounded-md bg-primary text-primary-foreground disabled:opacity-50"
      >
        {isLoading ? "Đang phân tích..." : "Biên Dịch SDI"}
      </button>
    </form>
  );
};
