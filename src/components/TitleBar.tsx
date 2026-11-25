import { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  HardDrive,
  Languages,
  Maximize2,
  Minimize2,
  Minus,
  Monitor,
  Moon,
  Sun,
  X,
} from "lucide-react";
import { api } from "../api/client";
import type { Dict } from "../i18n";
import type { CoreInfo, Locale, RouteMode, ThemeMode, TrafficSample } from "../types";
import { formatSpeed } from "../utils/nodes";
import { BrandLogo } from "./BrandLogo";

interface TitleBarProps {
  dict: Dict;
  locale: Locale;
  theme: ThemeMode;
  coreInfo: CoreInfo;
  routeMode: RouteMode;
  sysProxyEnabled: boolean;
  tunEnabled: boolean;
  traffic: TrafficSample;
  onSelectMode: (mode: RouteMode) => void;
  onToggleSysProxy: () => void;
  onToggleTun: () => void;
  onOpenCore: () => void;
  onToggleLocale: () => void;
  onCycleTheme: () => void;
}

export function TitleBar({
  dict,
  locale,
  theme,
  coreInfo,
  routeMode,
  sysProxyEnabled,
  tunEnabled,
  traffic,
  onSelectMode,
  onToggleSysProxy,
  onToggleTun,
  onOpenCore,
  onToggleLocale,
  onCycleTheme,
}: TitleBarProps) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const sync = () => {
      api.windowControls.isMaximized().then(setMaximized).catch(() => {});
    };
    sync();
    let unlisten: (() => void) | undefined;
    void api.windowControls.onResized(sync).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const coreLabel = !coreInfo.installed
    ? dict.titlebar.coreMissing
    : coreInfo.version ?? dict.titlebar.coreStopped;

  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 items-center justify-between gap-2 bg-[var(--bg-chrome)] pr-1.5 pl-3.5 select-none"
    >
      {/* Left: Brand & Core Badge */}
      <div data-tauri-drag-region className="flex min-w-0 items-center gap-2">
        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[7px] border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--text-primary)]">
          <BrandLogo className="h-[15px] w-[15px]" />
        </span>
        <span
          data-tauri-drag-region
          className="text-[13px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]"
        >
          {dict.appTitle}
        </span>

        <button
          type="button"
          onClick={onOpenCore}
          className="inline-flex h-6 items-center gap-1.5 rounded-full border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2 text-[11px] font-medium text-[var(--text-secondary)] transition-colors hover:border-[var(--border-strong)] hover:text-[var(--text-primary)] cursor-pointer"
        >
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{
              backgroundColor: coreInfo.running
                ? "var(--success-dot)"
                : coreInfo.installed
                  ? "var(--warning-dot)"
                  : "var(--text-tertiary)",
            }}
          />
          <span className="font-mono num-tabular">{coreLabel}</span>
        </button>

        {coreInfo.portableMode && (
          <span className="hidden xl:inline-flex items-center gap-1 rounded-full bg-[var(--bg-hover)] px-2 py-0.5 text-[10.5px] font-medium text-[var(--text-secondary)]">
            <HardDrive className="h-2.5 w-2.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            {dict.titlebar.portable}
          </span>
        )}
      </div>

      {/* Center: Mode Switcher + Proxy & TUN Pills */}
      <div className="flex items-center gap-1.5">
        <div
          className="inline-flex items-center rounded-[9px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] p-0.5 text-[11.5px]"
          role="group"
        >
          {(["rule", "global", "direct"] as RouteMode[]).map((m) => {
            const active = routeMode === m;
            const label =
              m === "rule"
                ? dict.titlebar.modeRule
                : m === "global"
                  ? dict.titlebar.modeGlobal
                  : dict.titlebar.modeDirect;
            return (
              <button
                key={m}
                type="button"
                onClick={() => onSelectMode(m)}
                className={`rounded-[6px] px-2.5 py-0.5 font-medium transition-colors cursor-pointer ${
                  active
                    ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>

        <button
          type="button"
          onClick={onToggleSysProxy}
          className={`inline-flex h-7 items-center gap-1.5 rounded-[8px] border px-2.5 text-[11.5px] font-medium transition-colors cursor-pointer ${
            sysProxyEnabled
              ? "border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--text-primary)]"
              : "border-[var(--border-hairline)] bg-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          }`}
        >
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{
              backgroundColor: sysProxyEnabled ? "var(--success-dot)" : "var(--text-tertiary)",
            }}
          />
          <span>{dict.titlebar.sysProxy}</span>
        </button>

        <button
          type="button"
          onClick={onToggleTun}
          className={`inline-flex h-7 items-center gap-1.5 rounded-[8px] border px-2.5 text-[11.5px] font-medium transition-colors cursor-pointer ${
            tunEnabled
              ? "border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--text-primary)]"
              : "border-[var(--border-hairline)] bg-transparent text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          }`}
        >
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{
              backgroundColor: tunEnabled ? "var(--success-dot)" : "var(--text-tertiary)",
            }}
          />
          <span>{dict.titlebar.tun}</span>
        </button>
      </div>

      {/* Right: Throughput + Locale + Theme + Window Controls */}
      <div className="flex items-center gap-0.5">
        <div className="mr-2 hidden lg:flex items-center gap-2.5 font-mono text-[11px] num-tabular text-[var(--text-secondary)]">
          <span className="inline-flex items-center gap-0.5">
            <ArrowDown className="h-3 w-3" style={{ color: "var(--success-text)" }} strokeWidth={1.6} />
            {formatSpeed(traffic.downBytesPerSec)}
          </span>
          <span className="inline-flex items-center gap-0.5">
            <ArrowUp className="h-3 w-3 text-[var(--text-tertiary)]" strokeWidth={1.6} />
            {formatSpeed(traffic.upBytesPerSec)}
          </span>
        </div>

        <button
          type="button"
          onClick={onToggleLocale}
          title={dict.titlebar.switchLang}
          className="ui-btn-ghost h-7 px-2 text-[11.5px]"
        >
          <Languages className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          <span className="font-mono num-tabular">{locale === "zh-CN" ? "ZH" : "EN"}</span>
        </button>

        <button
          type="button"
          onClick={onCycleTheme}
          title={dict.titlebar.switchTheme}
          aria-label={dict.titlebar.switchTheme}
          className="ui-btn-ghost h-7 w-7 rounded-full p-0"
        >
          {theme === "dark" ? (
            <Moon className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          ) : theme === "light" ? (
            <Sun className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          ) : (
            <Monitor className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          )}
        </button>

        <div className="mx-1 h-3.5 w-px bg-[var(--border-hairline)]" />

        <button
          type="button"
          onClick={() => api.windowControls.minimize()}
          aria-label={dict.titlebar.minimize}
          className="inline-flex h-7 w-8 items-center justify-center rounded-[8px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
        >
          <Minus className="h-3.5 w-3.5" strokeWidth={1.5} />
        </button>
        <button
          type="button"
          onClick={async () => {
            await api.windowControls.toggleMaximize();
            setMaximized(await api.windowControls.isMaximized());
          }}
          aria-label={dict.titlebar.maximize}
          className="inline-flex h-7 w-8 items-center justify-center rounded-[8px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
        >
          {maximized ? (
            <Minimize2 className="h-3.5 w-3.5" strokeWidth={1.5} />
          ) : (
            <Maximize2 className="h-3.5 w-3.5" strokeWidth={1.5} />
          )}
        </button>
        <button
          type="button"
          onClick={() => api.windowControls.close()}
          aria-label={dict.titlebar.close}
          className="inline-flex h-7 w-8 items-center justify-center rounded-[8px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--danger-bg-hover)] hover:text-[var(--danger-text)] cursor-pointer"
        >
          <X className="h-3.5 w-3.5" strokeWidth={1.5} />
        </button>
      </div>
    </header>
  );
}
