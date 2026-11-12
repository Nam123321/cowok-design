import { useState, useEffect } from "react";
import viDict from "../i18n/locales/vi.json";
import enDict from "../i18n/locales/en.json";

/**
 * Minimalist reactive i18n hook conforming to cowok.ai 9-locale standards (DESIGN.md 11.3)
 * and Formatting Guardian skill requirements.
 */
const DICTS: Record<string, Record<string, any>> = {
  "vi-VN": viDict,
  vi: viDict,
  "en-US": enDict,
  en: enDict,
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
    if (cur && typeof cur === "object" && p in cur) {
      cur = cur[p];
    } else {
      return undefined;
    }
  }
  return typeof cur === "string" ? cur : undefined;
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

  const dict = DICTS[locale] || (locale.startsWith("en") ? enDict : viDict);

  return {
    t: (key: string, defaultVal?: string) => {
      const found = resolveKey(dict, key);
      return found !== undefined ? found : defaultVal || key;
    },
    locale,
    setLocale: setGlobalLocale,
  };
}
