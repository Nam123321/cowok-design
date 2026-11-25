import { useState, useEffect, useCallback, useMemo } from "react";
import viDict from "../i18n/locales/vi.json";
import enDict from "../i18n/locales/en.json";
import zhDict from "../i18n/locales/zh.json";
import jaDict from "../i18n/locales/ja.json";
import koDict from "../i18n/locales/ko.json";
import frDict from "../i18n/locales/fr.json";
import esDict from "../i18n/locales/es.json";
import ptDict from "../i18n/locales/pt.json";
import itDict from "../i18n/locales/it.json";

/**
 * Minimalist reactive i18n hook conforming to cowok.ai 9-locale standards (DESIGN.md 11.3)
 * and Formatting Guardian skill requirements.
 */
const DICTS: Record<string, Record<string, any>> = {
  "vi-VN": viDict,
  vi: viDict,
  "en-US": enDict,
  en: enDict,
  "zh-CN": zhDict,
  zh: zhDict,
  "ja-JP": jaDict,
  ja: jaDict,
  "ko-KR": koDict,
  ko: koDict,
  "fr-FR": frDict,
  fr: frDict,
  "es-ES": esDict,
  es: esDict,
  "pt-PT": ptDict,
  pt: ptDict,
  "it-IT": itDict,
  it: itDict,
};

let currentGlobalLocale = "vi-VN";
const listeners = new Set<(locale: string) => void>();

if (typeof window !== "undefined") {
  const saved = localStorage.getItem("cowok_locale");
  if (saved && (DICTS[saved] || saved.startsWith("en"))) {
    currentGlobalLocale = saved;
  }
}

export function setGlobalLocale(newLocale: string) {
  currentGlobalLocale = newLocale;
  if (typeof window !== "undefined") {
    localStorage.setItem("cowok_locale", newLocale);
  }
  listeners.forEach((listener) => listener(newLocale));
}

export function getGlobalLocale(): string {
  return currentGlobalLocale;
}

function resolveKey(dict: Record<string, any>, key: string): string | undefined {
  const parts = key.split(".");
  let cur: any = dict;
  for (const p of parts) {
    if (p === "__proto__" || p === "prototype" || p === "constructor") {
      return undefined;
    }
    if (cur && typeof cur === "object" && Object.prototype.hasOwnProperty.call(cur, p)) {
      cur = cur[p];
    } else {
      return undefined;
    }
  }
  return typeof cur === "string" ? cur : undefined;
}

function interpolateVars(
  template: string,
  vars?: Record<string, string | number>
): string {
  if (!vars) return template;
  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (match, varKey: string) =>
    Object.prototype.hasOwnProperty.call(vars, varKey) ? String(vars[varKey]) : match
  );
}

export function translateGlobal(
  key: string,
  defaultVal?: string,
  vars?: Record<string, string | number>
): string {
  const dict =
    DICTS[currentGlobalLocale] ||
    (currentGlobalLocale.startsWith("en") ? enDict : viDict);
  const found = resolveKey(dict, key);
  const raw = found !== undefined ? found : defaultVal || key;
  return interpolateVars(raw, vars);
}

export function useTranslation(overrideLocale?: string) {
  const [locale, setLocale] = useState(overrideLocale || currentGlobalLocale);

  useEffect(() => {
    if (overrideLocale) {
      setLocale(overrideLocale);
      return;
    }
    const handler = (newLoc: string) => setLocale(newLoc);
    listeners.add(handler);
    return () => {
      listeners.delete(handler);
    };
  }, [overrideLocale]);

  const dict = useMemo(
    () => DICTS[locale] || (locale.startsWith("en") ? enDict : viDict),
    [locale]
  );

  const t = useCallback(
    (key: string, defaultVal?: string, vars?: Record<string, string | number>) => {
      const found = resolveKey(dict, key);
      const raw = found !== undefined ? found : defaultVal || key;
      return interpolateVars(raw, vars);
    },
    [dict]
  );

  return {
    t,
    locale,
    setLocale: setGlobalLocale,
  };
}
