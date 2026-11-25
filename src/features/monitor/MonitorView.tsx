import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Search, Trash2, X } from "lucide-react";
import type { Dict } from "../../i18n";
import type {
  ConnectionItem,
  InboundPort,
  Locale,
  LogEntry,
  NodeIR,
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
  onCloseConnection: (id: string) => void;
  onCloseAllConnections: () => void;
  onClearLogs: () => void;
}

type LogLevelFilter = "ALL" | "INFO" | "WARN" | "ERROR";

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
  onCloseConnection,
  onCloseAllConnections,
  onClearLogs,
}: MonitorViewProps) {
  const [subTab, setSubTab] = useState<"conns" | "logs">("conns");
  const [logFilter, setLogFilter] = useState<LogLevelFilter>("ALL");
  const [query, setQuery] = useState("");
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
      lanDirect: dict.monitor.ruleLan,
      portRulePrefix: dict.monitor.rulePortPrefix,
      domainRuleFallback: dict.monitor.ruleDomain,
      processRuleFallback: dict.monitor.ruleProcess,
    }),
    [dict]
  );

  const presetTitles = dict.routing.presetTitles as Record<string, string>;

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
        return {
          conn: c,
          displayProcess,
          ruleInfo,
          exitInfo,
          totalBytes,
        };
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
        <div className="flex items-center gap-3">
          <div className="inline-flex items-center gap-1 text-[12px]">
            <button
              type="button"
              onClick={() => setSubTab("conns")}
              className={`rounded-[7px] px-3 py-1 font-medium transition-colors cursor-pointer ${
                subTab === "conns"
                  ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                  : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              }`}
            >
              {dict.monitor.tabConns}
            </button>
            <button
              type="button"
              onClick={() => setSubTab("logs")}
              className={`rounded-[7px] px-3 py-1 font-medium transition-colors cursor-pointer ${
                subTab === "logs"
                  ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                  : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              }`}
            >
              {dict.monitor.tabLogs}
            </button>
          </div>

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
          {subTab === "conns" ? (
            <button
              type="button"
              onClick={onCloseAllConnections}
              className="ui-btn-danger-subtle h-7"
            >
              <X className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.monitor.closeAll}
            </button>
          ) : (
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

      {/* Body */}
      {subTab === "conns" ? (
        <div className="min-h-0 flex-1 overflow-auto">
          {filteredConns.length === 0 ? (
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
                  <th className="w-[78px] px-2 py-2 font-medium">
                    {dict.monitor.colInbound}
                  </th>
                  <th className="w-[136px] px-2 py-2 font-medium">
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
                {filteredConns.map(({ conn, displayProcess, ruleInfo, exitInfo, totalBytes }) => {
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
                          <span className="truncate font-medium text-[var(--text-primary)]">
                            {exitInfo.label}
                          </span>
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
      ) : (
        <div className="min-h-0 flex-1 overflow-auto bg-[var(--bg-subtle)] p-4 font-mono text-[11.5px]">
          {filteredLogs.length === 0 ? (
            <div className="py-12 text-center font-sans text-[12.5px] text-[var(--text-secondary)]">
              {dict.monitor.emptyLogs}
            </div>
          ) : (
            <div className="space-y-1.5">
              {filteredLogs.map(({ entry, displayMessage }, idx) => {
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
    </section>
  );
}
