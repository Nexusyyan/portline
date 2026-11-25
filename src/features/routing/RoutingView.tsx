import { useMemo, useState } from "react";
import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  Copy,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import { CustomSelect, type SelectGroup } from "../../components/CustomSelect";
import type { Dict } from "../../i18n";
import type {
  InboundPort,
  Locale,
  NodeIR,
  RouteMode,
  SimpleRoutingRule,
} from "../../types";
import {
  computeAvailableRegions,
  evaluateRouteSimulation,
  resolveTargetExit,
} from "../../utils/nodes";

interface RoutingViewProps {
  dict: Dict;
  locale: Locale;
  routeMode: RouteMode;
  nodes: NodeIR[];
  rules: SimpleRoutingRule[];
  extraPorts: InboundPort[];
  mainNodeId?: string | null;
  mixedPort: number;
  onSaveRule: (rule: SimpleRoutingRule) => void;
  onDeleteRule: (id: string) => void;
  onSavePort: (port: InboundPort) => void;
  onDeletePort: (id: string) => void;
  onGoToNodes: () => void;
  onCopyText: (text: string) => void;
  onInvalidPort: () => void;
}

export function RoutingView({
  dict,
  locale,
  routeMode,
  nodes,
  rules,
  extraPorts,
  mainNodeId,
  mixedPort,
  onSaveRule,
  onDeleteRule,
  onSavePort,
  onDeletePort,
  onGoToNodes,
  onCopyText,
  onInvalidPort,
}: RoutingViewProps) {
  const [simHost, setSimHost] = useState("chatgpt.com");
  const [portsOpen, setPortsOpen] = useState(true);
  const [showAddRule, setShowAddRule] = useState(false);

  const [newRuleName, setNewRuleName] = useState("");
  const [newRuleDomains, setNewRuleDomains] = useState("");
  const [newRuleTarget, setNewRuleTarget] = useState("auto:ALL");

  const [newPortNum, setNewPortNum] = useState("10809");
  const [newPortNote, setNewPortNote] = useState("");
  const [newPortTarget, setNewPortTarget] = useState("auto:ALL");

  const presets = dict.routing.presetTitles as Record<string, string>;
  const regionNames = dict.routing.regions as Record<string, string>;
  const availableRegions = useMemo(() => computeAvailableRegions(nodes), [nodes]);

  const targetLabels = useMemo(
    () => ({
      direct: dict.routing.optDirect,
      reject: dict.routing.optReject,
      noNode: dict.nav.noNodeSelected,
    }),
    [dict]
  );

  const buildSelectGroups = (
    currentValue: string,
    includeDirectReject: boolean
  ): SelectGroup[] => {
    const autoOptions = availableRegions.map((bucket) => {
      const name = regionNames[bucket.code] ?? bucket.code;
      return {
        value: `auto:${bucket.code}`,
        label: `${dict.routing.optAutoRegionPrefix} ${name}${dict.routing.optAutoRegionSuffix}`,
        meta: String(bucket.count),
      };
    });

    autoOptions.push({
      value: "auto:ALL",
      label: dict.routing.optAutoAll,
      meta: String(nodes.length),
    });

    // If a persisted rule points to an auto:<REGION> not currently present,
    // keep it visible cleanly as a 0-node fallback so the trigger never renders blank.
    if (
      currentValue.startsWith("auto:") &&
      currentValue !== "auto:ALL" &&
      !autoOptions.some((o) => o.value === currentValue)
    ) {
      const code = currentValue.slice("auto:".length).toUpperCase();
      const name = regionNames[code] ?? code;
      autoOptions.unshift({
        value: currentValue,
        label: `${dict.routing.optAutoRegionPrefix} ${name}${dict.routing.optAutoRegionSuffix}`,
        meta: "0",
      });
    }

    const actionOptions = [{ value: "main", label: dict.routing.optMain }];
    if (includeDirectReject) {
      actionOptions.push(
        { value: "direct", label: dict.routing.optDirect },
        { value: "reject", label: dict.routing.optReject }
      );
    }

    const groups: SelectGroup[] = [
      { label: dict.routing.optAutoGroup, options: autoOptions },
      { label: dict.routing.optActionsGroup, options: actionOptions },
    ];

    if (nodes.length > 0) {
      groups.push({
        label: dict.routing.optNodesGroup,
        options: nodes.map((n) => {
          const ms = n.realDelayMs ?? n.tcpingMs;
          return {
            value: n.id,
            label: n.remark,
            meta: ms != null ? `${ms} ms` : undefined,
          };
        }),
      });
    }

    return groups;
  };

  const simResult = useMemo(
    () =>
      evaluateRouteSimulation(
        simHost,
        routeMode,
        rules,
        nodes,
        mainNodeId,
        {
          ...targetLabels,
          defaultFallback: dict.routing.defaultFallback,
          directMode: dict.routing.directMode,
          globalMode: dict.routing.globalMode,
          matchedPrefix: dict.routing.matchedRule,
        },
        locale === "en",
        presets
      ),
    [simHost, routeMode, rules, nodes, mainNodeId, targetLabels, dict, locale, presets]
  );

  const mainNode = nodes.find((n) => n.id === mainNodeId) ?? nodes[0] ?? null;

  const handleConfirmRule = () => {
    const cleanName = newRuleName.trim();
    const patterns = newRuleDomains
      .split(/[,\s]+/)
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean);
    if (!cleanName || patterns.length === 0) return;

    const minPriority =
      rules.length > 0 ? Math.min(...rules.map((r) => r.priority)) - 1 : 1;
    onSaveRule({
      id: `r-custom-${Date.now()}`,
      title: cleanName,
      titleEn: cleanName,
      description: patterns.join(", "),
      patterns,
      processNames: [],
      ruleSet: null,
      target: newRuleTarget,
      enabled: true,
      priority: minPriority,
    });
    setNewRuleName("");
    setNewRuleDomains("");
    setShowAddRule(false);
  };

  const handleAddPort = () => {
    const port = Number.parseInt(newPortNum.trim(), 10);
    if (
      !Number.isFinite(port) ||
      port < 1024 ||
      port > 65535 ||
      port === mixedPort ||
      extraPorts.some((p) => p.port === port)
    ) {
      onInvalidPort();
      return;
    }
    const note = newPortNote.trim() || `Port ${port}`;
    onSavePort({
      id: `in-${port}`,
      port,
      note,
      noteEn: note,
      target: newPortTarget,
      enabled: true,
      allowLan: false,
    });
    setNewPortNum(String(port + 1));
    setNewPortNote("");
  };

  const activePortsList = [
    mixedPort,
    ...extraPorts.filter((p) => p.enabled).map((p) => p.port),
  ].join(", ");

  return (
    <section className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto w-full max-w-4xl space-y-5 px-6 py-5">
        {/* Header + Plain-Language Simulator */}
        <div className="space-y-3">
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 className="text-[16px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]">
                {dict.routing.title}
              </h1>
              <p className="mt-0.5 text-[12px] text-[var(--text-secondary)]">
                {dict.routing.subtitle}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setShowAddRule(true)}
              className="ui-btn-primary h-8"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.routing.addRule}
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-2.5 rounded-[12px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-3 text-[12px]">
            <span className="shrink-0 font-medium text-[var(--text-secondary)]">
              {dict.routing.simLabel}
            </span>
            <input
              type="text"
              value={simHost}
              onChange={(e) => setSimHost(e.target.value)}
              placeholder={dict.routing.simPlaceholder}
              className="ui-input h-7 w-44 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px]"
            />

            <div className="flex min-w-[200px] flex-1 items-center gap-1.5">
              <ArrowRight
                className="h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)]"
                strokeWidth={1.5}
              />
              <span className="rounded-[6px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2 py-0.5 font-medium text-[var(--text-primary)]">
                {simResult.ruleLabel}
              </span>
              <ArrowRight
                className="h-3.5 w-3.5 shrink-0 text-[var(--text-tertiary)]"
                strokeWidth={1.5}
              />
              <span
                className="truncate font-semibold"
                style={{
                  color:
                    simResult.exit.kind === "reject"
                      ? "var(--danger-text)"
                      : simResult.exit.kind === "direct"
                        ? "var(--text-secondary)"
                        : "var(--success-text)",
                }}
              >
                {simResult.exit.label}
              </span>
            </div>

            <div className="flex items-center gap-1">
              {["chatgpt.com", "netflix.com", "github.com", "baidu.com"].map(
                (sample) => (
                  <button
                    key={sample}
                    type="button"
                    onClick={() => setSimHost(sample)}
                    className="rounded-[6px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2 py-0.5 font-mono text-[11px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                  >
                    {sample}
                  </button>
                )
              )}
            </div>
          </div>
        </div>

        {/* One-Line Plain-Language Rules List */}
        <div className="ui-card overflow-hidden">
          <div className="grid grid-cols-12 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            <div className="col-span-5">{dict.routing.colWhen}</div>
            <div className="col-span-5">{dict.routing.colThen}</div>
            <div className="col-span-2 text-right">{dict.routing.colState}</div>
          </div>

          <div className="divide-y divide-[var(--border-hairline)]">
            {rules.map((r) => {
              const title = presets[r.id] ?? (locale === "en" ? r.titleEn : r.title);
              const isHit = simResult.matchedRuleId === r.id;
              const resolved = resolveTargetExit(
                r.target,
                nodes,
                mainNodeId,
                targetLabels
              );
              const showAutoHint = r.target.startsWith("auto:") && resolved.node !== null;
              return (
                <div
                  key={r.id}
                  className={`grid grid-cols-12 items-center gap-3 px-4 py-3 transition-shadow ${
                    r.enabled ? "" : "opacity-50"
                  } ${isHit ? "hit-ring" : ""}`}
                >
                  <div className="col-span-5 min-w-0">
                    <div className="truncate text-[12.5px] font-medium text-[var(--text-primary)]">
                      {title}
                    </div>
                    <div className="mt-0.5 truncate text-[11.5px] text-[var(--text-secondary)]">
                      {r.description}
                    </div>
                  </div>

                  <div className="col-span-5 space-y-1">
                    <CustomSelect
                      value={r.target}
                      groups={buildSelectGroups(r.target, true)}
                      onChange={(val) => onSaveRule({ ...r, target: val })}
                      searchPlaceholder={dict.routing.selectSearchPh}
                      emptyLabel={dict.routing.selectEmpty}
                      className="w-full"
                    />
                    {showAutoHint && resolved.node && (
                      <div
                        className="truncate pl-0.5 font-mono text-[11px] num-tabular"
                        style={{ color: "var(--success-text)" }}
                      >
                        {dict.routing.fastestNow} {resolved.node.remark}
                        {resolved.node.realDelayMs != null
                          ? ` (${resolved.node.realDelayMs} ms)`
                          : ""}
                      </div>
                    )}
                  </div>

                  <div className="col-span-2 flex items-center justify-end gap-2">
                    <button
                      type="button"
                      role="switch"
                      aria-checked={r.enabled}
                      onClick={() => onSaveRule({ ...r, enabled: !r.enabled })}
                      className={`relative h-[18px] w-8 rounded-full transition-colors cursor-pointer ${
                        r.enabled
                          ? "bg-[var(--accent-primary)]"
                          : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
                      }`}
                    >
                      <span
                        className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                        style={{ left: r.enabled ? "16px" : "2px" }}
                      />
                    </button>
                    <button
                      type="button"
                      aria-label={dict.nodes.actDelete}
                      onClick={() => onDeleteRule(r.id)}
                      className="ui-btn-danger-ghost h-7 w-7 p-0"
                    >
                      <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>

          {/* Fallback Row */}
          <div
            className={`grid grid-cols-12 items-center gap-3 border-t border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-4 py-3 ${
              routeMode === "rule" && simResult.matchedRuleId === null ? "hit-ring" : ""
            }`}
          >
            <div className="col-span-5">
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.routing.fallbackTitle}
              </div>
              <div className="mt-0.5 text-[11.5px] text-[var(--text-secondary)]">
                {dict.routing.fallbackDesc}
              </div>
            </div>

            <div className="col-span-5">
              <div className="flex items-center justify-between gap-2 rounded-[8px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2.5 py-1.5 text-[12px]">
                <span className="truncate font-medium text-[var(--text-primary)]">
                  {dict.routing.followMain}{" "}
                  {mainNode ? mainNode.remark : dict.nav.noNodeSelected}
                </span>
                <button
                  type="button"
                  onClick={onGoToNodes}
                  className="shrink-0 text-[11.5px] text-[var(--text-secondary)] underline hover:text-[var(--text-primary)] cursor-pointer"
                >
                  {dict.routing.pickInNodes}
                </button>
              </div>
            </div>

            <div className="col-span-2 text-right font-mono text-[11.5px] text-[var(--text-secondary)]">
              {dict.routing.alwaysOn}
            </div>
          </div>
        </div>

        {/* Collapsible Local Inbound Ports (Optional Advanced) */}
        <div className="ui-card overflow-hidden">
          <button
            type="button"
            onClick={() => setPortsOpen((v) => !v)}
            className="flex w-full items-center justify-between px-4 py-3 transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
          >
            <div className="flex items-center gap-2.5">
              <span className="text-[12.5px] font-semibold text-[var(--text-primary)]">
                {dict.routing.advPortsTitle}
              </span>
              <span className="rounded-full border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2 py-0.5 font-mono text-[11px] text-[var(--text-secondary)] num-tabular">
                {activePortsList}
              </span>
            </div>
            <div className="flex items-center gap-2 text-[11.5px] text-[var(--text-secondary)]">
              <span>{dict.routing.advPortsHint}</span>
              {portsOpen ? (
                <ChevronDown
                  className="h-4 w-4 text-[var(--text-tertiary)]"
                  strokeWidth={1.5}
                />
              ) : (
                <ChevronRight
                  className="h-4 w-4 text-[var(--text-tertiary)]"
                  strokeWidth={1.5}
                />
              )}
            </div>
          </button>

          {portsOpen && (
            <div className="divide-y divide-[var(--border-hairline)] border-t border-[var(--border-hairline)]">
              {/* Primary System Proxy Port */}
              <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3">
                <div className="flex items-center gap-3">
                  <span className="rounded-[6px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2 py-1 font-mono text-[12px] font-semibold text-[var(--text-primary)] num-tabular">
                    :{mixedPort}
                  </span>
                  <div>
                    <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                      {dict.routing.mainPortName}
                    </div>
                    <div className="text-[11.5px] text-[var(--text-secondary)]">
                      {dict.routing.mainPortDesc}
                    </div>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => onCopyText(`http://127.0.0.1:${mixedPort}`)}
                  className="ui-btn-secondary h-7 font-mono text-[11.5px]"
                >
                  <Copy
                    className="h-3 w-3 text-[var(--text-tertiary)]"
                    strokeWidth={1.5}
                  />
                  {dict.routing.copyUrl}
                </button>
              </div>

              {/* Extra Dedicated Ports */}
              {extraPorts.map((p) => (
                <div
                  key={p.id}
                  className="flex flex-wrap items-center justify-between gap-3 px-4 py-3"
                >
                  <div className="flex items-center gap-3">
                    <span className="rounded-[6px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2 py-1 font-mono text-[12px] font-semibold text-[var(--text-primary)] num-tabular">
                      :{p.port}
                    </span>
                    <div>
                      <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                        {locale === "en" ? p.noteEn : p.note}
                      </div>
                      <div className="text-[11.5px] text-[var(--text-secondary)]">
                        {dict.routing.extraPortDesc}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <CustomSelect
                      value={p.target}
                      groups={buildSelectGroups(p.target, false)}
                      onChange={(val) => onSavePort({ ...p, target: val })}
                      searchPlaceholder={dict.routing.selectSearchPh}
                      emptyLabel={dict.routing.selectEmpty}
                      size="sm"
                      className="w-52"
                    />
                    <button
                      type="button"
                      onClick={() => onCopyText(`http://127.0.0.1:${p.port}`)}
                      className="ui-btn-secondary h-7 px-2 font-mono text-[11px]"
                    >
                      URL
                    </button>
                    <button
                      type="button"
                      onClick={() => onDeletePort(p.id)}
                      className="ui-btn-danger-ghost h-7 w-7 p-0"
                    >
                      <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                    </button>
                  </div>
                </div>
              ))}

              {/* Compact Inline Add Port Strip */}
              <div className="flex flex-wrap items-center gap-2 bg-[var(--bg-subtle)] px-4 py-2.5 text-[12px]">
                <span className="font-medium text-[var(--text-secondary)]">
                  {dict.routing.addPortLabel}
                </span>
                <input
                  type="text"
                  inputMode="numeric"
                  value={newPortNum}
                  onChange={(e) =>
                    setNewPortNum(e.target.value.replace(/[^0-9]/g, "").slice(0, 5))
                  }
                  className="ui-input h-7 w-20 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px] num-tabular"
                />
                <input
                  type="text"
                  value={newPortNote}
                  onChange={(e) => setNewPortNote(e.target.value)}
                  placeholder={dict.routing.portNotePh}
                  className="ui-input h-7 w-44 bg-[var(--bg-surface)] px-2.5 text-[12px]"
                />
                <CustomSelect
                  value={newPortTarget}
                  groups={buildSelectGroups(newPortTarget, false)}
                  onChange={setNewPortTarget}
                  searchPlaceholder={dict.routing.selectSearchPh}
                  emptyLabel={dict.routing.selectEmpty}
                  size="sm"
                  className="w-52 bg-[var(--bg-surface)]"
                />
                <button
                  type="button"
                  onClick={handleAddPort}
                  className="ui-btn-primary h-7 px-3"
                >
                  {dict.routing.btnAdd}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Add Rule Modal */}
      {showAddRule && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ backgroundColor: "var(--backdrop-scrim)" }}
        >
          <div className="ui-modal w-full max-w-md overflow-hidden">
            <div className="flex items-center justify-between border-b border-[var(--border-hairline)] px-5 py-3.5">
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                {dict.routing.addRuleTitle}
              </h3>
              <button
                type="button"
                onClick={() => setShowAddRule(false)}
                className="ui-btn-ghost h-7 w-7 p-0"
              >
                <X className="h-4 w-4" strokeWidth={1.5} />
              </button>
            </div>
            <div className="space-y-3.5 p-5 text-[12px]">
              <div className="space-y-1">
                <label className="block font-medium text-[var(--text-primary)]">
                  {dict.routing.ruleNameLabel}
                </label>
                <input
                  type="text"
                  value={newRuleName}
                  onChange={(e) => setNewRuleName(e.target.value)}
                  placeholder={dict.routing.ruleNamePh}
                  className="ui-input h-8 w-full px-3"
                />
              </div>
              <div className="space-y-1">
                <label className="block font-medium text-[var(--text-primary)]">
                  {dict.routing.ruleDomainsLabel}
                </label>
                <input
                  type="text"
                  value={newRuleDomains}
                  onChange={(e) => setNewRuleDomains(e.target.value)}
                  placeholder={dict.routing.ruleDomainsPh}
                  className="ui-input h-8 w-full px-3 font-mono"
                />
              </div>
              <div className="space-y-1">
                <label className="block font-medium text-[var(--text-primary)]">
                  {dict.routing.ruleExitLabel}
                </label>
                <CustomSelect
                  value={newRuleTarget}
                  groups={buildSelectGroups(newRuleTarget, true)}
                  onChange={setNewRuleTarget}
                  searchPlaceholder={dict.routing.selectSearchPh}
                  emptyLabel={dict.routing.selectEmpty}
                  className="w-full"
                />
              </div>
              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setShowAddRule(false)}
                  className="ui-btn-secondary"
                >
                  {dict.nodes.cancel}
                </button>
                <button
                  type="button"
                  onClick={handleConfirmRule}
                  className="ui-btn-primary"
                >
                  {dict.routing.btnAdd}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
