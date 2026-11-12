import React from "react";
import { parsePatchLines } from "../utils/diff-engine";

interface DiffViewerProps {
  patch: string;
}

function getLineClasses(type: "added" | "deleted" | "context"): string {
  if (type === "added") return "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400";
  if (type === "deleted") return "bg-rose-500/10 text-rose-600 dark:text-rose-400";
  return "text-muted-foreground";
}

function getLinePrefix(type: "added" | "deleted" | "context"): string {
  if (type === "added") return "+";
  if (type === "deleted") return "-";
  return " ";
}

export const DiffViewer: React.FC<DiffViewerProps> = ({ patch }) => {
  const lines = parsePatchLines(patch);

  return (
    <div className="font-mono text-xs rounded-md bg-muted/40 p-2 overflow-x-auto border border-border">
      {lines.map((l, idx) => (
        <div key={`${idx}-${l.content}`} className={`flex px-1 py-0.5 rounded ${getLineClasses(l.type)}`}>
          <span className="w-4 select-none opacity-50">{getLinePrefix(l.type)}</span>
          <span className="flex-1 whitespace-pre">{l.content}</span>
        </div>
      ))}
    </div>
  );
};
