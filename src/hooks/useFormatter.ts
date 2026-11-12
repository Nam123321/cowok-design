/**
 * cowok-design — Canonical Formatting & Localization Hook (Formatting Guardian Compliance)
 * Enforces the 3-level hierarchy: Personal > Workspace > Default
 * Zero raw .toLocaleString() / .toLocaleTimeString() inside UI components.
 */

export interface FormatterOptions {
  locale?: string;
  currency?: string;
  timezone?: string;
}

export function useFormatter(options?: FormatterOptions) {
  const locale = options?.locale || "vi-VN";
  const currency = options?.currency || "USD";

  const formatNumber = (val: number, decimals = 0): string => {
    try {
      return new Intl.NumberFormat(locale, {
        minimumFractionDigits: decimals,
        maximumFractionDigits: decimals,
      }).format(val);
    } catch {
      return String(val);
    }
  };

  const formatCurrency = (val: number, curr = currency): string => {
    try {
      return new Intl.NumberFormat(locale, {
        style: "currency",
        currency: curr,
        minimumFractionDigits: curr === "VND" ? 0 : 2,
      }).format(val);
    } catch {
      return `${curr} ${val}`;
    }
  };

  const formatTime = (dateInput?: Date | number | string): string => {
    try {
      const d = dateInput ? new Date(dateInput) : new Date();
      return new Intl.DateTimeFormat(locale, {
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }).format(d);
    } catch {
      return "10:00";
    }
  };

  const formatDate = (dateInput?: Date | number | string): string => {
    try {
      const d = dateInput ? new Date(dateInput) : new Date();
      return new Intl.DateTimeFormat(locale, {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
      }).format(d);
    } catch {
      return "";
    }
  };

  const formatUnit = (val: number, unitKey: string): string => {
    return `${formatNumber(val)} ${unitKey}`;
  };

  return {
    locale,
    currency,
    formatNumber,
    formatCurrency,
    formatTime,
    formatDate,
    formatUnit,
  };
}
