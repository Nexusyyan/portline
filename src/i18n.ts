import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import type { Locale } from "./types";

export type Dict = typeof en;

const DICTS: Record<Locale, Dict> = {
  en,
  "zh-CN": zhCN,
};

const LOCALE_STORAGE_KEY = "portline.locale";

export function detectInitialLocale(): Locale {
  try {
    const saved = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    if (saved === "zh-CN" || saved === "en") {
      return saved;
    }
  } catch {
    // ignore storage access issues
  }
  const navLang = typeof navigator !== "undefined" ? navigator.language : "en";
  return navLang.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function persistLocale(locale: Locale): void {
  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // ignore
  }
}

export function getDict(locale: Locale): Dict {
  return DICTS[locale] ?? en;
}
