import type { ComponentType, SVGProps } from "react";
import * as Flags from "country-flag-icons/react/3x2";

type FlagComponent = ComponentType<SVGProps<SVGSVGElement>>;
const FLAG_MAP = Flags as unknown as Record<string, FlagComponent | undefined>;

const REGIONAL_INDICATOR_RE = /[\u{1F1E6}-\u{1F1FF}]{2}/gu;

/**
 * Extracts ISO alpha-2 country code from any Unicode Regional Indicator flag pair
 * (e.g. US, JP, HK, AU) and strips broken Windows flag glyph pairs from display text.
 */
export function parseFlagText(
  text: string,
  fallbackRegion?: string | null
): { isoCode: string | null; cleanText: string } {
  let detected: string | null = null;
  const matches = text.match(REGIONAL_INDICATOR_RE);
  if (matches && matches.length > 0) {
    const first = Array.from(matches[0]);
    if (first.length === 2) {
      const c1 = first[0].codePointAt(0);
      const c2 = first[1].codePointAt(0);
      if (c1 !== undefined && c2 !== undefined) {
        detected = String.fromCharCode(c1 - 0x1f1e6 + 65, c2 - 0x1f1e6 + 65);
      }
    }
  }
  const cleanText = text
    .replace(REGIONAL_INDICATOR_RE, "")
    .replace(/\s{2,}/g, " ")
    .trim();

  const isoCode = normalizeIsoCode(detected ?? fallbackRegion);
  return {
    isoCode,
    cleanText: cleanText || text.trim(),
  };
}

export function normalizeIsoCode(raw?: string | null): string | null {
  if (!raw) return null;
  const upper = raw.trim().toUpperCase();
  if (!upper || upper === "OTHER" || upper === "ALL") return null;
  const mapped = upper === "UK" ? "GB" : upper;
  if (/^[A-Z]{2}$/.test(mapped)) {
    return mapped;
  }
  return null;
}

interface RegionFlagProps {
  code?: string | null;
  size?: "xs" | "sm" | "md";
  className?: string;
}

export function RegionFlag({ code, size = "md", className = "" }: RegionFlagProps) {
  const iso = normalizeIsoCode(code);
  if (!iso) return null;

  const sizeClass =
    size === "xs"
      ? "h-[10px] w-[15px]"
      : size === "sm"
        ? "h-[11px] w-[16px]"
        : "h-[12px] w-[18px]";

  const FlagSvg = FLAG_MAP[iso];
  if (FlagSvg) {
    return (
      <span
        aria-hidden="true"
        className={`inline-flex shrink-0 items-center justify-center overflow-hidden rounded-[2px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] ${sizeClass} ${className}`}
      >
        <FlagSvg className="h-full w-full object-cover" />
      </span>
    );
  }

  return (
    <span
      aria-hidden="true"
      className={`inline-flex shrink-0 items-center justify-center rounded-[3px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1 font-mono text-[9.5px] leading-none text-[var(--text-secondary)] ${className}`}
    >
      {iso}
    </span>
  );
}

interface NodeLabelProps {
  remark: string;
  region?: string | null;
  size?: "xs" | "sm" | "md";
  className?: string;
}

export function NodeLabel({
  remark,
  region,
  size = "md",
  className = "",
}: NodeLabelProps) {
  const { isoCode, cleanText } = parseFlagText(remark, region);
  return (
    <span className={`inline-flex min-w-0 items-center gap-1.5 ${className}`}>
      {isoCode && <RegionFlag code={isoCode} size={size} />}
      <span className="truncate">{cleanText}</span>
    </span>
  );
}
