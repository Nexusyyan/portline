import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Search, Trash2, X } from "lucide-react";
import type { Dict } from "../../i18n";
import type { ConnectionItem, LogEntry, TrafficSample } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/nodes";

interface MonitorViewProps {
  dict: Dict;
  coreRunning: boolean;
  connections: ConnectionItem[];
  logs: LogEntry[];
  traffic: TrafficSample;
  downHistory: number[];
  upHistory: number[];
  onCloseConnection: (id: string) => void;
  onCloseAllConnections: () => void;
  onClearLogs: () => void;
}

export function MonitorView({
  dict,
  coreRunning,
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
  const [query, setQuery] = useState("");
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

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

  const filteredConns = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return connections;
    return connections.filter(
      (c) =>
        c.process.toLowerCase().includes(q) ||
        c.destination.toLowerCase().includes(q) ||
        c.rule.toLowerCase().includes(q)
    );
  }, [connections, query]);

  const filteredLogs = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return logs;
    return logs.filter(
      (l) =>
        l.message.toLowerCase().includes(q) || l.level.toLowerCase().includes(q)
    );
  }, [logs, query]);

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
            <div className="text-[11px] text-[var(--text-secondary)]">
              {dict.monitor.kpiCore}
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
            <div className="text-[11px] text-[var(--text-secondary)]">
              {dict.monitor.kpiLogRing}
            </div>
            <div className="mt-0.5 font-mono text-[15px] font-semibold num-tabular text-[var(--text-primary)]">
              {logs.length}/500
            </div>
          </div>
        </div>
      </div>

      {/* Sub-Tabs & Filter Bar */}
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--border-hairline)] px-5 py-2.5">
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
              className="ui-input h-7 w-48 pr-2.5 pl-8 text-[12px]"
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
            <table className="w-full border-collapse text-left">
              <thead className="sticky top-0 z-10 border-b border-[var(--border-hairline)] bg-[var(--bg-canvas)] text-[11.5px] font-medium text-[var(--text-secondary)]">
                <tr>
                  <th className="py-2 pr-2 pl-5 font-medium">{dict.monitor.colProcess}</th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colDest}</th>
                  <th className="w-24 px-2 py-2 font-medium">{dict.monitor.colInbound}</th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colRule}</th>
                  <th className="px-2 py-2 font-medium">{dict.monitor.colExit}</th>
                  <th className="w-28 px-2 py-2 text-right font-medium">
                    {dict.monitor.colTraffic}
                  </th>
                  <th className="w-10 py-2 pr-5 pl-2" />
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--border-hairline)] text-[12px]">
                {filteredConns.map((c) => (
                  <tr key={c.id} className="hover:bg-[var(--bg-hover)]">
                    <td className="py-2.5 pr-2 pl-5 font-mono font-medium text-[var(--text-primary)]">
                      {c.process}
                    </td>
                    <td className="px-2 py-2.5 font-mono text-[var(--text-secondary)]">
                      {c.destination}
                    </td>
                    <td className="px-2 py-2.5 font-mono text-[11px]">
                      <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5">
                        {c.inbound}
                      </span>
                    </td>
                    <td className="px-2 py-2.5 text-[var(--text-secondary)]">{c.rule}</td>
                    <td className="px-2 py-2.5 font-medium text-[var(--text-primary)]">
                      {c.chain[0] ?? "direct"}
                    </td>
                    <td className="px-2 py-2.5 text-right font-mono text-[11.5px] num-tabular text-[var(--text-secondary)]">
                      {formatBytes(c.uploadBytes + c.downloadBytes)}
                    </td>
                    <td className="py-2.5 pr-5 pl-2 text-right">
                      <button
                        type="button"
                        onClick={() => onCloseConnection(c.id)}
                        className="ui-btn-danger-ghost h-6 w-6 p-0"
                      >
                        <X className="h-3.5 w-3.5" strokeWidth={1.5} />
                      </button>
                    </td>
                  </tr>
                ))}
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
              {filteredLogs.map((entry, idx) => {
                const levelColor =
                  entry.level === "ERROR"
                    ? "var(--danger-text)"
                    : entry.level === "WARN"
                      ? "var(--warning-text)"
                      : "var(--success-text)";
                return (
                  <div key={`${entry.timestamp}-${idx}`} className="flex gap-2.5">
                    <span className="shrink-0 num-tabular text-[var(--text-tertiary)]">
                      {entry.timestamp}
                    </span>
                    <span
                      className="w-11 shrink-0 font-semibold"
                      style={{ color: levelColor }}
                    >
                      {entry.level}
                    </span>
                    <span className="break-all text-[var(--text-primary)]">
                      {entry.message}
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
