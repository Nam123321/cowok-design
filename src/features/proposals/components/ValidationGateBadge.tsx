import React from "react";
import { ValidationCheckResult } from "../../../lib/validations/change-proposal";

interface ValidationGateBadgeProps {
  checks: ValidationCheckResult[];
}

function getStatusBadge(status: "passed" | "failed" | "skipped") {
  if (status === "passed") {
    return {
      classes: "bg-emerald-500/10 text-emerald-600 border-emerald-500/20",
      icon: "✓",
    };
  }
  if (status === "failed") {
    return {
      classes: "bg-rose-500/10 text-rose-600 border-rose-500/20",
      icon: "✕",
    };
  }
  return {
    classes: "bg-secondary text-secondary-foreground border-border",
    icon: "•",
  };
}

export const ValidationGateBadge: React.FC<ValidationGateBadgeProps> = ({ checks }) => {
  return (
    <div className="flex flex-col gap-1.5 p-3 rounded-lg border border-border bg-card">
      <span className="text-xs font-semibold text-foreground">Cổng Kiểm Tra Tự Động (Validation Gate)</span>
      <div className="flex flex-col gap-1">
        {checks.map((c) => {
          const badge = getStatusBadge(c.status);
          return (
            <div key={c.checkType} className="flex items-center justify-between text-xs py-0.5">
              <span className="text-muted-foreground uppercase font-mono">{c.checkType}</span>
              <span className={`px-2 py-0.5 rounded text-[11px] border font-medium flex items-center gap-1 ${badge.classes}`}>
                <span>{badge.icon}</span>
                <span>{c.status}</span>
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
};
