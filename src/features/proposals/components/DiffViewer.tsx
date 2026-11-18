// @story 05.2
import React from "react";
import { parsePatchLines } from "../utils/diff-engine";

const KEY_SNIPPET_LEN = 16;

interface DiffViewerProps {
  patch: string;
}

function getLineClass(type: "added" | "deleted" | "context"): string {
  if (type === "added") return "cp-diff-line cp-diff-line--added";
  if (type === "deleted") return "cp-diff-line cp-diff-line--deleted";
  return "cp-diff-line cp-diff-line--context";
}

function getLinePrefix(type: "added" | "deleted" | "context"): string {
  if (type === "added") return "+";
  if (type === "deleted") return "-";
  return " ";
}

export const DiffViewer: React.FC<DiffViewerProps> = ({ patch }) => {
  const lines = parsePatchLines(patch);

  return (
    <div className="cp-diff-viewer" data-testid="cp-diff-viewer">
      {lines.map((line, idx) => (
        <div key={`${idx}-${line.type}-${line.content.slice(0, KEY_SNIPPET_LEN)}`} className={getLineClass(line.type)}>
          <span className="cp-diff-gutter">{getLinePrefix(line.type)}</span>
          <span className="cp-diff-content">{line.content}</span>
        </div>
      ))}
    </div>
  );
};
