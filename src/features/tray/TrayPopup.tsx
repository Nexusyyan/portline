import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  Copy,
  ExternalLink,
  Play,
  Power,
  ShieldCheck,
  Square,
} from "lucide-react";
import { api } from "../../api/client";
import { BrandLogo } from "../../components/BrandLogo";
import { NodeLabel } from "../../components/RegionFlag";
import { detectInitialLocale, getDict } from "../../i18n";
import type {
  AppSettings,
  CoreInfo,
  Locale,
  NodeIR,
  RouteMode,
  ThemeMode,
  TrafficSample,
} from "../../types";
import { delayColorVar, formatSpeed, PROTO_BADGES } from "../../utils/nodes";

const THEME_STORAGE_KEY = "portline.theme";

function applyThemeClass(theme: ThemeMode) {
  const dark =
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function TrayPopup() {
  const [locale, setLocale] = useState<Locale>(detectInitialLocale);
  const [coreInfo, setCoreInfo] = useState<CoreInfo | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [nodes, setNodes] = useState<NodeIR[]>([]);
  const [traffic, setTraffic] = useState<TrafficSample>({
    upBytesPerSec: 0,
    downBytesPerSec: 0,
    memoryBytes: 0,
    activeConnections: 0,
  });
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  const dict = useMemo(() => getDict(locale), [locale]);

  const refreshAll = useCallback(async () => {
    if (!api.isDesktop()) return;
    try {
      const data = await api.getBootstrapState();
      setCoreInfo(data.coreInfo);
      setSettings(data.settings);
      setNodes(data.nodes);
      const nextLoc: Locale = data.settings.locale === "en" ? "en" : "zh-CN";
      setLocale(nextLoc);
      const nextTheme = (data.settings.theme as ThemeMode) || "system";
      applyThemeClass(nextTheme);
    } catch {
      // ignore transient startup read
    }
  }, []);

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(THEME_STORAGE_KEY) as ThemeMode | null;
      applyThemeClass(saved ?? "system");
    } catch {
      applyThemeClass("system");
    }
    void refreshAll();
    let unlisten: (() => void) | undefined;
    void api.onStateSync(() => void refreshAll()).then((fn) => {
      unlisten = fn;
    });
    const timer = window.setInterval(async () => {
      if (!api.isDesktop()) return;
      try {
        const snap = await api.getLiveMonitor();
        setTraffic(snap.traffic);
        setCoreInfo((prev) =>
          prev ? { ...prev, running: snap.coreRunning } : prev
        );
      } catch {
        // ignore
      }
    }, 1500);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.hideTrayPopup();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      if (unlisten) unlisten();
      window.clearInterval(timer);
      window.removeEventListener("keydown", onKey);
    };
  }, [refreshAll]);

  const mainNode = useMemo(() => {
    if (!settings) return nodes[0] ?? null;
    return nodes.find((n) => n.id === settings.mainNodeId) ?? nodes[0] ?? null;
  }, [nodes, settings]);

  const running = coreInfo?.running ?? false;
  const routeMode: RouteMode = settings?.routeMode ?? "rule";
  const sysProxy = settings?.systemProxyEnabled ?? false;
  const tun = settings?.tunEnabled ?? false;
  const mixedPort = settings?.mixedPort ?? 10808;
  const delay = mainNode ? (mainNode.realDelayMs ?? mainNode.tcpingMs ?? null) : null;
  const delayKind = mainNode?.realDelayMs != null ? "real" : "tcping";

  const handleToggleCore = async () => {
    if (busy || !coreInfo?.installed) return;
    setBusy(true);
    try {
      if (running) {
        const info = await api.stopCore();
        setCoreInfo(info);
      } else {
        const [info] = await api.startCore();
        setCoreInfo(info);
      }
    } finally {
      setBusy(false);
    }
  };

  const handleMode = async (m: RouteMode) => {
    setSettings((prev) => (prev ? { ...prev, routeMode: m } : prev));
    try {
      await api.setRouteMode(m);
    } catch {
      // ignore
    }
  };

  const handleSysProxy = async () => {
    const next = !sysProxy;
    setSettings((prev) => (prev ? { ...prev, systemProxyEnabled: next } : prev));
    try {
      const info = await api.toggleSystemProxy(next);
      setCoreInfo(info);
    } catch {
      // ignore
    }
  };

  const handleTun = async () => {
    const next = !tun;
    setSettings((prev) => (prev ? { ...prev, tunEnabled: next } : prev));
    try {
      await api.toggleTun(next);
    } catch {
      // ignore
    }
  };

  const handleCopyProxy = () => {
    const url = `http://127.0.0.1:${mixedPort}`;
    if (navigator.clipboard) {
      void navigator.clipboard.writeText(url).catch(() => {});
    }
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="flex h-screen w-screen flex-col justify-between overflow-hidden border border-[var(--border-strong)] bg-[var(--bg-chrome)] p-3 select-none">
      {/* Header */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <span className="flex h-7 w-7 items-center justify-center rounded-[8px] border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--text-primary)]">
              <BrandLogo className="h-4 w-4" />
            </span>
            <div>
              <div className="text-[13px] font-semibold leading-tight text-[var(--text-primary)]">
                Portline
              </div>
              <div className="flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
                <span
                  className="h-1.5 w-1.5 rounded-full"
                  style={{
                    backgroundColor: running ? "var(--success-dot)" : "var(--text-tertiary)",
                  }}
                />
                <span>{running ? dict.tray.statusRunning : dict.tray.statusStopped}</span>
                {coreInfo?.version && (
                  <span className="font-mono text-[10.5px] text-[var(--text-tertiary)]">
                    {coreInfo.version}
                  </span>
                )}
              </div>
            </div>
          </div>

          <button
            type="button"
            disabled={busy || !coreInfo?.installed}
            onClick={() => void handleToggleCore()}
            className={running ? "ui-btn-secondary h-7 px-2.5 text-[11.5px]" : "ui-btn-primary h-7 px-2.5 text-[11.5px]"}
          >
            {running ? (
              <>
                <Square className="h-3 w-3" strokeWidth={1.6} />
                {dict.nav.stopCore}
              </>
            ) : (
              <>
                <Play className="h-3 w-3" strokeWidth={1.6} />
                {dict.nav.startCore}
              </>
            )}
          </button>
        </div>

        {/* Active Node + Live Speed Card */}
        <div className="rounded-[11px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] p-2.5 space-y-2">
          <div className="flex items-center justify-between text-[11px] text-[var(--text-secondary)]">
            <span>{dict.tray.currentNode}</span>
            <span className="font-mono num-tabular text-[var(--text-tertiary)]">
              :{mixedPort}
            </span>
          </div>

          <div className="flex items-center justify-between gap-2">
            <div className="min-w-0 flex-1 text-[12.5px] font-medium text-[var(--text-primary)]">
              {mainNode ? (
                <NodeLabel remark={mainNode.remark} region={mainNode.region} size="sm" />
              ) : (
                <span className="text-[var(--text-tertiary)]">{dict.tray.noNode}</span>
              )}
            </div>
            {mainNode && (
              <span
                className="shrink-0 font-mono text-[11px] font-medium num-tabular"
                style={{ color: delayColorVar(delay, delayKind) }}
              >
                {delay != null ? `${delay} ms` : "--"}
              </span>
            )}
          </div>

          <div className="flex items-center justify-between border-t border-[var(--border-hairline)] pt-2 font-mono text-[11px] num-tabular text-[var(--text-secondary)]">
            <span className="inline-flex items-center gap-1">
              <ArrowDown className="h-3 w-3" style={{ color: "var(--success-text)" }} strokeWidth={1.6} />
              {formatSpeed(traffic.downBytesPerSec)}
            </span>
            <span className="inline-flex items-center gap-1">
              <ArrowUp className="h-3 w-3 text-[var(--text-tertiary)]" strokeWidth={1.6} />
              {formatSpeed(traffic.upBytesPerSec)}
            </span>
            {mainNode && (
              <span className="rounded-[4px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 text-[10px]">
                {PROTO_BADGES[mainNode.protocol]}
              </span>
            )}
          </div>
        </div>

        {/* Mode Segmented Control */}
        <div className="grid grid-cols-3 gap-1 rounded-[9px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] p-1 text-[11.5px]">
          {(
            [
              ["rule", dict.tray.modeRule],
              ["global", dict.tray.modeGlobal],
              ["direct", dict.tray.modeDirect],
            ] as const
          ).map(([m, label]) => {
            const active = routeMode === m;
            return (
              <button
                key={m}
                type="button"
                onClick={() => void handleMode(m)}
                className={`rounded-[6px] py-1 font-medium transition-colors cursor-pointer ${
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

        {/* Quick Switches */}
        <div className="rounded-[11px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] divide-y divide-[var(--border-hairline)]">
          <div className="flex items-center justify-between px-3 py-2 text-[12px]">
            <span className="font-medium text-[var(--text-primary)]">
              {dict.tray.sysProxy}
            </span>
            <button
              type="button"
              role="switch"
              aria-checked={sysProxy}
              onClick={() => void handleSysProxy()}
              className={`relative h-[18px] w-8 rounded-full transition-colors cursor-pointer ${
                sysProxy
                  ? "bg-[var(--accent-primary)]"
                  : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
              }`}
            >
              <span
                className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                style={{ left: sysProxy ? "16px" : "2px" }}
              />
            </button>
          </div>
          <div className="flex items-center justify-between px-3 py-2 text-[12px]">
            <span className="font-medium text-[var(--text-primary)]">{dict.tray.tun}</span>
            <button
              type="button"
              role="switch"
              aria-checked={tun}
              onClick={() => void handleTun()}
              className={`relative h-[18px] w-8 rounded-full transition-colors cursor-pointer ${
                tun
                  ? "bg-[var(--accent-primary)]"
                  : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
              }`}
            >
              <span
                className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                style={{ left: tun ? "16px" : "2px" }}
              />
            </button>
          </div>
        </div>
      </div>

      {/* Bottom Action Menu */}
      <div className="space-y-1 border-t border-[var(--border-hairline)] pt-2 text-[12px]">
        <button
          type="button"
          onClick={() => void api.showMainWindow()}
          className="flex w-full items-center justify-between rounded-[8px] px-2.5 py-1.5 text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
        >
          <span className="inline-flex items-center gap-2">
            <ExternalLink className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            {dict.tray.openMain}
          </span>
        </button>

        <button
          type="button"
          onClick={handleCopyProxy}
          className="flex w-full items-center justify-between rounded-[8px] px-2.5 py-1.5 text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
        >
          <span className="inline-flex items-center gap-2">
            {copied ? (
              <Check className="h-3.5 w-3.5" style={{ color: "var(--success-text)" }} strokeWidth={1.6} />
            ) : (
              <Copy className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            )}
            {copied ? dict.tray.copied : dict.tray.copyProxy}
          </span>
          <span className="font-mono text-[10.5px] text-[var(--text-tertiary)] num-tabular">
            :{mixedPort}
          </span>
        </button>

        <button
          type="button"
          onClick={async () => {
            await api.restoreNetguardNow().catch(() => {});
            await refreshAll();
          }}
          className="flex w-full items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
        >
          <ShieldCheck className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          {dict.tray.restoreNetguard}
        </button>

        <button
          type="button"
          onClick={() => void api.quitApp()}
          className="flex w-full items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-[var(--text-primary)] transition-colors hover:bg-[var(--danger-bg-hover)] hover:text-[var(--danger-text)] cursor-pointer"
        >
          <Power className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          {dict.tray.quit}
        </button>
      </div>
    </div>
  );
}
