import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowUpDown,
  Download,
  FolderOpen,
  Gauge,
  Link2,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Wifi,
  X,
} from "lucide-react";
import type { Dict } from "../../i18n";
import type { CoreInfo, NodeIR, SimpleRoutingRule, SubscriptionGroup } from "../../types";
import {
  delayColorVar,
  filterAndSortNodes,
  PROTO_BADGES,
  resolveTargetExit,
  SAMPLE_SHARE_URIS,
  type NodeSortMode,
} from "../../utils/nodes";

interface NodesViewProps {
  dict: Dict;
  coreInfo: CoreInfo;
  nodes: NodeIR[];
  subscriptions: SubscriptionGroup[];
  rules: SimpleRoutingRule[];
  mainNodeId?: string | null;
  testingKind: "tcping" | "real" | null;
  downloadingCore: boolean;
  onDownloadCore: () => Promise<void>;
  onImportCoreBinary: () => Promise<void>;
  onSelectMainNode: (id: string) => void;
  onDeleteNode: (id: string) => void;
  onRunBatchTest: (kind: "tcping" | "real", nodeIds: string[]) => void;
  onImportPayload: (payload: string, groupName: string) => Promise<void>;
  onImportSubscription: (name: string, url: string) => Promise<void>;
  onRefreshSubscription: (id: string) => Promise<void>;
  onDeleteSubscription: (id: string) => Promise<void>;
}

export function NodesView({
  dict,
  coreInfo,
  nodes,
  subscriptions,
  rules,
  mainNodeId,
  testingKind,
  downloadingCore,
  onDownloadCore,
  onImportCoreBinary,
  onSelectMainNode,
  onDeleteNode,
  onRunBatchTest,
  onImportPayload,
  onImportSubscription,
  onRefreshSubscription,
  onDeleteSubscription,
}: NodesViewProps) {
  const [activeGroup, setActiveGroup] = useState<string>("ALL");
  const [query, setQuery] = useState<string>("");
  const [sortMode, setSortMode] = useState<NodeSortMode>("default");
  const [showImport, setShowImport] = useState(false);
  const [importTab, setImportTab] = useState<"paste" | "sub">("paste");
  const [importGroup, setImportGroup] = useState("");
  const [importPayload, setImportPayload] = useState("");
  const [subName, setSubName] = useState("");
  const [subUrl, setSubUrl] = useState("");
  const [importing, setImporting] = useState(false);
  const [showSubs, setShowSubs] = useState(false);
  const [refreshingSubId, setRefreshingSubId] = useState<string | null>(null);

  const groups = useMemo(() => {
    const names = Array.from(new Set(nodes.map((n) => n.group)));
    return ["ALL", ...names];
  }, [nodes]);

  const visibleNodes = useMemo(
    () => filterAndSortNodes(nodes, activeGroup, query, sortMode),
    [nodes, activeGroup, query, sortMode]
  );

  const ruleUsageMap = useMemo(() => {
    const map = new Map<string, string[]>();
    const labels = {
      direct: dict.routing.optDirect,
      reject: dict.routing.optReject,
      noNode: dict.nav.noNodeSelected,
    };
    const presets = dict.routing.presetTitles as Record<string, string>;
    for (const r of rules) {
      if (!r.enabled || r.target === "direct" || r.target === "reject" || r.target === "main") {
        continue;
      }
      const res = resolveTargetExit(r.target, nodes, mainNodeId, labels);
      if (res.node) {
        const title = presets[r.id] ?? r.title;
        const arr = map.get(res.node.id) ?? [];
        arr.push(title);
        map.set(res.node.id, arr);
      }
    }
    return map;
  }, [rules, nodes, mainNodeId, dict]);

  const scrollRef = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: visibleNodes.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 44,
    overscan: 12,
  });

  const cycleSort = () => {
    setSortMode((prev) =>
      prev === "default" ? "delay" : prev === "delay" ? "name" : "default"
    );
  };

  const sortLabel =
    sortMode === "delay"
      ? dict.nodes.sortDelay
      : sortMode === "name"
        ? dict.nodes.sortName
        : dict.nodes.sortDefault;

  const handleConfirmImport = async () => {
    setImporting(true);
    try {
      if (importTab === "paste") {
        if (!importPayload.trim()) return;
        await onImportPayload(
          importPayload,
          importGroup.trim() || dict.nodes.groupNamePlaceholder
        );
        setImportPayload("");
        setImportGroup("");
        setShowImport(false);
      } else {
        if (!subUrl.trim()) return;
        await onImportSubscription(
          subName.trim() || dict.nodes.subNamePlaceholder,
          subUrl.trim()
        );
        setSubName("");
        setSubUrl("");
        setShowImport(false);
      }
    } finally {
      setImporting(false);
    }
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col">
      {/* Toolbar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-[var(--border-hairline)] px-5 py-3">
        <div className="flex flex-wrap items-center gap-1.5">
          {groups.map((g) => {
            const active = activeGroup === g;
            const count =
              g === "ALL" ? nodes.length : nodes.filter((n) => n.group === g).length;
            return (
              <button
                key={g}
                type="button"
                onClick={() => setActiveGroup(g)}
                className={`rounded-[8px] px-2.5 py-1 text-[12px] font-medium transition-colors cursor-pointer ${
                  active
                    ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                    : "border border-[var(--border-hairline)] bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                }`}
              >
                <span>{g === "ALL" ? dict.nodes.groupAll : g}</span>
                <span className="ml-1.5 font-mono text-[11px] opacity-70 num-tabular">
                  {count}
                </span>
              </button>
            );
          })}
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <div className="relative">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-tertiary)]"
              strokeWidth={1.5}
            />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={dict.nodes.searchPlaceholder}
              className="ui-input h-8 w-56 pr-3 pl-8 text-[12px]"
            />
          </div>

          <button
            type="button"
            disabled={testingKind !== null || visibleNodes.length === 0}
            onClick={() =>
              onRunBatchTest(
                "tcping",
                visibleNodes.map((n) => n.id)
              )
            }
            className="ui-btn-secondary h-8"
          >
            <Wifi className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            {dict.nodes.tcping}
          </button>

          <button
            type="button"
            disabled={testingKind !== null || visibleNodes.length === 0}
            onClick={() =>
              onRunBatchTest(
                "real",
                visibleNodes.map((n) => n.id)
              )
            }
            className="ui-btn-primary h-8"
          >
            <Gauge className="h-3.5 w-3.5" strokeWidth={1.5} />
            {dict.nodes.realDelay}
          </button>

          <button
            type="button"
            onClick={() => setShowSubs(true)}
            className="ui-btn-secondary h-8"
          >
            <Link2 className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            <span>{dict.nodes.subsBtn}</span>
            {subscriptions.length > 0 && (
              <span className="font-mono text-[11px] text-[var(--text-tertiary)] num-tabular">
                {subscriptions.length}
              </span>
            )}
          </button>

          <button
            type="button"
            onClick={() => setShowImport(true)}
            className="ui-btn-secondary h-8"
          >
            <Plus className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            {dict.nodes.importBtn}
          </button>
        </div>
      </div>

      {/* Offline-First Core Setup Banner when official sing-box is not yet present */}
      {!coreInfo.installed && (
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-5 py-2 text-[12px]">
          <span className="text-[var(--text-secondary)]">
            {dict.nodes.coreBannerText}
          </span>
          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              onClick={() => void onImportCoreBinary()}
              className="ui-btn-primary h-7 text-[11.5px]"
            >
              <FolderOpen className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.nodes.coreBannerImportBtn}
            </button>
            <button
              type="button"
              disabled={downloadingCore}
              onClick={() => void onDownloadCore()}
              className="ui-btn-secondary h-7 text-[11.5px]"
            >
              <Download className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
              {downloadingCore
                ? dict.nodes.coreBannerDownloading
                : dict.nodes.coreBannerBtn}
            </button>
          </div>
        </div>
      )}

      {/* Non-blocking Test Progress Strip */}
      {testingKind && (
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-5 py-2 text-[12px]">
          <span className="flex items-center gap-2 text-[var(--text-primary)]">
            <span
              className="h-2 w-2 animate-pulse rounded-full"
              style={{ backgroundColor: "var(--warning-dot)" }}
            />
            {dict.nodes.testingProgress}
          </span>
          <span className="font-mono text-[11px] text-[var(--text-secondary)] uppercase">
            {testingKind}
          </span>
        </div>
      )}

      {/* Subtle Tip + Sort Strip */}
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-5 py-1.5 text-[11.5px] text-[var(--text-secondary)]">
        <span className="truncate">{dict.nodes.hint}</span>
        <button
          type="button"
          onClick={cycleSort}
          className="inline-flex shrink-0 items-center gap-1 font-mono text-[11.5px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] cursor-pointer"
        >
          <ArrowUpDown className="h-3 w-3 text-[var(--text-tertiary)]" strokeWidth={1.5} />
          {sortLabel}
        </button>
      </div>

      {/* Table Header */}
      {visibleNodes.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
          <div className="text-[14px] font-medium text-[var(--text-primary)]">
            {dict.nodes.emptyTitle}
          </div>
          <p className="max-w-md text-[12.5px] text-[var(--text-secondary)]">
            {dict.nodes.emptyDesc}
          </p>
          <div className="mt-1 flex items-center gap-2">
            <button
              type="button"
              onClick={() => setShowImport(true)}
              className="ui-btn-primary"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.nodes.importBtn}
            </button>
            <button
              type="button"
              onClick={() =>
                void onImportPayload(SAMPLE_SHARE_URIS, dict.nodes.groupNamePlaceholder)
              }
              className="ui-btn-secondary"
            >
              {dict.nodes.importSample}
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="grid shrink-0 grid-cols-12 items-center border-b border-[var(--border-hairline)] bg-[var(--bg-canvas)] px-5 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            <div className="col-span-1">{dict.nodes.colProto}</div>
            <div className="col-span-4">{dict.nodes.colName}</div>
            <div className="col-span-2 hidden md:block">{dict.nodes.colServer}</div>
            <div className="col-span-1 hidden sm:block">{dict.nodes.colGroup}</div>
            <div className="col-span-1 text-right">{dict.nodes.colTcping}</div>
            <div className="col-span-1 text-right">{dict.nodes.colReal}</div>
            <div className="col-span-2 text-right">{dict.nodes.colAction}</div>
          </div>

          {/* Virtualized Rows */}
          <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
            <div
              style={{
                height: `${virtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {virtualizer.getVirtualItems().map((vRow) => {
                const n = visibleNodes[vRow.index];
                const isMain = n.id === mainNodeId;
                const tags = ruleUsageMap.get(n.id) ?? [];
                return (
                  <div
                    key={n.id}
                    onDoubleClick={() => onSelectMainNode(n.id)}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: `${vRow.size}px`,
                      transform: `translateY(${vRow.start}px)`,
                    }}
                    className={`group grid grid-cols-12 items-center border-b border-[var(--border-hairline)] px-5 text-[12px] transition-colors cursor-pointer ${
                      isMain
                        ? "bg-[var(--bg-selected)]"
                        : "hover:bg-[var(--bg-hover)]"
                    }`}
                  >
                    <div className="col-span-1 flex items-center gap-2">
                      <span
                        className="h-1.5 w-1.5 shrink-0 rounded-full"
                        style={{
                          backgroundColor: isMain
                            ? "var(--success-dot)"
                            : "var(--border-strong)",
                        }}
                      />
                      <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[10.5px] font-medium text-[var(--text-primary)]">
                        {PROTO_BADGES[n.protocol]}
                      </span>
                    </div>

                    <div className="col-span-4 flex min-w-0 items-center gap-1.5 pr-2">
                      <span className="truncate font-medium text-[var(--text-primary)]">
                        {n.remark}
                      </span>
                      {tags.map((tag) => (
                        <span
                          key={tag}
                          className="shrink-0 rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-1.5 py-0.5 text-[10.5px] text-[var(--text-secondary)]"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>

                    <div className="col-span-2 hidden truncate pr-2 font-mono text-[11.5px] text-[var(--text-secondary)] md:block">
                      {n.server}:{n.port}
                    </div>

                    <div className="col-span-1 hidden truncate pr-2 text-[11.5px] text-[var(--text-secondary)] sm:block">
                      {n.group}
                    </div>

                    <div
                      className="col-span-1 text-right font-mono text-[11.5px] num-tabular"
                      style={{ color: delayColorVar(n.tcpingMs) }}
                    >
                      {n.tcpingMs != null ? `${n.tcpingMs} ms` : "--"}
                    </div>

                    <div
                      className="col-span-1 text-right font-mono text-[11.5px] font-medium num-tabular"
                      style={{ color: delayColorVar(n.realDelayMs) }}
                    >
                      {n.realDelayMs != null ? `${n.realDelayMs} ms` : "--"}
                    </div>

                    <div className="col-span-2 flex items-center justify-end gap-1">
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onSelectMainNode(n.id);
                        }}
                        className={`rounded-[7px] px-2.5 py-1 text-[11.5px] font-medium transition-colors cursor-pointer ${
                          isMain
                            ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                            : "border border-[var(--border-hairline)] bg-[var(--bg-surface)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
                        }`}
                      >
                        {isMain ? dict.nodes.actActive : dict.nodes.actUse}
                      </button>
                      <button
                        type="button"
                        aria-label={dict.nodes.actDelete}
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteNode(n.id);
                        }}
                        className="ui-btn-danger-ghost h-7 w-7 p-0 opacity-60 group-hover:opacity-100"
                      >
                        <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}

      {/* Import Modal */}
      {showImport && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ backgroundColor: "var(--backdrop-scrim)" }}
        >
          <div className="ui-modal w-full max-w-lg overflow-hidden">
            <div className="flex items-center justify-between border-b border-[var(--border-hairline)] px-5 py-3.5">
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                {dict.nodes.importTitle}
              </h3>
              <button
                type="button"
                onClick={() => setShowImport(false)}
                className="ui-btn-ghost h-7 w-7 p-0"
              >
                <X className="h-4 w-4" strokeWidth={1.5} />
              </button>
            </div>

            <div className="space-y-3.5 p-5">
              <div className="inline-flex rounded-[9px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5 text-[12px]">
                <button
                  type="button"
                  onClick={() => setImportTab("paste")}
                  className={`rounded-[7px] px-3 py-1 font-medium cursor-pointer ${
                    importTab === "paste"
                      ? "bg-[var(--bg-surface)] text-[var(--text-primary)]"
                      : "text-[var(--text-secondary)]"
                  }`}
                >
                  {dict.nodes.tabPaste}
                </button>
                <button
                  type="button"
                  onClick={() => setImportTab("sub")}
                  className={`rounded-[7px] px-3 py-1 font-medium cursor-pointer ${
                    importTab === "sub"
                      ? "bg-[var(--bg-surface)] text-[var(--text-primary)]"
                      : "text-[var(--text-secondary)]"
                  }`}
                >
                  {dict.nodes.tabSub}
                </button>
              </div>

              {importTab === "paste" ? (
                <div className="space-y-3">
                  <div className="space-y-1">
                    <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                      {dict.nodes.groupNameLabel}
                    </label>
                    <input
                      type="text"
                      value={importGroup}
                      onChange={(e) => setImportGroup(e.target.value)}
                      placeholder={dict.nodes.groupNamePlaceholder}
                      className="ui-input h-8 w-full px-3 text-[12px]"
                    />
                  </div>
                  <textarea
                    rows={5}
                    value={importPayload}
                    onChange={(e) => setImportPayload(e.target.value)}
                    placeholder={dict.nodes.payloadPlaceholder}
                    className="ui-input w-full p-3 font-mono text-[11.5px]"
                  />
                </div>
              ) : (
                <div className="space-y-3">
                  <div className="space-y-1">
                    <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                      {dict.nodes.subNameLabel}
                    </label>
                    <input
                      type="text"
                      value={subName}
                      onChange={(e) => setSubName(e.target.value)}
                      placeholder={dict.nodes.subNamePlaceholder}
                      className="ui-input h-8 w-full px-3 text-[12px]"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                      {dict.nodes.subUrlLabel}
                    </label>
                    <input
                      type="url"
                      value={subUrl}
                      onChange={(e) => setSubUrl(e.target.value)}
                      placeholder={dict.nodes.subUrlPlaceholder}
                      className="ui-input h-8 w-full px-3 font-mono text-[12px]"
                    />
                  </div>
                </div>
              )}

              <div className="flex items-center justify-between pt-1">
                {importTab === "paste" ? (
                  <button
                    type="button"
                    onClick={() => setImportPayload(SAMPLE_SHARE_URIS)}
                    className="text-[12px] text-[var(--text-secondary)] underline hover:text-[var(--text-primary)] cursor-pointer"
                  >
                    {dict.nodes.fillSample}
                  </button>
                ) : (
                  <span />
                )}
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setShowImport(false)}
                    className="ui-btn-secondary"
                  >
                    {dict.nodes.cancel}
                  </button>
                  <button
                    type="button"
                    disabled={importing}
                    onClick={() => void handleConfirmImport()}
                    className="ui-btn-primary"
                  >
                    {importTab === "paste"
                      ? dict.nodes.confirmImport
                      : dict.nodes.confirmSync}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Subscriptions Manager Modal */}
      {showSubs && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ backgroundColor: "var(--backdrop-scrim)" }}
        >
          <div className="ui-modal w-full max-w-lg overflow-hidden">
            <div className="flex items-center justify-between border-b border-[var(--border-hairline)] px-5 py-3.5">
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                {dict.nodes.subsTitle}
              </h3>
              <button
                type="button"
                onClick={() => setShowSubs(false)}
                className="ui-btn-ghost h-7 w-7 p-0"
              >
                <X className="h-4 w-4" strokeWidth={1.5} />
              </button>
            </div>
            <div className="max-h-80 overflow-auto p-5">
              {subscriptions.length === 0 ? (
                <div className="py-6 text-center text-[12.5px] text-[var(--text-secondary)]">
                  {dict.nodes.subsEmpty}
                </div>
              ) : (
                <div className="space-y-2">
                  {subscriptions.map((sub) => (
                    <div
                      key={sub.id}
                      className="ui-card flex items-center justify-between gap-3 p-3"
                    >
                      <div className="min-w-0">
                        <div className="flex items-center gap-2 text-[12.5px] font-medium text-[var(--text-primary)]">
                          <span className="truncate">{sub.name}</span>
                          <span className="font-mono text-[11px] text-[var(--text-secondary)] num-tabular">
                            {sub.nodeCount} {dict.nodes.subsNodes}
                          </span>
                        </div>
                        <div className="truncate font-mono text-[11px] text-[var(--text-tertiary)]">
                          {sub.url}
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        <button
                          type="button"
                          disabled={refreshingSubId === sub.id}
                          onClick={async () => {
                            setRefreshingSubId(sub.id);
                            try {
                              await onRefreshSubscription(sub.id);
                            } finally {
                              setRefreshingSubId(null);
                            }
                          }}
                          className="ui-btn-secondary h-7 px-2 text-[11.5px]"
                        >
                          <RefreshCw className="h-3 w-3" strokeWidth={1.5} />
                          {dict.nodes.subsRefresh}
                        </button>
                        <button
                          type="button"
                          onClick={() => void onDeleteSubscription(sub.id)}
                          className="ui-btn-danger-ghost h-7 w-7 p-0"
                        >
                          <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
