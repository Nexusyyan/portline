import { useEffect, useMemo, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Database,
  Layers,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { CustomSelect, type SelectGroup } from "../../components/CustomSelect";
import { NodeLabel, RegionFlag, parseFlagText } from "../../components/RegionFlag";
import type { Dict } from "../../i18n";
import type {
  InboundPort,
  Locale,
  NodeIR,
  RouteMode,
  RouteTestResult,
  RulePresetSpec,
  RuleSetStatus,
  SimpleRoutingRule,
} from "../../types";
import {
  computeAvailableRegions,
  evaluateRouteSimulation,
  formatBytes,
  resolveTargetExit,
} from "../../utils/nodes";

interface RoutingViewProps {
  dict: Dict;
  locale: Locale;
  routeMode: RouteMode;
  nodes: NodeIR[];
  rules: SimpleRoutingRule[];
  rulePresets: RulePresetSpec[];
  ruleSets: RuleSetStatus[];
  extraPorts: InboundPort[];
  mainNodeId?: string | null;
  mixedPort: number;
  syncingRuleSets: boolean;
  applyingStrategy: boolean;
  onSaveRule: (rule: SimpleRoutingRule) => void;
  onDeleteRule: (id: string) => void;
  onSyncRuleSets: (tags: string[]) => Promise<void>;
  onApplyStrategy: (strategyId: string) => Promise<void>;
  onTestRoute: (input: string, processName?: string) => Promise<RouteTestResult | null>;
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
  rulePresets,
  ruleSets,
  extraPorts,
  mainNodeId,
  mixedPort,
  syncingRuleSets,
  applyingStrategy,
  onSaveRule,
  onDeleteRule,
  onSyncRuleSets,
  onApplyStrategy,
  onTestRoute,
  onSavePort,
  onDeletePort,
  onGoToNodes,
  onCopyText,
  onInvalidPort,
}: RoutingViewProps) {
  const [testInput, setTestInput] = useState("chatgpt.com");
  const [testProcess, setTestProcess] = useState("");
  const [testResult, setTestResult] = useState<RouteTestResult | null>(null);
  const [testingRoute, setTestingRoute] = useState(false);

  const [portsOpen, setPortsOpen] = useState(false);
  const [showPresetModal, setShowPresetModal] = useState(false);
  const [presetCategory, setPresetCategory] = useState<string>("ALL");
  const [presetQuery, setPresetQuery] = useState("");

  const [editingRuleId, setEditingRuleId] = useState<string | null>(null);
  const [showRuleModal, setShowRuleModal] = useState(false);
  const [ruleName, setRuleName] = useState("");
  const [ruleDomains, setRuleDomains] = useState("");
  const [ruleKeywords, setRuleKeywords] = useState("");
  const [ruleCidrs, setRuleCidrs] = useState("");
  const [ruleProcesses, setRuleProcesses] = useState("");
  const [ruleSetTag, setRuleSetTag] = useState("");
  const [rulePriority, setRulePriority] = useState("15");
  const [ruleTarget, setRuleTarget] = useState("auto:ALL");

  const [newPortNum, setNewPortNum] = useState("10809");
  const [newPortNote, setNewPortNote] = useState("");
  const [newPortTarget, setNewPortTarget] = useState("auto:ALL");

  const presets = dict.routing.presetTitles as Record<string, string>;
  const presetCatLabels = dict.routing.presetCategories as Record<string, string>;
  const regionNames = dict.routing.regions as Record<string, string>;
  const availableRegions = useMemo(() => computeAvailableRegions(nodes), [nodes]);

  const ruleSetMap = useMemo(
    () => new Map<string, RuleSetStatus>(ruleSets.map((s) => [s.tag, s])),
    [ruleSets]
  );

  const existingRuleIds = useMemo(() => new Set(rules.map((r) => r.id)), [rules]);

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
        icon: <RegionFlag code={bucket.code} size="xs" />,
      };
    });

    autoOptions.push({
      value: "auto:ALL",
      label: dict.routing.optAutoAll,
      meta: String(nodes.length),
      icon: <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-tertiary)]" />,
    });

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
        icon: <RegionFlag code={code} size="xs" />,
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
          const parsed = parseFlagText(n.remark, n.region);
          return {
            value: n.id,
            label: parsed.cleanText,
            meta: ms != null ? `${ms} ms` : undefined,
            icon: parsed.isoCode ? <RegionFlag code={parsed.isoCode} size="xs" /> : undefined,
          };
        }),
      });
    }

    return groups;
  };

  // Fallback client-side simulation when running in non-desktop browser preview.
  const fallbackSim = useMemo(
    () =>
      evaluateRouteSimulation(
        testInput,
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
    [testInput, routeMode, rules, nodes, mainNodeId, targetLabels, dict, locale, presets]
  );

  const runRouteTest = async (overrideInput?: string) => {
    const target = (overrideInput ?? testInput).trim();
    if (!target) return;
    setTestingRoute(true);
    try {
      const res = await onTestRoute(target, testProcess.trim() || undefined);
      if (res) {
        setTestResult(res);
      }
    } finally {
      setTestingRoute(false);
    }
  };

  useEffect(() => {
    void runRouteTest(testInput);
    // Re-evaluate automatically when mode or rule list changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routeMode, rules.length, mainNodeId]);

  const activeMatchedRuleId =
    testResult !== null ? (testResult.matchedRuleId ?? null) : fallbackSim.matchedRuleId;

  const displayMatchedRule = (() => {
    if (!testResult) return fallbackSim.ruleLabel;
    if (testResult.isPrivateIp) return dict.routing.lanDirect;
    if (testResult.routeMode === "direct") return dict.routing.directMode;
    if (testResult.routeMode === "global") return dict.routing.globalMode;
    if (testResult.matchedRuleId) {
      const r = rules.find((x) => x.id === testResult.matchedRuleId);
      if (r) return presets[r.id] ?? (locale === "en" ? r.titleEn : r.title);
      return testResult.matchedRuleTitle ?? testResult.matchedRuleId;
    }
    return dict.routing.defaultFallback;
  })();

  const displayExitKind = testResult ? testResult.resolvedExitKind : fallbackSim.exit.kind;
  const displayExitLabel = (() => {
    if (!testResult) return fallbackSim.exit.label;
    if (testResult.resolvedExitKind === "direct") return dict.routing.optDirect;
    if (testResult.resolvedExitKind === "reject") return dict.routing.optReject;
    if (testResult.resolvedExitKind === "none") return dict.nav.noNodeSelected;
    return testResult.resolvedExitLabel;
  })();
  const displayExitRegion = testResult
    ? testResult.resolvedNodeRegion
    : fallbackSim.exit.node?.region;

  const displayDns = (() => {
    if (!testResult) return "--";
    if (testResult.dnsServer === "none") return "IP";
    if (testResult.dnsServer === "dns-local") return dict.dns.ruleServerLocal;
    if (testResult.dnsServer === "dns-remote") return dict.dns.ruleServerRemote;
    if (testResult.dnsServer === "dns-fakeip") return dict.dns.ruleServerFakeip;
    if (testResult.dnsServer === "dns-hosts") return dict.dns.flowHosts;
    if (testResult.dnsServer === "reject") return dict.dns.ruleServerReject;
    return testResult.dnsServer;
  })();

  const openAddModal = () => {
    const minPriority =
      rules.length > 0 ? Math.min(...rules.map((r) => r.priority)) - 1 : 10;
    setEditingRuleId(null);
    setRuleName("");
    setRuleDomains("");
    setRuleKeywords("");
    setRuleCidrs("");
    setRuleProcesses("");
    setRuleSetTag("");
    setRulePriority(String(Math.max(1, minPriority)));
    setRuleTarget("auto:ALL");
    setShowRuleModal(true);
  };

  const openEditModal = (r: SimpleRoutingRule) => {
    setEditingRuleId(r.id);
    setRuleName(presets[r.id] ?? (locale === "en" ? r.titleEn : r.title));
    setRuleDomains(r.patterns.join(", "));
    setRuleKeywords((r.domainKeywords ?? []).join(", "));
    setRuleCidrs((r.ipCidrs ?? []).join(", "));
    setRuleProcesses(r.processNames.join(", "));
    setRuleSetTag(r.ruleSet ?? "");
    setRulePriority(String(r.priority));
    setRuleTarget(r.target);
    setShowRuleModal(true);
  };

  const handleConfirmRule = () => {
    const cleanName = ruleName.trim();
    const splitList = (s: string) =>
      s
        .split(/[,\n]+/)
        .map((x) => x.trim())
        .filter(Boolean);
    const patterns = splitList(ruleDomains).map((s) => s.toLowerCase());
    const keywords = splitList(ruleKeywords).map((s) => s.toLowerCase());
    const cidrs = splitList(ruleCidrs);
    const procs = splitList(ruleProcesses);
    const rs = ruleSetTag.trim() || null;

    if (
      !cleanName ||
      (patterns.length === 0 &&
        keywords.length === 0 &&
        cidrs.length === 0 &&
        procs.length === 0 &&
        !rs)
    ) {
      return;
    }

    const parsedPriority = Number.parseInt(rulePriority, 10);
    const priority = Number.isFinite(parsedPriority) ? parsedPriority : 15;
    const descParts = [
      ...patterns.slice(0, 6),
      ...keywords.slice(0, 2).map((k) => `*${k}*`),
      ...cidrs.slice(0, 2),
      ...procs.slice(0, 2),
    ];
    const description = descParts.length > 0 ? descParts.join(", ") : rs ?? cleanName;

    const existing = editingRuleId ? rules.find((r) => r.id === editingRuleId) : null;
    onSaveRule({
      id: existing ? existing.id : `r-custom-${Date.now()}`,
      title: cleanName,
      titleEn: cleanName,
      description,
      patterns,
      domainKeywords: keywords,
      ipCidrs: cidrs,
      processNames: procs,
      ruleSet: rs,
      target: ruleTarget,
      enabled: existing ? existing.enabled : true,
      priority,
    });
    setShowRuleModal(false);
  };

  const handleAddPreset = (preset: RulePresetSpec) => {
    onSaveRule({
      id: preset.id,
      title: preset.title,
      titleEn: preset.titleEn,
      description: preset.description,
      patterns: preset.patterns,
      domainKeywords: [],
      ipCidrs: [],
      processNames: preset.processNames,
      ruleSet: preset.ruleSet ?? null,
      target: preset.defaultTarget,
      enabled: true,
      priority: preset.defaultPriority,
    });
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

  const filteredPresets = useMemo(() => {
    const q = presetQuery.trim().toLowerCase();
    return rulePresets.filter((p) => {
      if (presetCategory !== "ALL" && p.category !== presetCategory) return false;
      if (!q) return true;
      const localTitle = (presets[p.id] ?? p.title).toLowerCase();
      return (
        localTitle.includes(q) ||
        p.titleEn.toLowerCase().includes(q) ||
        p.description.toLowerCase().includes(q) ||
        (p.ruleSet && p.ruleSet.toLowerCase().includes(q))
      );
    });
  }, [rulePresets, presetCategory, presetQuery, presets]);

  const presetCategories = useMemo(() => {
    const cats = Array.from(new Set(rulePresets.map((p) => p.category)));
    return ["ALL", ...cats];
  }, [rulePresets]);

  const mainNode = nodes.find((n) => n.id === mainNodeId) ?? nodes[0] ?? null;
  const activePortsList = [
    mixedPort,
    ...extraPorts.filter((p) => p.enabled).map((p) => p.port),
  ].join(", ");

  const activeStrategyId = useMemo(() => {
    const builtInIds = rules
      .filter((r) => !r.id.startsWith("r-custom-"))
      .map((r) => r.id)
      .sort()
      .join(",");
    const match = (ids: string[]) => ids.slice().sort().join(",") === builtInIds;
    if (
      match(["classic-ads", "classic-private", "classic-geosite-cn", "classic-geoip-cn"])
    ) {
      return "bypass-cn";
    }
    if (
      match([
        "classic-ads",
        "classic-private",
        "classic-gfw",
        "classic-geolocation-nocn",
        "classic-geosite-cn",
        "classic-geoip-cn",
      ])
    ) {
      return "gfw-list";
    }
    if (
      match([
        "classic-ads",
        "classic-private",
        "classic-ai",
        "classic-netflix",
        "classic-youtube",
        "classic-telegram-site",
        "classic-telegram-ip",
        "classic-geosite-cn",
        "classic-geoip-cn",
      ])
    ) {
      return "bypass-cn-split";
    }
    return null;
  }, [rules]);

  const strategyCards = [
    {
      id: "bypass-cn",
      title: dict.routing.strategyBypassCn,
      desc: dict.routing.strategyBypassCnDesc,
      tags: ["geosite-cn", "geoip-cn", "ads"],
    },
    {
      id: "gfw-list",
      title: dict.routing.strategyGfw,
      desc: dict.routing.strategyGfwDesc,
      tags: ["geosite-gfw", "geosite-!cn", "geoip-cn"],
    },
    {
      id: "bypass-cn-split",
      title: dict.routing.strategySplit,
      desc: dict.routing.strategySplitDesc,
      tags: ["ai-!cn", "netflix", "youtube", "telegram"],
    },
  ];

  return (
    <section className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto w-full max-w-4xl space-y-5 px-6 py-5">
        {/* Header */}
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 className="text-[16px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]">
              {dict.routing.title}
            </h1>
            <p className="mt-0.5 text-[12px] text-[var(--text-secondary)]">
              {dict.routing.subtitle}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-1.5">
            <button
              type="button"
              onClick={() => setShowPresetModal(true)}
              className="ui-btn-secondary h-8"
            >
              <Layers className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
              {dict.routing.presetLibBtn}
            </button>
            <button
              type="button"
              disabled={syncingRuleSets}
              onClick={() => void onSyncRuleSets([])}
              className="ui-btn-secondary h-8"
            >
              <RefreshCw
                className={`h-3.5 w-3.5 text-[var(--text-tertiary)] ${
                  syncingRuleSets ? "animate-spin" : ""
                }`}
                strokeWidth={1.5}
              />
              {syncingRuleSets
                ? dict.routing.syncingRuleSets
                : dict.routing.syncRuleSetsBtn}
            </button>
            <button
              type="button"
              onClick={openAddModal}
              className="ui-btn-primary h-8"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.routing.addRule}
            </button>
          </div>
        </div>

        {/* Dedicated Route & DNS Target Tester Card */}
        <div className="ui-card overflow-hidden">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-4 py-2">
            <span className="text-[11.5px] font-medium text-[var(--text-secondary)]">
              {dict.routing.testTitle}
            </span>
            <div className="flex flex-wrap items-center gap-1">
              {["chatgpt.com", "youtube.com", "github.com", "baidu.com", "192.168.1.1"].map(
                (sample) => (
                  <button
                    key={sample}
                    type="button"
                    onClick={() => {
                      setTestInput(sample);
                      void runRouteTest(sample);
                    }}
                    className="rounded-[6px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2 py-0.5 font-mono text-[10.5px] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                  >
                    {sample}
                  </button>
                )
              )}
            </div>
          </div>

          <div className="space-y-3 p-4">
            <div className="flex flex-wrap items-center gap-2">
              <input
                type="text"
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void runRouteTest();
                }}
                placeholder={dict.routing.simPlaceholder}
                className="ui-input h-8 min-w-[220px] flex-1 px-3 font-mono text-[12px]"
              />
              <input
                type="text"
                value={testProcess}
                onChange={(e) => setTestProcess(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void runRouteTest();
                }}
                placeholder={dict.routing.testProcessPh}
                className="ui-input h-8 w-44 px-3 font-mono text-[12px]"
              />
              <button
                type="button"
                disabled={testingRoute || !testInput.trim()}
                onClick={() => void runRouteTest()}
                className="ui-btn-primary h-8 px-3.5"
              >
                <Play className="h-3.5 w-3.5" strokeWidth={1.5} />
                {dict.routing.testBtn}
              </button>
            </div>

            <div className="grid grid-cols-2 gap-2.5 rounded-[10px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-3 text-[12px] sm:grid-cols-4">
              <div className="min-w-0">
                <div className="text-[10.5px] text-[var(--text-tertiary)]">
                  {dict.routing.testResultRule}
                </div>
                <div className="mt-0.5 truncate font-medium text-[var(--text-primary)]">
                  {displayMatchedRule}
                </div>
              </div>

              <div className="min-w-0">
                <div className="text-[10.5px] text-[var(--text-tertiary)]">
                  {dict.routing.testResultMatcher}
                </div>
                <div
                  className="mt-0.5 truncate font-mono text-[11.5px] text-[var(--text-secondary)]"
                  title={testResult?.matchedBy}
                >
                  {testResult?.matchedBy ?? "--"}
                </div>
              </div>

              <div className="min-w-0">
                <div className="text-[10.5px] text-[var(--text-tertiary)]">
                  {dict.routing.testResultExit}
                </div>
                <div
                  className="mt-0.5 flex items-center gap-1.5 truncate font-semibold"
                  style={{
                    color:
                      displayExitKind === "reject"
                        ? "var(--danger-text)"
                        : displayExitKind === "direct"
                          ? "var(--text-secondary)"
                          : "var(--success-text)",
                  }}
                >
                  {displayExitKind === "node" || displayExitKind === "auto" ? (
                    <NodeLabel
                      remark={displayExitLabel}
                      region={displayExitRegion}
                      size="xs"
                    />
                  ) : (
                    <span className="truncate">{displayExitLabel}</span>
                  )}
                </div>
              </div>

              <div className="min-w-0">
                <div className="text-[10.5px] text-[var(--text-tertiary)]">
                  {dict.routing.testResultDns}
                </div>
                <div
                  className="mt-0.5 truncate font-mono text-[11.5px] text-[var(--text-primary)]"
                  title={testResult?.dnsReason}
                >
                  {displayDns}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Classic Community Site/IP Strategies */}
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-[11.5px] font-medium text-[var(--text-secondary)]">
              {dict.routing.strategyLabel}
            </span>
            {applyingStrategy && (
              <span className="inline-flex items-center gap-1.5 text-[11px] text-[var(--text-secondary)]">
                <RefreshCw className="h-3 w-3 animate-spin" strokeWidth={1.5} />
                {dict.routing.strategyApplying}
              </span>
            )}
          </div>
          <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-3">
            {strategyCards.map((s) => {
              const active = activeStrategyId === s.id;
              return (
                <button
                  key={s.id}
                  type="button"
                  disabled={applyingStrategy}
                  onClick={() => void onApplyStrategy(s.id)}
                  className={`flex flex-col items-start justify-between gap-2 rounded-[12px] border p-3 text-left transition-colors cursor-pointer disabled:opacity-60 ${
                    active
                      ? "border-[var(--accent-primary)] bg-[var(--bg-selected)]"
                      : "border-[var(--border-hairline)] bg-[var(--bg-surface)] hover:border-[var(--border-strong)] hover:bg-[var(--bg-hover)]"
                  }`}
                >
                  <div className="w-full space-y-0.5">
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-[12.5px] font-medium text-[var(--text-primary)]">
                        {s.title}
                      </span>
                      {active && (
                        <Check
                          className="h-3.5 w-3.5 shrink-0"
                          style={{ color: "var(--success-text)" }}
                          strokeWidth={2}
                        />
                      )}
                    </div>
                    <div className="text-[11px] leading-snug text-[var(--text-secondary)]">
                      {s.desc}
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {s.tags.map((tag) => (
                      <span
                        key={tag}
                        className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--text-secondary)]"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Routing Rules List */}
        <div className="ui-card overflow-hidden">
          <div className="grid grid-cols-12 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            <div className="col-span-5">{dict.routing.colWhen}</div>
            <div className="col-span-5">{dict.routing.colThen}</div>
            <div className="col-span-2 text-right">{dict.routing.colState}</div>
          </div>

          <div className="divide-y divide-[var(--border-hairline)]">
            {rules.map((r) => {
              const title = presets[r.id] ?? (locale === "en" ? r.titleEn : r.title);
              const isHit = activeMatchedRuleId === r.id;
              const resolved = resolveTargetExit(
                r.target,
                nodes,
                mainNodeId,
                targetLabels
              );
              const showAutoHint = r.target.startsWith("auto:") && resolved.node !== null;
              const rsStatus = r.ruleSet ? ruleSetMap.get(r.ruleSet) : undefined;
              return (
                <div
                  key={r.id}
                  className={`grid grid-cols-12 items-center gap-3 px-4 py-3 transition-shadow ${
                    r.enabled ? "" : "opacity-50"
                  } ${isHit ? "hit-ring" : ""}`}
                >
                  <div className="col-span-5 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="truncate text-[12.5px] font-medium text-[var(--text-primary)]">
                        {title}
                      </span>
                      {r.ruleSet && (
                        <span
                          className="inline-flex shrink-0 items-center gap-1 rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--text-secondary)]"
                          title={r.ruleSet}
                        >
                          <span
                            className="h-1.5 w-1.5 rounded-full"
                            style={{
                              backgroundColor: rsStatus?.cachedLocally
                                ? "var(--success-dot)"
                                : "var(--text-tertiary)",
                            }}
                          />
                          <span>
                            {rsStatus?.cachedLocally
                              ? `${r.ruleSet} (${formatBytes(rsStatus.bytes)})`
                              : dict.routing.ruleSetFallback}
                          </span>
                        </span>
                      )}
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
                        className="flex items-center gap-1 truncate pl-0.5 font-mono text-[11px] num-tabular"
                        style={{ color: "var(--success-text)" }}
                      >
                        <span className="shrink-0">{dict.routing.fastestNow}</span>
                        <NodeLabel
                          remark={resolved.node.remark}
                          region={resolved.node.region}
                          size="xs"
                        />
                        {resolved.node.realDelayMs != null && (
                          <span className="shrink-0">({resolved.node.realDelayMs} ms)</span>
                        )}
                      </div>
                    )}
                  </div>

                  <div className="col-span-2 flex items-center justify-end gap-1">
                    <button
                      type="button"
                      onClick={() => openEditModal(r)}
                      aria-label={dict.routing.editRuleTitle}
                      className="ui-btn-ghost h-7 w-7 p-0"
                    >
                      <Pencil className="h-3.5 w-3.5" strokeWidth={1.5} />
                    </button>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={r.enabled}
                      onClick={() => onSaveRule({ ...r, enabled: !r.enabled })}
                      className={`relative h-[18px] w-8 shrink-0 rounded-full transition-colors cursor-pointer ${
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
              routeMode === "rule" && activeMatchedRuleId === null && !testResult?.isPrivateIp
                ? "hit-ring"
                : ""
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
                <span className="flex min-w-0 items-center gap-1.5 font-medium text-[var(--text-primary)]">
                  <span className="shrink-0 text-[var(--text-secondary)]">
                    {dict.routing.followMain}
                  </span>
                  {mainNode ? (
                    <NodeLabel remark={mainNode.remark} region={mainNode.region} size="xs" />
                  ) : (
                    <span className="truncate">{dict.nav.noNodeSelected}</span>
                  )}
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

        {/* Collapsible Local Inbound Ports */}
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
                <ChevronDown className="h-4 w-4 text-[var(--text-tertiary)]" strokeWidth={1.5} />
              ) : (
                <ChevronRight className="h-4 w-4 text-[var(--text-tertiary)]" strokeWidth={1.5} />
              )}
            </div>
          </button>

          {portsOpen && (
            <div className="divide-y divide-[var(--border-hairline)] border-t border-[var(--border-hairline)]">
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
                  <Copy className="h-3 w-3 text-[var(--text-tertiary)]" strokeWidth={1.5} />
                  {dict.routing.copyUrl}
                </button>
              </div>

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

      {/* Geosite Preset Library Modal */}
      {showPresetModal && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ backgroundColor: "var(--backdrop-scrim)" }}
        >
          <div className="ui-modal flex max-h-[82vh] w-full max-w-2xl flex-col overflow-hidden">
            <div className="flex items-center justify-between border-b border-[var(--border-hairline)] px-5 py-3.5">
              <div>
                <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                  {dict.routing.presetLibTitle}
                </h3>
                <p className="mt-0.5 text-[11.5px] text-[var(--text-secondary)]">
                  {dict.routing.presetLibDesc}
                </p>
              </div>
              <button
                type="button"
                onClick={() => setShowPresetModal(false)}
                className="ui-btn-ghost h-7 w-7 p-0"
              >
                <X className="h-4 w-4" strokeWidth={1.5} />
              </button>
            </div>

            <div className="space-y-2.5 border-b border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-5 py-3">
              <div className="relative">
                <Search
                  className="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-tertiary)]"
                  strokeWidth={1.5}
                />
                <input
                  type="text"
                  value={presetQuery}
                  onChange={(e) => setPresetQuery(e.target.value)}
                  placeholder={dict.routing.presetSearchPh}
                  className="ui-input h-8 w-full bg-[var(--bg-surface)] pr-3 pl-8 text-[12px]"
                />
              </div>
              <div className="flex flex-wrap gap-1">
                {presetCategories.map((cat) => {
                  const active = presetCategory === cat;
                  const label =
                    cat === "ALL" ? dict.routing.presetAll : (presetCatLabels[cat] ?? cat);
                  return (
                    <button
                      key={cat}
                      type="button"
                      onClick={() => setPresetCategory(cat)}
                      className={`rounded-[7px] px-2.5 py-1 text-[11.5px] font-medium transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                          : "border border-[var(--border-hairline)] bg-[var(--bg-surface)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                      }`}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-auto p-4">
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                {filteredPresets.map((p) => {
                  const added = existingRuleIds.has(p.id);
                  const title = presets[p.id] ?? (locale === "en" ? p.titleEn : p.title);
                  const cached = p.ruleSet ? ruleSetMap.get(p.ruleSet)?.cachedLocally : false;
                  return (
                    <div
                      key={p.id}
                      className="ui-card flex items-start justify-between gap-2.5 p-3"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-[12.5px] font-medium text-[var(--text-primary)]">
                            {title}
                          </span>
                        </div>
                        <div className="mt-0.5 truncate text-[11px] text-[var(--text-secondary)]">
                          {p.description}
                        </div>
                        {p.ruleSet && (
                          <div className="mt-1.5 inline-flex items-center gap-1 rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--text-secondary)]">
                            <Database className="h-2.5 w-2.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
                            <span>{p.ruleSet}</span>
                            {cached && (
                              <span
                                className="h-1.5 w-1.5 rounded-full"
                                style={{ backgroundColor: "var(--success-dot)" }}
                              />
                            )}
                          </div>
                        )}
                      </div>
                      <button
                        type="button"
                        disabled={added}
                        onClick={() => handleAddPreset(p)}
                        className={added ? "ui-btn-secondary h-7 px-2.5 text-[11px]" : "ui-btn-primary h-7 px-2.5 text-[11px]"}
                      >
                        {added ? dict.routing.presetAdded : dict.routing.presetAdd}
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Add / Edit Rule Modal */}
      {showRuleModal && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4"
          style={{ backgroundColor: "var(--backdrop-scrim)" }}
        >
          <div className="ui-modal w-full max-w-lg overflow-hidden">
            <div className="flex items-center justify-between border-b border-[var(--border-hairline)] px-5 py-3.5">
              <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                {editingRuleId ? dict.routing.editRuleTitle : dict.routing.addRuleTitle}
              </h3>
              <button
                type="button"
                onClick={() => setShowRuleModal(false)}
                className="ui-btn-ghost h-7 w-7 p-0"
              >
                <X className="h-4 w-4" strokeWidth={1.5} />
              </button>
            </div>
            <div className="max-h-[75vh] space-y-3 overflow-auto p-5 text-[12px]">
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                <div className="space-y-1 sm:col-span-2">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.ruleNameLabel}
                  </label>
                  <input
                    type="text"
                    value={ruleName}
                    onChange={(e) => setRuleName(e.target.value)}
                    placeholder={dict.routing.ruleNamePh}
                    className="ui-input h-8 w-full px-3"
                  />
                </div>
                <div className="space-y-1">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.rulePriorityLabel}
                  </label>
                  <input
                    type="text"
                    inputMode="numeric"
                    value={rulePriority}
                    onChange={(e) =>
                      setRulePriority(e.target.value.replace(/[^0-9-]/g, "").slice(0, 5))
                    }
                    className="ui-input h-8 w-full px-3 font-mono num-tabular"
                  />
                </div>
              </div>

              <div className="space-y-1">
                <label className="block font-medium text-[var(--text-primary)]">
                  {dict.routing.ruleDomainsLabel}
                </label>
                <textarea
                  rows={2}
                  value={ruleDomains}
                  onChange={(e) => setRuleDomains(e.target.value)}
                  placeholder={dict.routing.ruleDomainsPh}
                  className="ui-input w-full p-2.5 font-mono text-[11.5px]"
                />
              </div>

              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <div className="space-y-1">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.ruleKeywordsLabel}
                  </label>
                  <input
                    type="text"
                    value={ruleKeywords}
                    onChange={(e) => setRuleKeywords(e.target.value)}
                    placeholder={dict.routing.ruleKeywordsPh}
                    className="ui-input h-8 w-full px-3 font-mono text-[11.5px]"
                  />
                </div>
                <div className="space-y-1">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.ruleCidrsLabel}
                  </label>
                  <input
                    type="text"
                    value={ruleCidrs}
                    onChange={(e) => setRuleCidrs(e.target.value)}
                    placeholder={dict.routing.ruleCidrsPh}
                    className="ui-input h-8 w-full px-3 font-mono text-[11.5px]"
                  />
                </div>
              </div>

              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <div className="space-y-1">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.ruleProcessLabel}
                  </label>
                  <input
                    type="text"
                    value={ruleProcesses}
                    onChange={(e) => setRuleProcesses(e.target.value)}
                    placeholder={dict.routing.ruleProcessPh}
                    className="ui-input h-8 w-full px-3 font-mono text-[11.5px]"
                  />
                </div>
                <div className="space-y-1">
                  <label className="block font-medium text-[var(--text-primary)]">
                    {dict.routing.ruleSetLabel}
                  </label>
                  <input
                    type="text"
                    value={ruleSetTag}
                    onChange={(e) => setRuleSetTag(e.target.value)}
                    placeholder={dict.routing.ruleSetPh}
                    className="ui-input h-8 w-full px-3 font-mono text-[11.5px]"
                  />
                </div>
              </div>

              <div className="space-y-1">
                <label className="block font-medium text-[var(--text-primary)]">
                  {dict.routing.ruleExitLabel}
                </label>
                <CustomSelect
                  value={ruleTarget}
                  groups={buildSelectGroups(ruleTarget, true)}
                  onChange={setRuleTarget}
                  searchPlaceholder={dict.routing.selectSearchPh}
                  emptyLabel={dict.routing.selectEmpty}
                  className="w-full"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setShowRuleModal(false)}
                  className="ui-btn-secondary"
                >
                  {dict.nodes.cancel}
                </button>
                <button
                  type="button"
                  onClick={handleConfirmRule}
                  className="ui-btn-primary"
                >
                  {editingRuleId ? dict.routing.btnSave : dict.routing.btnAdd}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
