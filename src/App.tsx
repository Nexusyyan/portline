import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api/client";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { CoreView } from "./features/core/CoreView";
import { MonitorView } from "./features/monitor/MonitorView";
import { NodesView } from "./features/nodes/NodesView";
import { RoutingView } from "./features/routing/RoutingView";
import { SettingsView } from "./features/settings/SettingsView";
import { detectInitialLocale, getDict, persistLocale } from "./i18n";
import type {
  AppSettings,
  CompileOutput,
  ConnectionItem,
  CoreInfo,
  InboundPort,
  Locale,
  LogEntry,
  NavTab,
  NodeIR,
  PreflightReport,
  RouteMode,
  SimpleRoutingRule,
  SubscriptionGroup,
  ThemeMode,
  ToastMessage,
  TrafficSample,
} from "./types";

const THEME_STORAGE_KEY = "portline.theme";

const DEFAULT_CORE_INFO: CoreInfo = {
  installed: false,
  running: false,
  version: null,
  binaryPath: "core/sing-box.exe",
  sha256: null,
  portableMode: false,
  dataDir: "",
  netguardArmed: false,
  netguardRecoveredFromCrash: false,
};

const DEFAULT_SETTINGS: AppSettings = {
  locale: "zh-CN",
  theme: "system",
  routeMode: "rule",
  mainNodeId: null,
  mixedPort: 10808,
  controlPort: 9090,
  allowLan: false,
  systemProxyEnabled: false,
  tunEnabled: false,
  autoStartCore: true,
  minimizeToTray: true,
  dnsLocalServer: "223.5.5.5",
  dnsRemoteServer: "https://1.1.1.1/dns-query",
  dnsStrategy: "prefer_ipv4",
  dnsFakeIp: false,
  latencyTestUrl: "https://www.gstatic.com/generate_204",
  customJsonOverride: null,
};

const EMPTY_COMPILED: CompileOutput = {
  config: {},
  totalNodes: 0,
  reachableNodes: 0,
  reachableNodeIds: [],
  synthesizedGroups: [],
  controlSecret: "",
};

const EMPTY_TRAFFIC: TrafficSample = {
  upBytesPerSec: 0,
  downBytesPerSec: 0,
  memoryBytes: 0,
  activeConnections: 0,
};

function readInitialTheme(): ThemeMode {
  try {
    const v = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    // ignore
  }
  return "system";
}

export default function App() {
  const [locale, setLocale] = useState<Locale>(detectInitialLocale);
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme);
  const [activeTab, setActiveTab] = useState<NavTab>("nodes");

  const [coreInfo, setCoreInfo] = useState<CoreInfo>(DEFAULT_CORE_INFO);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [nodes, setNodes] = useState<NodeIR[]>([]);
  const [subscriptions, setSubscriptions] = useState<SubscriptionGroup[]>([]);
  const [rules, setRules] = useState<SimpleRoutingRule[]>([]);
  const [extraPorts, setExtraPorts] = useState<InboundPort[]>([]);
  const [compiled, setCompiled] = useState<CompileOutput>(EMPTY_COMPILED);
  const [preflight, setPreflight] = useState<PreflightReport | null>(null);

  const [connections, setConnections] = useState<ConnectionItem[]>([]);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [traffic, setTraffic] = useState<TrafficSample>(EMPTY_TRAFFIC);
  const [downHistory, setDownHistory] = useState<number[]>(() => Array(40).fill(0));
  const [upHistory, setUpHistory] = useState<number[]>(() => Array(40).fill(0));

  const [testingKind, setTestingKind] = useState<"tcping" | "real" | null>(null);
  const [coreBusy, setCoreBusy] = useState(false);
  const [downloadingCore, setDownloadingCore] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const dict = useMemo(() => getDict(locale), [locale]);

  const pushToast = useCallback((text: string, tone: "ok" | "err" | "neutral" = "ok") => {
    const id = `${Date.now()}-${Math.random()}`;
    setToasts((prev) => [...prev.slice(-3), { id, text, tone }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 2800);
  }, []);

  // Apply light/dark theme class cleanly to document root.
  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      const dark =
        theme === "dark" ||
        (theme === "system" &&
          window.matchMedia("(prefers-color-scheme: dark)").matches);
      root.classList.toggle("dark", dark);
    };
    apply();
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    } catch {
      // ignore
    }
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  const reloadBootstrap = useCallback(async () => {
    if (!api.isDesktop()) return;
    try {
      const data = await api.getBootstrapState();
      setCoreInfo(data.coreInfo);
      setSettings(data.settings);
      setNodes(data.nodes);
      setSubscriptions(data.subscriptions);
      setRules(data.rules);
      setExtraPorts(data.extraPorts);
      setCompiled(data.compiled);
      setLogs(data.logs);
    } catch (err) {
      pushToast(String(err), "err");
    }
  }, [pushToast]);

  useEffect(() => {
    void reloadBootstrap();
  }, [reloadBootstrap]);

  // Periodic live monitor polling (1.5s interval, bounded ring buffer).
  useEffect(() => {
    if (!api.isDesktop()) return;
    const timer = window.setInterval(async () => {
      try {
        const snap = await api.getLiveMonitor();
        setCoreInfo((prev) =>
          prev.running === snap.coreRunning ? prev : { ...prev, running: snap.coreRunning }
        );
        setConnections(snap.connections);
        setLogs(snap.logs);
        setTraffic(snap.traffic);
        setDownHistory((prev) => [...prev.slice(1), snap.traffic.downBytesPerSec]);
        setUpHistory((prev) => [...prev.slice(1), snap.traffic.upBytesPerSec]);
      } catch {
        // ignore transient polling error
      }
    }, 1500);
    return () => window.clearInterval(timer);
  }, []);

  const mainNode = useMemo(
    () => nodes.find((n) => n.id === settings.mainNodeId) ?? nodes[0] ?? null,
    [nodes, settings.mainNodeId]
  );

  const handleChangeLocale = (next: Locale) => {
    setLocale(next);
    persistLocale(next);
    if (api.isDesktop()) {
      const updated = { ...settings, locale: next };
      setSettings(updated);
      void api.updateSettings(updated).then(setCompiled).catch(() => {});
    }
  };

  const handleToggleLocale = () => {
    handleChangeLocale(locale === "zh-CN" ? "en" : "zh-CN");
  };

  const handleChangeTheme = (next: ThemeMode) => {
    setTheme(next);
    if (api.isDesktop()) {
      const updated = { ...settings, theme: next };
      setSettings(updated);
      void api.updateSettings(updated).then(setCompiled).catch(() => {});
    }
  };

  const handleCycleTheme = () => {
    const next: ThemeMode =
      theme === "system" ? "light" : theme === "light" ? "dark" : "system";
    handleChangeTheme(next);
  };

  const handleSelectMode = async (mode: RouteMode) => {
    setSettings((prev) => ({ ...prev, routeMode: mode }));
    if (!api.isDesktop()) return;
    try {
      const nextCompiled = await api.setRouteMode(mode);
      setCompiled(nextCompiled);
      const msg =
        mode === "direct"
          ? dict.toast.modeDirect
          : mode === "global"
            ? dict.toast.modeGlobal
            : dict.toast.modeRule;
      pushToast(msg, "ok");
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleToggleSysProxy = async () => {
    const next = !settings.systemProxyEnabled;
    if (!api.isDesktop()) {
      setSettings((prev) => ({ ...prev, systemProxyEnabled: next }));
      return;
    }
    try {
      const info = await api.toggleSystemProxy(next);
      setCoreInfo(info);
      setSettings((prev) => ({ ...prev, systemProxyEnabled: next }));
      pushToast(next ? dict.toast.sysProxyOn : dict.toast.sysProxyOff);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleToggleTun = async () => {
    const next = !settings.tunEnabled;
    setSettings((prev) => ({ ...prev, tunEnabled: next }));
    if (!api.isDesktop()) return;
    try {
      const nextCompiled = await api.toggleTun(next);
      setCompiled(nextCompiled);
      pushToast(dict.toast.tunUpdated);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleToggleCore = async () => {
    if (!api.isDesktop()) return;
    setCoreBusy(true);
    try {
      if (coreInfo.running) {
        const info = await api.stopCore();
        setCoreInfo(info);
        pushToast(dict.toast.coreStopped, "neutral");
      } else {
        const [info, report] = await api.startCore();
        setCoreInfo(info);
        setPreflight(report);
        if (report.ok) {
          pushToast(dict.toast.coreStarted);
        } else {
          pushToast(dict.toast.preflightBlocked, "err");
        }
      }
    } catch (err) {
      const msg = String(err);
      pushToast(
        msg.includes("CORE_NOT_INSTALLED") ? dict.toast.coreMissingAction : msg,
        "err"
      );
    } finally {
      setCoreBusy(false);
    }
  };

  const handleSelectMainNode = async (id: string) => {
    setSettings((prev) => ({ ...prev, mainNodeId: id }));
    if (!api.isDesktop()) {
      pushToast(dict.toast.hotSwitched);
      return;
    }
    try {
      const res = await api.selectMainNode(id);
      setCompiled(res.compiled);
      pushToast(res.hotSwitched ? dict.toast.hotSwitched : dict.toast.nodeSelected);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleDeleteNode = async (id: string) => {
    if (!api.isDesktop()) {
      setNodes((prev) => prev.filter((n) => n.id !== id));
      return;
    }
    try {
      await api.deleteNode(id);
      await reloadBootstrap();
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleImportPayload = async (payload: string, groupName: string) => {
    if (!api.isDesktop()) return;
    try {
      const res = await api.importNodes(payload, groupName);
      await reloadBootstrap();
      pushToast(`${dict.toast.imported} (${res.nodes.length})`);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleImportSubscription = async (name: string, url: string) => {
    if (!api.isDesktop()) return;
    try {
      const sub = await api.importSubscription(name, url);
      await reloadBootstrap();
      pushToast(`${dict.toast.subSynced}: ${sub.name} (${sub.nodeCount})`);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleRefreshSubscription = async (id: string) => {
    if (!api.isDesktop()) return;
    try {
      const sub = await api.refreshSubscription(id);
      await reloadBootstrap();
      pushToast(`${dict.toast.subSynced}: ${sub.name} (${sub.nodeCount})`);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleDeleteSubscription = async (id: string) => {
    if (!api.isDesktop()) return;
    try {
      await api.deleteSubscription(id);
      await reloadBootstrap();
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleRunBatchTest = async (kind: "tcping" | "real", nodeIds: string[]) => {
    if (!api.isDesktop()) return;
    setTestingKind(kind);
    try {
      if (kind === "tcping") {
        await api.runBatchTcping(nodeIds);
      } else {
        await api.runBatchRealDelay(nodeIds);
      }
      await reloadBootstrap();
    } catch (err) {
      const msg = String(err);
      pushToast(
        msg.includes("CORE_NOT_INSTALLED") ? dict.toast.coreMissingAction : msg,
        "err"
      );
    } finally {
      setTestingKind(null);
    }
  };

  const handleSaveRule = async (rule: SimpleRoutingRule) => {
    setRules((prev) => {
      const exists = prev.some((r) => r.id === rule.id);
      return exists ? prev.map((r) => (r.id === rule.id ? rule : r)) : [rule, ...prev];
    });
    if (!api.isDesktop()) return;
    try {
      const next = await api.saveRule(rule);
      setCompiled(next);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleDeleteRule = async (id: string) => {
    setRules((prev) => prev.filter((r) => r.id !== id));
    if (!api.isDesktop()) return;
    try {
      const next = await api.deleteRule(id);
      setCompiled(next);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleSavePort = async (port: InboundPort) => {
    setExtraPorts((prev) => {
      const exists = prev.some((p) => p.id === port.id);
      return exists ? prev.map((p) => (p.id === port.id ? port : p)) : [...prev, port];
    });
    if (!api.isDesktop()) return;
    try {
      const next = await api.saveInboundPort(port);
      setCompiled(next);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleDeletePort = async (id: string) => {
    setExtraPorts((prev) => prev.filter((p) => p.id !== id));
    if (!api.isDesktop()) return;
    try {
      const next = await api.deleteInboundPort(id);
      setCompiled(next);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleCopyText = (text: string) => {
    if (navigator.clipboard) {
      void navigator.clipboard.writeText(text).catch(() => {});
    }
    pushToast(dict.toast.copied);
  };

  const handleRunPreflight = async (simulateBad: boolean) => {
    if (!api.isDesktop()) return;
    try {
      const report = await api.runPreflightCheck(simulateBad);
      setPreflight(report);
      pushToast(
        report.ok ? dict.toast.preflightPass : dict.toast.preflightBlocked,
        report.ok ? "ok" : "err"
      );
    } catch (err) {
      const msg = String(err);
      pushToast(
        msg.includes("CORE_NOT_INSTALLED") ? dict.toast.coreMissingAction : msg,
        "err"
      );
    }
  };

  const handleDownloadCore = async () => {
    if (!api.isDesktop()) return;
    setDownloadingCore(true);
    try {
      const info = await api.downloadOfficialCore();
      setCoreInfo(info);
      pushToast(dict.toast.coreDownloaded);
    } catch (err) {
      pushToast(String(err), "err");
    } finally {
      setDownloadingCore(false);
    }
  };

  const handleImportCoreBinary = async () => {
    if (!api.isDesktop()) return;
    try {
      const path = await api.pickBinaryFile();
      if (!path) return;
      const info = await api.importCoreBinary(path);
      setCoreInfo(info);
      pushToast(dict.toast.coreDownloaded);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleUpdateSettings = async (next: AppSettings) => {
    setSettings(next);
    if (!api.isDesktop()) return;
    try {
      const nextCompiled = await api.updateSettings(next);
      setCompiled(nextCompiled);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  const handleRestoreNetguard = async () => {
    if (!api.isDesktop()) return;
    try {
      const info = await api.restoreNetguardNow();
      setCoreInfo(info);
      setSettings((prev) => ({ ...prev, systemProxyEnabled: false }));
      pushToast(dict.toast.netguardRestored);
    } catch (err) {
      pushToast(String(err), "err");
    }
  };

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-[var(--bg-chrome)]">
      <TitleBar
        dict={dict}
        locale={locale}
        theme={theme}
        coreInfo={coreInfo}
        routeMode={settings.routeMode}
        sysProxyEnabled={settings.systemProxyEnabled}
        tunEnabled={settings.tunEnabled}
        traffic={traffic}
        onSelectMode={(m) => void handleSelectMode(m)}
        onToggleSysProxy={() => void handleToggleSysProxy()}
        onToggleTun={() => void handleToggleTun()}
        onOpenCore={() => setActiveTab("core")}
        onToggleLocale={handleToggleLocale}
        onCycleTheme={handleCycleTheme}
      />

      <div className="flex min-h-0 flex-1 bg-[var(--bg-chrome)]">
        <Sidebar
          dict={dict}
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          nodeCount={nodes.length}
          activeRuleCount={rules.filter((r) => r.enabled).length}
          connectionCount={connections.length}
          mainNode={mainNode}
          mixedPort={settings.mixedPort}
          coreInfo={coreInfo}
          coreBusy={coreBusy}
          onToggleCore={() => void handleToggleCore()}
        />

        <main className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-tl-[14px] border-t border-l border-[var(--border-hairline)] bg-[var(--bg-canvas)]">
          {activeTab === "nodes" && (
            <NodesView
              dict={dict}
              coreInfo={coreInfo}
              nodes={nodes}
              subscriptions={subscriptions}
              rules={rules}
              mainNodeId={settings.mainNodeId}
              testingKind={testingKind}
              downloadingCore={downloadingCore}
              onDownloadCore={handleDownloadCore}
              onImportCoreBinary={handleImportCoreBinary}
              onSelectMainNode={(id) => void handleSelectMainNode(id)}
              onDeleteNode={(id) => void handleDeleteNode(id)}
              onRunBatchTest={(k, ids) => void handleRunBatchTest(k, ids)}
              onImportPayload={handleImportPayload}
              onImportSubscription={handleImportSubscription}
              onRefreshSubscription={handleRefreshSubscription}
              onDeleteSubscription={handleDeleteSubscription}
            />
          )}

          {activeTab === "routing" && (
            <RoutingView
              dict={dict}
              locale={locale}
              routeMode={settings.routeMode}
              nodes={nodes}
              rules={rules}
              extraPorts={extraPorts}
              mainNodeId={settings.mainNodeId}
              mixedPort={settings.mixedPort}
              onSaveRule={(r) => void handleSaveRule(r)}
              onDeleteRule={(id) => void handleDeleteRule(id)}
              onSavePort={(p) => void handleSavePort(p)}
              onDeletePort={(id) => void handleDeletePort(id)}
              onGoToNodes={() => setActiveTab("nodes")}
              onCopyText={handleCopyText}
              onInvalidPort={() => pushToast(dict.toast.invalidPort, "err")}
            />
          )}

          {activeTab === "monitor" && (
            <MonitorView
              dict={dict}
              locale={locale}
              coreRunning={coreInfo.running}
              nodes={nodes}
              rules={rules}
              extraPorts={extraPorts}
              connections={connections}
              logs={logs}
              traffic={traffic}
              downHistory={downHistory}
              upHistory={upHistory}
              onCloseConnection={(id) => {
                if (api.isDesktop()) void api.closeConnection(id).catch(() => {});
              }}
              onCloseAllConnections={() => {
                if (api.isDesktop()) void api.closeAllConnections().catch(() => {});
              }}
              onClearLogs={() => {
                setLogs([]);
                if (api.isDesktop()) void api.clearLogs().catch(() => {});
              }}
            />
          )}

          {activeTab === "core" && (
            <CoreView
              dict={dict}
              coreInfo={coreInfo}
              compiled={compiled}
              preflight={preflight}
              coreBusy={coreBusy}
              onRunPreflight={handleRunPreflight}
              onDownloadCore={handleDownloadCore}
              onImportCoreBinary={handleImportCoreBinary}
              onToggleCore={handleToggleCore}
              onCopyJson={handleCopyText}
            />
          )}

          {activeTab === "settings" && (
            <SettingsView
              dict={dict}
              locale={locale}
              theme={theme}
              settings={settings}
              coreInfo={coreInfo}
              onChangeLocale={handleChangeLocale}
              onChangeTheme={handleChangeTheme}
              onUpdateSettings={(s) => void handleUpdateSettings(s)}
              onRestoreNetguard={() => void handleRestoreNetguard()}
            />
          )}
        </main>
      </div>

      {/* Floating Toasts */}
      <div className="pointer-events-none fixed right-4 bottom-4 z-50 space-y-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className="ui-toast pointer-events-auto flex max-w-md items-center gap-2 px-3 py-2 text-[12px] text-[var(--text-primary)]"
          >
            <span
              className="h-1.5 w-1.5 shrink-0 rounded-full"
              style={{
                backgroundColor:
                  t.tone === "err"
                    ? "var(--danger-dot)"
                    : t.tone === "ok"
                      ? "var(--success-dot)"
                      : "var(--text-tertiary)",
              }}
            />
            <span>{t.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
