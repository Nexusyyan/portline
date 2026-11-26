import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { NodeLabel } from "../../components/RegionFlag";
import type { Dict } from "../../i18n";
import type {
  ConnectionHistoryPage,
  ConnectionItem,
  InboundPort,
  Locale,
  LogEntry,
  NodeIR,
  PlatformTrafficPayload,
  SimpleRoutingRule,
  TrafficSample,
} from "../../types";
import {
  formatBytes,
  formatConnectionExit,
  formatConnectionRule,
  formatSpeed,
  humanizeLogMessage,
} from "../../utils/nodes";

interface MonitorViewProps {
  dict: Dict;
  locale: Locale;
  coreRunning: boolean;
  nodes: NodeIR[];
  rules: SimpleRoutingRule[];
  extraPorts: InboundPort[];
  connections: ConnectionItem[];
  logs: LogEntry[];
  traffic: TrafficSample;
  downHistory: number[];
  upHistory: number[];
  onFetchHistory: (
    query: string,
    page: number,
    pageSize: number
  ) => Promise<ConnectionHistoryPage | null>;
  onFetchPlatforms: (range: string) => Promise<PlatformTrafficPayload | null>;
  onClearHistory: () => Promise<void>;
  onCloseConnection: (id: string) => void;
  onCloseAllConnections: () => void;
  onClearLogs: () => void;
}

type SubTab = "conns" | "history" | "platforms" | "logs";
type LogLevelFilter = "ALL" | "INFO" | "WARN" | "ERROR";
type RangeFilter = "today" | "7d" | "30d" | "all";

function formatShortTime(iso: string): string {
  if (!iso) return "--";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso.slice(11, 19) || iso;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function MonitorView({
  dict,
  locale,
  coreRunning,
  nodes,
  rules,
  extraPorts,
  connections,
  logs,
  traffic,
  downHistory,
  upHistory,
  onFetchHistory,
  onFetchPlatforms,
  onClearHistory,
  onCloseConnection,
  onCloseAllConnections,
  onClearLogs,
}: MonitorViewProps) {
  const [subTab, setSubTab] = useState<SubTab>("conns");
  const [logFilter, setLogFilter] = useState<LogLevelFilter>("ALL");
  const [platformRange, setPlatformRange] = useState<RangeFilter>("today");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(25);

  const [historyData, setHistoryData] = useState<ConnectionHistoryPage>({
    items: [],
    total: 0,
    page: 1,
    pageSize: 25,
  });
  const [platformData, setPlatformData] = useState<PlatformTrafficPayload>({
    items: [],
    totalUpload: 0,
    totalDownload: 0,
    totalConnections: 0,
  });

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const useEnglish = locale === "en";

  const nodeById = useMemo(
    () => new Map<string, NodeIR>(nodes.map((n) => [n.id, n])),
    [nodes]
  );

  const exitLabels = useMemo(
    () => ({
      direct: dict.monitor.exitDirect,
      reject: dict.monitor.exitReject,
      mainExit: dict.monitor.exitMain,
      autoAll: dict.monitor.exitAutoAll,
      autoRegionPrefix: dict.monitor.exitAutoPrefix,
      regions: dict.routing.regions as Record<string, string>,
    }),
    [dict]
  );

  const ruleLabels = useMemo(
    () => ({
      defaultFallback: dict.monitor.ruleFallback,
      directMode: dict.routing.directMode,
      globalMode: dict.routing.globalMode,
      lanDirect: dict.monitor.ruleLan,
      portRulePrefix: dict.monitor.rulePortPrefix,
      domainRuleFallback: dict.monitor.ruleDomain,
      processRuleFallback: dict.monitor.ruleProcess,
    }),
    [dict]
  );

  const presetTitles = dict.routing.presetTitles as Record<string, string>;

  // Reset pagination whenever active tab, search query, filter, or pageSize changes.
  useEffect(() => {
    setPage(1);
  }, [subTab, query, logFilter, platformRange, pageSize]);

  // Fetch SQLite history when History tab is active.
  useEffect(() => {
    if (subTab !== "history") return;
    let alive = true;
    const load = async () => {
      const data = await onFetchHistory(query, page, pageSize);
      if (alive && data) setHistoryData(data);
    };
    void load();
    const timer = window.setInterval(() => void load(), 3000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [subTab, query, page, pageSize, onFetchHistory]);

  // Fetch aggregated platform traffic when Platforms tab is active.
  useEffect(() => {
    if (subTab !== "platforms") return;
    let alive = true;
    const load = async () => {
      const data = await onFetchPlatforms(platformRange);
      if (alive && data) setPlatformData(data);
    };
    void load();
    const timer = window.setInterval(() => void load(), 2500);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [subTab, platformRange, onFetchPlatforms]);

  // Throughput sparkline canvas.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || canvas.offsetWidth === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.offsetWidth;
    const h = 58;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    const cs = getComputedStyle(document.documentElement);
    const downColor = cs.getPropertyValue("--success-dot").trim() || "#10b981";
    const upColor = cs.getPropertyValue("--text-tertiary").trim() || "#71717a";
    const maxV = Math.max(1024, ...downHistory, ...upHistory) * 1.15;

    const drawSeries = (series: number[], color: string, fillAlpha: number) => {
      if (series.length < 2) return;
      ctx.beginPath();
      series.forEach((v, i) => {
        const x = (i / (series.length - 1)) * w;
        const y = h - 6 - (v / maxV) * (h - 14);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.6;
      ctx.stroke();

      if (fillAlpha > 0) {
        ctx.lineTo(w, h);
        ctx.lineTo(0, h);
        ctx.closePath();
        ctx.globalAlpha = fillAlpha;
        ctx.fillStyle = color;
        ctx.fill();
        ctx.globalAlpha = 1;
      }
    };

    drawSeries(downHistory, downColor, 0.12);
    drawSeries(upHistory, upColor, 0);
  }, [downHistory, upHistory]);

  const decoratedConns = useMemo(() => {
    return connections
      .map((c) => {
        const displayProcess =
          c.process === "Local App" ? dict.monitor.localApp : c.process;
        const ruleInfo = formatConnectionRule(
          c.rule,
          rules,
          extraPorts,
          ruleLabels,
          presetTitles,
          useEnglish
        );
        const exitInfo = formatConnectionExit(c.chain, nodeById, exitLabels);
        const totalBytes = c.uploadBytes + c.downloadBytes;
        return { conn: c, displayProcess, ruleInfo, exitInfo, totalBytes };
      })
      .sort(
        (a, b) =>
          b.totalBytes - a.totalBytes ||
          b.conn.start.localeCompare(a.conn.start) ||
          a.conn.id.localeCompare(b.conn.id)
      );
  }, [
    connections,
    dict.monitor.localApp,
    rules,
    extraPorts,
    ruleLabels,
    presetTitles,
    useEnglish,
    nodeById,
    exitLabels,
  ]);

  const filteredConns = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return decoratedConns;
    return decoratedConns.filter(
      ({ conn, displayProcess, ruleInfo, exitInfo }) =>
        displayProcess.toLowerCase().includes(q) ||
        conn.destination.toLowerCase().includes(q) ||
        conn.inbound.toLowerCase().includes(q) ||
        ruleInfo.label.toLowerCase().includes(q) ||
        ruleInfo.raw.toLowerCase().includes(q) ||
        exitInfo.label.toLowerCase().includes(q) ||
        (exitInfo.protocol && exitInfo.protocol.toLowerCase().includes(q)) ||
        conn.chain.some((tag) => tag.toLowerCase().includes(q))
    );
  }, [decoratedConns, query]);

  const filteredPlatforms = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return platformData.items;
    return platformData.items.filter(
      (p) =>
        p.platform.toLowerCase().includes(q) ||
        p.topDomains.some((d) => d.toLowerCase().includes(q))
    );
  }, [platformData.items, query]);

  const logCounts = useMemo(() => {
    let info = 0;
    let warn = 0;
    let err = 0;
    for (const l of logs) {
      const upper = l.level.toUpperCase();
      if (upper === "ERROR") err += 1;
      else if (upper === "WARN") warn += 1;
      else info += 1;
    }
    return { all: logs.length, info, warn, err };
  }, [logs]);

  const filteredLogs = useMemo(() => {
    const q = query.trim().toLowerCase();
    return logs
      .filter((l) => {
        if (logFilter === "ALL") return true;
        const upper = l.level.toUpperCase();
        if (logFilter === "INFO") return upper === "INFO" || upper === "DEBUG";
        return upper === logFilter;
      })
      .map((entry) => ({
        entry,
        displayMessage: humanizeLogMessage(entry.message, nodeById, exitLabels),
      }))
      .filter(({ entry, displayMessage }) => {
        if (!q) return true;
        return (
          displayMessage.toLowerCase().includes(q) ||
          entry.message.toLowerCase().includes(q) ||
          entry.level.toLowerCase().includes(q)
        );
      });
  }, [logs, logFilter, query, nodeById, exitLabels]);

  const totalItemsForTab =
    subTab === "conns"
      ? filteredConns.length
      : subTab === "history"
        ? historyData.total
        : subTab === "platforms"
          ? filteredPlatforms.length
          : filteredLogs.length;

  const totalPages = Math.max(1, Math.ceil(totalItemsForTab / pageSize));
  const safePage = Math.min(page, totalPages);

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  const pagedConns = useMemo(
    () => filteredConns.slice((safePage - 1) * pageSize, safePage * pageSize),
    [filteredConns, safePage, pageSize]
  );

  const pagedPlatforms = useMemo(
    () => filteredPlatforms.slice((safePage - 1) * pageSize, safePage * pageSize),
    [filteredPlatforms, safePage, pageSize]
  );

  const pagedLogs = useMemo(
    () => filteredLogs.slice((safePage - 1) * pageSize, safePage * pageSize),
    [filteredLogs, safePage, pageSize]
  );

  const grandTotalPlatformBytes = Math.max(
    1,
    platformData.totalDownload + platformData.totalUpload
  );

  const pageLabel = dict.monitor.pageInfo
    .replace("{page}", String(safePage))
    .replace("{totalPages}", String(totalPages));
  const totalCountLabel = dict.monitor.totalItems.replace(
    "{count}",
    String(totalItemsForTab)
  );

  return (
    <section className="flex min-h-0 flex-1 flex-col">
      {/* Top Traffic Sparkline + KPI Row */}
      <div className="grid shrink-0 grid-cols-1 items-center gap-3 border-b border-[var(--border-hairline)] px-5 py-3.5 md:grid-cols-12">
        <div className="flex flex-col gap-1 md:col-span-7">
          <div className="flex items-center justify-between text-[11.5px]">
            <span className="font-medium text-[var(--text-secondary)]">
              {dict.monitor.liveTraffic}
            </span>
            <span className="flex items-center gap-2.5 font-mono text-[11.5px] num-tabular text-[var(--text-secondary)]">
              <span className="inline-flex items-center gap-0.5">
                <ArrowDown
                  className="h-3 w-3"
                  style={{ color: "var(--success-text)" }}
                  strokeWidth={1.6}
                />
                {formatSpeed(traffic.downBytesPerSec)}
              </span>
              <span className="inline-flex items-center gap-0.5">
                <ArrowUp className="h-3 w-3 text-[var(--text-tertiary)]" strokeWidth={1.6} />
                {formatSpeed(traffic.upBytesPerSec)}
              </span>
            </span>
          </div>
          <canvas
            ref={canvasRef}
            height={58}
            className="w-full rounded-[9px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)]"
          />
        </div>

        <div className="grid grid-cols-3 gap-2 md:col-span-5">
          <div className="rounded-[10px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-2.5">
            <div className="text-[11px] text-[var(--text-secondary)]">
              {dict.monitor.kpiConns}
            </div>
            <div className="mt-0.5 font-mono text-[15px] font-semibold num-tabular text-[var(--text-primary)]">
              {connections.length}
            </div>
          </div>
          <div className="rounded-[10px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-2.5">
            <div className="flex items-center justify-between text-[11px] text-[var(--text-secondary)]">
              <span>{dict.monitor.kpiCore}</span>
              {coreRunning && traffic.memoryBytes > 0 && (
                <span className="font-mono text-[10.5px] num-tabular text-[var(--text-tertiary)]">
                  {formatBytes(traffic.memoryBytes)}
                </span>
              )}
            </div>
            <div
              className="mt-0.5 text-[14px] font-semibold"
              style={{
                color: coreRunning ? "var(--success-text)" : "var(--text-secondary)",
              }}
            >
              {coreRunning ? dict.monitor.statusRunning : dict.monitor.statusIdle}
            </div>
          </div>
          <div className="rounded-[10px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-2.5">
            <div className="flex items-center justify-between text-[11px] text-[var(--text-secondary)]">
              <span>{dict.monitor.kpiLogRing}</span>
              {logCounts.err > 0 && (
                <span
                  className="font-mono text-[10.5px] font-medium num-tabular"
                  style={{ color: "var(--danger-text)" }}
                >
                  {logCounts.err} ERR
                </span>
              )}
            </div>
            <div className="mt-0.5 font-mono text-[15px] font-semibold num-tabular text-[var(--text-primary)]">
              {logs.length}/500
            </div>
          </div>
        </div>
      </div>

      {/* Sub-Tabs & Filter Bar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-[var(--border-hairline)] px-5 py-2.5">
        <div className="flex flex-wrap items-center gap-2.5">
          <div className="inline-flex items-center gap-1 text-[12px]">
            {(
              [
                ["conns", dict.monitor.tabConns],
                ["history", dict.monitor.tabHistory],
                ["platforms", dict.monitor.tabPlatforms],
                ["logs", dict.monitor.tabLogs],
              ] as const
            ).map(([key, label]) => (
              <button
                key={key}
                type="button"
                onClick={() => setSubTab(key)}
                className={`rounded-[7px] px-3 py-1 font-medium transition-colors cursor-pointer ${
                  subTab === key
                    ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          {subTab === "platforms" && (
            <div className="inline-flex items-center gap-0.5 rounded-[8px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5 text-[11px]">
              {(
                [
                  ["today", dict.monitor.rangeToday],
                  ["7d", dict.monitor.range7d],
                  ["30d", dict.monitor.range30d],
                  ["all", dict.monitor.rangeAll],
                ] as const
              ).map(([rKey, rLabel]) => (
                <button
                  key={rKey}
                  type="button"
                  onClick={() => setPlatformRange(rKey)}
                  className={`rounded-[6px] px-2 py-0.5 font-medium transition-colors cursor-pointer ${
                    platformRange === rKey
                      ? "bg-[var(--bg-surface)] text-[var(--text-primary)] shadow-xs"
                      : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  {rLabel}
                </button>
              ))}
            </div>
          )}

          {subTab === "logs" && (
            <div className="inline-flex items-center gap-1 rounded-[8px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5 text-[11px]">
              {(
                [
                  ["ALL", dict.monitor.logAll, logCounts.all],
                  ["INFO", "INFO", logCounts.info],
                  ["WARN", "WARN", logCounts.warn],
                  ["ERROR", "ERROR", logCounts.err],
                ] as const
              ).map(([key, label, count]) => {
                const active = logFilter === key;
                return (
                  <button
                    key={key}
                    type="button"
                    onClick={() => setLogFilter(key)}
                    className={`inline-flex items-center gap-1 rounded-[6px] px-2 py-0.5 font-medium transition-colors cursor-pointer ${
                      active
                        ? "bg-[var(--bg-surface)] text-[var(--text-primary)] shadow-xs"
                        : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                    }`}
                  >
                    <span>{label}</span>
                    <span className="font-mono text-[10px] num-tabular text-[var(--text-tertiary)]">
                      {count}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex items-center gap-2">
          <div className="relative">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-tertiary)]"
              strokeWidth={1.5}
            />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={dict.monitor.searchPh}
              className="ui-input h-7 w-52 pr-2.5 pl-8 text-[12px]"
            />
          </div>
          {subTab === "conns" && (
            <button
              type="button"
              onClick={onCloseAllConnections}
              className="ui-btn-danger-subtle h-7"
            >
              <X className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.monitor.closeAll}
            </button>
          )}
          {(subTab === "history" || subTab === "platforms") && (
            <button
              type="button"
              onClick={async () => {
                await onClearHistory();
                setHistoryData({ items: [], total: 0, page: 1, pageSize });
                const fresh = await onFetchPlatforms(platformRange);
                if (fresh) setPlatformData(fresh);
              }}
              className="ui-btn-danger-subtle h-7"
            >
              <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.monitor.clearHistory}
            </button>
          )}
          {subTab === "logs" && (
            <button
              type="button"
              onClick={onClearLogs}
              className="ui-btn-danger-subtle h-7"
            >
              <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.monitor.clearLogs}
            </button>
          )}
        </div>
      </div>

      {/* Body Content */}
      {subTab === "conns" && (
        <div className="min-h-0 flex-1 overflow-auto">
          {pagedConns.length === 0 ? (
            <div className="flex h-full items-center justify-center p-8 text-[12.5px] text-[var(--text-secondary)]">
              {dict.monitor.emptyConns}
            </div>
          ) : (
            <table className="w-full min-w-[820px] table-fixed border-collapse text-left">
              <thead className="sticky top-0 z-10 border-b border-[var(--border-hairline)] bg-[var(--bg-canvas)] text-[11.5px] font-medium text-[var(--text-secondary)]">
                <tr>
                  <th className="w-[132px] py-2 pr-2 pl-5 font-medium">
                    {dict.monitor.colProcess}
                  </th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colDest}</th>
                  <th className="w-[74px] px-2 py-2 font-medium">
                    {dict.monitor.colInbound}
                  </th>
                  <th className="w-[132px] px-2 py-2 font-medium">
                    {dict.monitor.colRule}
                  </th>
                  <th className="w-[236px] px-2 py-2 font-medium">
                    {dict.monitor.colExit}
                  </th>
                  <th className="w-[88px] px-2 py-2 text-right font-medium">
                    {dict.monitor.colTraffic}
                  </th>
                  <th className="w-[42px] py-2 pr-5 pl-1" />
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border-hairline)] text-[12px]">
                {pagedConns.map(({ conn, displayProcess, ruleInfo, exitInfo, totalBytes }) => {
                  const exitTooltip = [
                    exitInfo.groupHint,
                    exitInfo.label,
                    exitInfo.rawTag && exitInfo.rawTag !== exitInfo.label
                      ? `(${exitInfo.rawTag})`
                      : "",
                  ]
                    .filter(Boolean)
                    .join(" / ");
                  const trafficTooltip = `${dict.monitor.uploadShort}: ${formatBytes(conn.uploadBytes)} | ${dict.monitor.downloadShort}: ${formatBytes(conn.downloadBytes)}`;

                  return (
                    <tr key={conn.id} className="hover:bg-[var(--bg-hover)]">
                      <td
                        className="truncate py-2.5 pr-2 pl-5 font-mono text-[12px] font-medium text-[var(--text-primary)]"
                        title={displayProcess}
                      >
                        {displayProcess}
                      </td>
                      <td
                        className="truncate px-2 py-2.5 font-mono text-[12px] text-[var(--text-secondary)]"
                        title={conn.destination}
                      >
                        {conn.destination}
                      </td>
                      <td className="px-2 py-2.5 whitespace-nowrap">
                        <span className="inline-block whitespace-nowrap rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[11px] num-tabular text-[var(--text-secondary)]">
                          {conn.inbound}
                        </span>
                      </td>
                      <td className="px-2 py-2.5" title={ruleInfo.raw}>
                        {ruleInfo.kind === "fallback" ? (
                          <span className="block truncate text-[12px] text-[var(--text-tertiary)]">
                            {ruleInfo.label}
                          </span>
                        ) : (
                          <span className="inline-block max-w-full truncate rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 text-[11.5px] font-medium text-[var(--text-primary)]">
                            {ruleInfo.label}
                          </span>
                        )}
                      </td>
                      <td className="px-2 py-2.5" title={exitTooltip}>
                        <div className="flex min-w-0 items-center gap-1.5">
                          {exitInfo.protocol ? (
                            <span className="shrink-0 whitespace-nowrap rounded-[4px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[10px] font-medium text-[var(--text-secondary)]">
                              {exitInfo.protocol}
                            </span>
                          ) : (
                            <span
                              className="h-1.5 w-1.5 shrink-0 rounded-full"
                              style={{
                                backgroundColor:
                                  exitInfo.kind === "direct"
                                    ? "var(--success-dot)"
                                    : exitInfo.kind === "reject"
                                      ? "var(--danger-dot)"
                                      : "var(--text-tertiary)",
                              }}
                            />
                          )}
                          {exitInfo.kind === "node" ? (
                            <NodeLabel
                              remark={exitInfo.label}
                              region={exitInfo.region}
                              size="xs"
                              className="font-medium text-[var(--text-primary)]"
                            />
                          ) : (
                            <span className="truncate font-medium text-[var(--text-primary)]">
                              {exitInfo.label}
                            </span>
                          )}
                        </div>
                      </td>
                      <td
                        className="px-2 py-2.5 text-right font-mono text-[11.5px] whitespace-nowrap num-tabular text-[var(--text-secondary)]"
                        title={trafficTooltip}
                      >
                        {formatBytes(totalBytes)}
                      </td>
                      <td className="py-2.5 pr-5 pl-1 text-right">
                        <button
                          type="button"
                          onClick={() => onCloseConnection(conn.id)}
                          title={dict.monitor.closeConn}
                          className="ui-btn-danger-ghost h-6 w-6 p-0"
                        >
                          <X className="h-3.5 w-3.5" strokeWidth={1.5} />
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}

      {subTab === "history" && (
        <div className="min-h-0 flex-1 overflow-auto">
          {historyData.items.length === 0 ? (
            <div className="flex h-full items-center justify-center p-8 text-[12.5px] text-[var(--text-secondary)]">
              {dict.monitor.emptyHistory}
            </div>
          ) : (
            <table className="w-full min-w-[860px] table-fixed border-collapse text-left">
              <thead className="sticky top-0 z-10 border-b border-[var(--border-hairline)] bg-[var(--bg-canvas)] text-[11.5px] font-medium text-[var(--text-secondary)]">
                <tr>
                  <th className="w-[126px] py-2 pr-2 pl-5 font-medium">
                    {dict.monitor.colClosedAt}
                  </th>
                  <th className="w-[126px] px-2 py-2 font-medium">
                    {dict.monitor.colPlatform}
                  </th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colDest}</th>
                  <th className="w-[116px] px-2 py-2 font-medium">
                    {dict.monitor.colProcess}
                  </th>
                  <th className="w-[196px] px-2 py-2 font-medium">
                    {dict.monitor.colExit}
                  </th>
                  <th className="w-[96px] py-2 pr-5 pl-2 text-right font-medium">
                    {dict.monitor.colTraffic}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border-hairline)] text-[12px]">
                {historyData.items.map((rec) => {
                  const exitInfo = formatConnectionExit(rec.chain, nodeById, exitLabels);
                  const total = rec.uploadBytes + rec.downloadBytes;
                  const proc = rec.process === "Local App" ? dict.monitor.localApp : rec.process;
                  const tip = `${dict.monitor.uploadShort}: ${formatBytes(rec.uploadBytes)} | ${dict.monitor.downloadShort}: ${formatBytes(rec.downloadBytes)}`;
                  return (
                    <tr key={rec.id} className="hover:bg-[var(--bg-hover)]">
                      <td className="py-2.5 pr-2 pl-5 font-mono text-[11px] num-tabular text-[var(--text-tertiary)]">
                        {formatShortTime(rec.closedAt)}
                      </td>
                      <td className="px-2 py-2.5">
                        <span className="inline-block max-w-full truncate rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--text-primary)]">
                          {rec.platform}
                        </span>
                      </td>
                      <td
                        className="truncate px-2 py-2.5 font-mono text-[11.5px] text-[var(--text-secondary)]"
                        title={rec.destination}
                      >
                        {rec.destination}
                      </td>
                      <td
                        className="truncate px-2 py-2.5 font-mono text-[11.5px] text-[var(--text-primary)]"
                        title={proc}
                      >
                        {proc}
                      </td>
                      <td className="px-2 py-2.5">
                        <div className="flex min-w-0 items-center gap-1.5">
                          {exitInfo.kind === "node" ? (
                            <NodeLabel
                              remark={exitInfo.label}
                              region={exitInfo.region}
                              size="xs"
                              className="text-[var(--text-primary)]"
                            />
                          ) : (
                            <span className="truncate text-[var(--text-secondary)]">
                              {exitInfo.label}
                            </span>
                          )}
                        </div>
                      </td>
                      <td
                        className="py-2.5 pr-5 pl-2 text-right font-mono text-[11.5px] num-tabular text-[var(--text-primary)]"
                        title={tip}
                      >
                        {formatBytes(total)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}

      {subTab === "platforms" && (
        <div className="min-h-0 flex-1 overflow-auto">
          {/* Summary Strip */}
          <div className="grid grid-cols-3 gap-2.5 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-5 py-3">
            <div>
              <div className="text-[11px] text-[var(--text-secondary)]">
                {dict.monitor.kpiTotalDown}
              </div>
              <div className="mt-0.5 font-mono text-[14px] font-semibold num-tabular text-[var(--text-primary)]">
                {formatBytes(platformData.totalDownload)}
              </div>
            </div>
            <div>
              <div className="text-[11px] text-[var(--text-secondary)]">
                {dict.monitor.kpiTotalUp}
              </div>
              <div className="mt-0.5 font-mono text-[14px] font-semibold num-tabular text-[var(--text-primary)]">
                {formatBytes(platformData.totalUpload)}
              </div>
            </div>
            <div>
              <div className="text-[11px] text-[var(--text-secondary)]">
                {dict.monitor.kpiTrackedConns}
              </div>
              <div className="mt-0.5 font-mono text-[14px] font-semibold num-tabular text-[var(--text-primary)]">
                {platformData.totalConnections}
              </div>
            </div>
          </div>

          {pagedPlatforms.length === 0 ? (
            <div className="flex h-48 items-center justify-center p-8 text-[12.5px] text-[var(--text-secondary)]">
              {dict.monitor.emptyPlatforms}
            </div>
          ) : (
            <table className="w-full min-w-[740px] table-fixed border-collapse text-left">
              <thead className="sticky top-0 z-10 border-b border-[var(--border-hairline)] bg-[var(--bg-canvas)] text-[11.5px] font-medium text-[var(--text-secondary)]">
                <tr>
                  <th className="w-[240px] py-2 pr-2 pl-5 font-medium">
                    {dict.monitor.colPlatform}
                  </th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colShare}</th>
                  <th className="w-[82px] px-2 py-2 text-right font-medium">
                    {dict.monitor.colConns}
                  </th>
                  <th className="w-[96px] px-2 py-2 text-right font-medium">
                    {dict.monitor.colDownload}
                  </th>
                  <th className="w-[92px] px-2 py-2 text-right font-medium">
                    {dict.monitor.colUpload}
                  </th>
                  <th className="w-[96px] py-2 pr-5 pl-2 text-right font-medium">
                    {dict.monitor.colTotal}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border-hairline)] text-[12px]">
                {pagedPlatforms.map((item) => {
                  const pct = Math.min(
                    100,
                    Math.round((item.totalBytes / grandTotalPlatformBytes) * 1000) / 10
                  );
                  return (
                    <tr key={item.platform} className="hover:bg-[var(--bg-hover)]">
                      <td className="py-2.5 pr-2 pl-5">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate font-medium text-[var(--text-primary)]">
                            {item.platform}
                          </span>
                          {item.activeCount > 0 && (
                            <span
                              className="shrink-0 rounded-full px-1.5 py-0.5 font-mono text-[9.5px] font-medium"
                              style={{
                                backgroundColor: "var(--success-bg)",
                                color: "var(--success-text)",
                              }}
                            >
                              {dict.monitor.activeTag} {item.activeCount}
                            </span>
                          )}
                        </div>
                        {item.topDomains.length > 0 && (
                          <div className="mt-0.5 truncate font-mono text-[10.5px] text-[var(--text-tertiary)]">
                            {item.topDomains.join(", ")}
                          </div>
                        )}
                      </td>
                      <td className="px-2 py-2.5">
                        <div className="flex items-center gap-2.5">
                          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[var(--bg-track)]">
                            <div
                              className="h-full rounded-full bg-[var(--accent-primary)]"
                              style={{ width: `${Math.max(1.5, pct)}%` }}
                            />
                          </div>
                          <span className="w-11 shrink-0 text-right font-mono text-[11px] num-tabular text-[var(--text-secondary)]">
                            {pct.toFixed(1)}%
                          </span>
                        </div>
                      </td>
                      <td className="px-2 py-2.5 text-right font-mono text-[11.5px] num-tabular text-[var(--text-secondary)]">
                        {item.connectionCount}
                      </td>
                      <td className="px-2 py-2.5 text-right font-mono text-[11.5px] num-tabular text-[var(--text-secondary)]">
                        {formatBytes(item.downloadBytes)}
                      </td>
                      <td className="px-2 py-2.5 text-right font-mono text-[11.5px] num-tabular text-[var(--text-tertiary)]">
                        {formatBytes(item.uploadBytes)}
                      </td>
                      <td className="py-2.5 pr-5 pl-2 text-right font-mono text-[11.5px] font-semibold num-tabular text-[var(--text-primary)]">
                        {formatBytes(item.totalBytes)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}

      {subTab === "logs" && (
        <div className="min-h-0 flex-1 overflow-auto bg-[var(--bg-subtle)] p-4 font-mono text-[11.5px]">
          {pagedLogs.length === 0 ? (
            <div className="py-12 text-center font-sans text-[12.5px] text-[var(--text-secondary)]">
              {dict.monitor.emptyLogs}
            </div>
          ) : (
            <div className="space-y-1.5">
              {pagedLogs.map(({ entry, displayMessage }, idx) => {
                const upper = entry.level.toUpperCase();
                const badgeStyle =
                  upper === "ERROR"
                    ? {
                        color: "var(--danger-text)",
                        backgroundColor: "var(--danger-bg-hover)",
                      }
                    : upper === "WARN"
                      ? {
                          color: "var(--warning-text)",
                          backgroundColor: "var(--warning-bg)",
                        }
                      : {
                          color: "var(--text-secondary)",
                          backgroundColor: "var(--bg-hover)",
                        };
                return (
                  <div
                    key={`${entry.timestamp}-${idx}`}
                    className="flex items-start gap-2.5 leading-relaxed"
                  >
                    <span className="shrink-0 num-tabular text-[var(--text-tertiary)]">
                      {entry.timestamp}
                    </span>
                    <span
                      className="w-12 shrink-0 rounded-[4px] px-1 py-0.5 text-center text-[10px] font-semibold leading-tight"
                      style={badgeStyle}
                    >
                      {upper}
                    </span>
                    <span
                      className="min-w-0 break-all text-[var(--text-primary)]"
                      title={entry.message !== displayMessage ? entry.message : undefined}
                    >
                      {displayMessage}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* Unified Pagination Footer Bar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-[var(--border-hairline)] bg-[var(--bg-canvas)] px-5 py-2 text-[11.5px] text-[var(--text-secondary)]">
        <div className="flex items-center gap-3">
          <span className="font-mono num-tabular">{totalCountLabel}</span>
          <div className="inline-flex items-center gap-1 rounded-[7px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5">
            {[25, 50, 100].map((sz) => (
              <button
                key={sz}
                type="button"
                onClick={() => setPageSize(sz)}
                className={`rounded-[5px] px-1.5 py-0.5 font-mono text-[10.5px] num-tabular cursor-pointer ${
                  pageSize === sz
                    ? "bg-[var(--bg-surface)] font-medium text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                }`}
              >
                {sz}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          <button
            type="button"
            disabled={safePage <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            className="ui-btn-secondary h-7 px-2 text-[11.5px]"
          >
            <ChevronLeft className="h-3.5 w-3.5" strokeWidth={1.5} />
            {dict.monitor.pagePrev}
          </button>
          <span className="min-w-[88px] text-center font-mono text-[11.5px] num-tabular text-[var(--text-primary)]">
            {pageLabel}
          </span>
          <button
            type="button"
            disabled={safePage >= totalPages}
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            className="ui-btn-secondary h-7 px-2 text-[11.5px]"
          >
            {dict.monitor.pageNext}
            <ChevronRight className="h-3.5 w-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </section>
  );
}
