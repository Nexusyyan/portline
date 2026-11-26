import { useEffect, useMemo, useState } from "react";
import { ArrowRight, Check, Plus, Trash2 } from "lucide-react";
import { CustomSelect, type SelectGroup } from "../../components/CustomSelect";
import { RegionFlag, parseFlagText } from "../../components/RegionFlag";
import type { Dict } from "../../i18n";
import type { AppSettings, DnsHostEntry, DnsRuleItem, NodeIR } from "../../types";
import { computeAvailableRegions } from "../../utils/nodes";

interface DnsViewProps {
  dict: Dict;
  settings: AppSettings;
  nodes: NodeIR[];
  onUpdateSettings: (next: AppSettings) => void;
}

const LOCAL_DNS_PRESETS = [
  { label: "AliDNS UDP", value: "223.5.5.5" },
  { label: "DNSPod UDP", value: "119.29.29.29" },
  { label: "AliDNS DoH", value: "https://dns.alidns.com/dns-query" },
  { label: "DNSPod DoH", value: "https://doh.pub/dns-query" },
  { label: "AliDNS DoT", value: "tls://dns.alidns.com" },
];

const REMOTE_DNS_PRESETS = [
  { label: "Cloudflare DoH", value: "https://1.1.1.1/dns-query" },
  { label: "Google DoH", value: "https://8.8.8.8/dns-query" },
  { label: "Cloudflare DoT", value: "tls://1.1.1.1" },
  { label: "Google DoT", value: "tls://8.8.8.8" },
  { label: "Quad9 DoH", value: "https://9.9.9.9/dns-query" },
];

const BOOTSTRAP_PRESETS = [
  { label: "AliDNS", value: "223.5.5.5" },
  { label: "DNSPod", value: "119.29.29.29" },
  { label: "114DNS", value: "114.114.114.114" },
];

function detectDnsProtocol(spec: string): string {
  const s = spec.trim().toLowerCase();
  if (s.startsWith("https://")) return "DoH";
  if (s.startsWith("h3://")) return "H3";
  if (s.startsWith("tls://")) return "DoT";
  if (s.startsWith("quic://")) return "DoQ";
  if (s.startsWith("tcp://")) return "TCP";
  return "UDP";
}

export function DnsView({ dict, settings, nodes, onUpdateSettings }: DnsViewProps) {
  const [savedTick, setSavedTick] = useState(false);

  const [localDraft, setLocalDraft] = useState(settings.dnsLocalServer);
  const [remoteDraft, setRemoteDraft] = useState(settings.dnsRemoteServer);
  const [bootstrapDraft, setBootstrapDraft] = useState(settings.dnsBootstrapServer);
  const [fake4Draft, setFake4Draft] = useState(settings.dnsFakeIpInet4);
  const [fake6Draft, setFake6Draft] = useState(settings.dnsFakeIpInet6);
  const [cacheCapDraft, setCacheCapDraft] = useState(String(settings.dnsCacheCapacity));
  const [ecsDraft, setEcsDraft] = useState(settings.dnsClientSubnet ?? "");

  const [hostDomain, setHostDomain] = useState("");
  const [hostIps, setHostIps] = useState("");

  const [ruleTitle, setRuleTitle] = useState("");
  const [ruleDomains, setRuleDomains] = useState("");
  const [ruleSetTag, setRuleSetTag] = useState("");
  const [ruleServer, setRuleServer] = useState("dns-local");

  useEffect(() => setLocalDraft(settings.dnsLocalServer), [settings.dnsLocalServer]);
  useEffect(() => setRemoteDraft(settings.dnsRemoteServer), [settings.dnsRemoteServer]);
  useEffect(() => setBootstrapDraft(settings.dnsBootstrapServer), [settings.dnsBootstrapServer]);
  useEffect(() => setFake4Draft(settings.dnsFakeIpInet4), [settings.dnsFakeIpInet4]);
  useEffect(() => setFake6Draft(settings.dnsFakeIpInet6), [settings.dnsFakeIpInet6]);
  useEffect(() => setCacheCapDraft(String(settings.dnsCacheCapacity)), [settings.dnsCacheCapacity]);
  useEffect(() => setEcsDraft(settings.dnsClientSubnet ?? ""), [settings.dnsClientSubnet]);

  const commit = (patch: Partial<AppSettings>) => {
    onUpdateSettings({ ...settings, ...patch });
    setSavedTick(true);
    window.setTimeout(() => setSavedTick(false), 1600);
  };

  const regionNames = dict.routing.regions as Record<string, string>;
  const availableRegions = useMemo(() => computeAvailableRegions(nodes), [nodes]);

  const strategyGroups: SelectGroup[] = useMemo(
    () => [
      {
        label: dict.dns.strategyLabel,
        options: [
          { value: "prefer_ipv4", label: dict.dns.strategyPrefer4 },
          { value: "ipv4_only", label: dict.dns.strategyOnly4 },
          { value: "prefer_ipv6", label: dict.dns.strategyPrefer6 },
          { value: "ipv6_only", label: dict.dns.strategyOnly6 },
        ],
      },
    ],
    [dict]
  );

  const detourGroups: SelectGroup[] = useMemo(() => {
    const groups: SelectGroup[] = [
      {
        label: dict.routing.optActionsGroup,
        options: [
          { value: "main", label: dict.routing.optMain },
          { value: "direct", label: dict.routing.optDirect },
        ],
      },
    ];
    if (availableRegions.length > 0 || nodes.length > 0) {
      const autoOpts = availableRegions.map((b) => ({
        value: `auto:${b.code}`,
        label: `${dict.routing.optAutoRegionPrefix} ${regionNames[b.code] ?? b.code}`,
        meta: String(b.count),
        icon: <RegionFlag code={b.code} size="xs" />,
      }));
      autoOpts.push({
        value: "auto:ALL",
        label: dict.routing.optAutoAll,
        meta: String(nodes.length),
        icon: <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-tertiary)]" />,
      });
      groups.push({ label: dict.routing.optAutoGroup, options: autoOpts });
    }
    if (nodes.length > 0) {
      groups.push({
        label: dict.routing.optNodesGroup,
        options: nodes.map((n) => {
          const parsed = parseFlagText(n.remark, n.region);
          return {
            value: n.id,
            label: parsed.cleanText,
            icon: parsed.isoCode ? <RegionFlag code={parsed.isoCode} size="xs" /> : undefined,
          };
        }),
      });
    }
    return groups;
  }, [availableRegions, nodes, dict, regionNames]);

  const dnsRuleServerGroups: SelectGroup[] = useMemo(() => {
    const opts = [
      { value: "dns-local", label: dict.dns.ruleServerLocal },
      { value: "dns-remote", label: dict.dns.ruleServerRemote },
      { value: "reject", label: dict.dns.ruleServerReject },
    ];
    if (settings.dnsFakeIp) {
      opts.splice(2, 0, { value: "dns-fakeip", label: dict.dns.ruleServerFakeip });
    }
    return [{ label: dict.dns.secRules, options: opts }];
  }, [dict, settings.dnsFakeIp]);

  const handleAddHost = () => {
    const dom = hostDomain.trim().toLowerCase();
    const ips = hostIps
      .split(/[,\s]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (!dom || ips.length === 0) return;
    const next: DnsHostEntry[] = [
      ...settings.dnsHosts.filter((h) => h.domain.toLowerCase() !== dom),
      { domain: dom, ips },
    ];
    commit({ dnsHosts: next });
    setHostDomain("");
    setHostIps("");
  };

  const handleRemoveHost = (domain: string) => {
    commit({ dnsHosts: settings.dnsHosts.filter((h) => h.domain !== domain) });
  };

  const handleAddDnsRule = () => {
    const doms = ruleDomains
      .split(/[,\s]+/)
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean);
    const rs = ruleSetTag.trim() || null;
    if (doms.length === 0 && !rs) return;
    const item: DnsRuleItem = {
      id: `dr-${Date.now()}`,
      title: ruleTitle.trim() || rs || doms[0],
      domains: doms,
      ruleSet: rs,
      server: ruleServer,
      enabled: true,
    };
    commit({ dnsCustomRules: [...settings.dnsCustomRules, item] });
    setRuleTitle("");
    setRuleDomains("");
    setRuleSetTag("");
  };

  const handleToggleDnsRule = (id: string) => {
    commit({
      dnsCustomRules: settings.dnsCustomRules.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      ),
    });
  };

  const handleDeleteDnsRule = (id: string) => {
    commit({
      dnsCustomRules: settings.dnsCustomRules.filter((r) => r.id !== id),
    });
  };

  const serverLabel = (srv: string) => {
    if (srv === "dns-local") return dict.dns.ruleServerLocal;
    if (srv === "dns-fakeip") return dict.dns.ruleServerFakeip;
    if (srv === "reject") return dict.dns.ruleServerReject;
    return dict.dns.ruleServerRemote;
  };

  const flowSteps = [
    ...(settings.dnsHosts.length > 0
      ? [{ label: dict.dns.flowHosts, meta: `${settings.dnsHosts.length}` }]
      : []),
    ...(settings.dnsCustomRules.filter((r) => r.enabled).length > 0
      ? [
          {
            label: dict.dns.flowCustom,
            meta: `${settings.dnsCustomRules.filter((r) => r.enabled).length}`,
          },
        ]
      : []),
    {
      label: dict.dns.flowDirect,
      meta: detectDnsProtocol(settings.dnsLocalServer),
    },
    ...(settings.dnsFakeIp
      ? [{ label: dict.dns.flowFakeIp, meta: settings.dnsFakeIpInet4 }]
      : []),
    {
      label: dict.dns.flowFinal,
      meta: detectDnsProtocol(settings.dnsRemoteServer),
    },
  ];

  return (
    <section className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto w-full max-w-4xl space-y-5 px-6 py-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 className="text-[16px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]">
              {dict.dns.title}
            </h1>
            <p className="mt-0.5 text-[12px] text-[var(--text-secondary)]">
              {dict.dns.subtitle}
            </p>
          </div>
          <span
            className={`inline-flex items-center gap-1 text-[11.5px] transition-opacity ${
              savedTick ? "opacity-100" : "opacity-60"
            }`}
            style={{ color: "var(--success-text)" }}
          >
            <Check className="h-3.5 w-3.5" strokeWidth={1.6} />
            {dict.settings.autoSaved}
          </span>
        </div>

        {/* Active Resolution Pipeline Preview */}
        <div className="rounded-[12px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-4 py-3">
          <div className="mb-2 text-[11px] font-medium text-[var(--text-secondary)]">
            {dict.dns.secFlow}
          </div>
          <div className="flex flex-wrap items-center gap-1.5 text-[11.5px]">
            {flowSteps.map((step, i) => (
              <div key={`${step.label}-${i}`} className="flex items-center gap-1.5">
                {i > 0 && (
                  <ArrowRight
                    className="h-3 w-3 shrink-0 text-[var(--text-tertiary)]"
                    strokeWidth={1.5}
                  />
                )}
                <span className="inline-flex items-center gap-1.5 rounded-[7px] border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2.5 py-1 text-[var(--text-primary)]">
                  <span className="font-medium">{step.label}</span>
                  <span className="font-mono text-[10.5px] text-[var(--text-tertiary)] num-tabular">
                    {step.meta}
                  </span>
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* Upstream DNS Servers */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.dns.secServers}
          </div>

          <div className="space-y-2 px-4 py-3">
            <div className="flex items-center justify-between gap-2">
              <div>
                <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                  {dict.dns.localLabel}
                </div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  {dict.dns.localDesc}
                </div>
              </div>
              <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2 py-0.5 font-mono text-[10.5px] text-[var(--text-secondary)]">
                {detectDnsProtocol(localDraft)}
              </span>
            </div>
            <input
              type="text"
              value={localDraft}
              onChange={(e) => setLocalDraft(e.target.value)}
              onBlur={() => {
                const v = localDraft.trim();
                if (v && v !== settings.dnsLocalServer) commit({ dnsLocalServer: v });
                else if (!v) setLocalDraft(settings.dnsLocalServer);
              }}
              className="ui-input h-8 w-full px-3 font-mono text-[12px]"
            />
            <div className="flex flex-wrap gap-1.5">
              {LOCAL_DNS_PRESETS.map((p) => (
                <button
                  key={p.value}
                  type="button"
                  onClick={() => {
                    setLocalDraft(p.value);
                    commit({ dnsLocalServer: p.value });
                  }}
                  className={`rounded-[6px] border px-2 py-0.5 font-mono text-[11px] transition-colors cursor-pointer ${
                    localDraft === p.value
                      ? "border-[var(--border-strong)] bg-[var(--bg-selected)] text-[var(--text-primary)]"
                      : "border-[var(--border-hairline)] bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-2 px-4 py-3">
            <div className="flex items-center justify-between gap-2">
              <div>
                <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                  {dict.dns.remoteLabel}
                </div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  {dict.dns.remoteDesc}
                </div>
              </div>
              <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2 py-0.5 font-mono text-[10.5px] text-[var(--text-secondary)]">
                {detectDnsProtocol(remoteDraft)}
              </span>
            </div>
            <input
              type="text"
              value={remoteDraft}
              onChange={(e) => setRemoteDraft(e.target.value)}
              onBlur={() => {
                const v = remoteDraft.trim();
                if (v && v !== settings.dnsRemoteServer) commit({ dnsRemoteServer: v });
                else if (!v) setRemoteDraft(settings.dnsRemoteServer);
              }}
              className="ui-input h-8 w-full px-3 font-mono text-[12px]"
            />
            <div className="flex flex-wrap gap-1.5">
              {REMOTE_DNS_PRESETS.map((p) => (
                <button
                  key={p.value}
                  type="button"
                  onClick={() => {
                    setRemoteDraft(p.value);
                    commit({ dnsRemoteServer: p.value });
                  }}
                  className={`rounded-[6px] border px-2 py-0.5 font-mono text-[11px] transition-colors cursor-pointer ${
                    remoteDraft === p.value
                      ? "border-[var(--border-strong)] bg-[var(--bg-selected)] text-[var(--text-primary)]"
                      : "border-[var(--border-hairline)] bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.dns.remoteDetourLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.dns.remoteDetourDesc}
              </div>
            </div>
            <CustomSelect
              value={settings.dnsRemoteDetour}
              groups={detourGroups}
              onChange={(val) => commit({ dnsRemoteDetour: val })}
              searchPlaceholder={dict.routing.selectSearchPh}
              emptyLabel={dict.routing.selectEmpty}
              className="w-56"
            />
          </div>

          <div className="space-y-2 px-4 py-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                  {dict.dns.bootstrapLabel}
                </div>
                <div className="text-[11.5px] text-[var(--text-secondary)]">
                  {dict.dns.bootstrapDesc}
                </div>
              </div>
              <div className="flex items-center gap-1.5">
                {BOOTSTRAP_PRESETS.map((p) => (
                  <button
                    key={p.value}
                    type="button"
                    onClick={() => {
                      setBootstrapDraft(p.value);
                      commit({ dnsBootstrapServer: p.value });
                    }}
                    className={`rounded-[6px] border px-2 py-0.5 font-mono text-[11px] cursor-pointer ${
                      bootstrapDraft === p.value
                        ? "border-[var(--border-strong)] bg-[var(--bg-selected)] text-[var(--text-primary)]"
                        : "border-[var(--border-hairline)] bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                    }`}
                  >
                    {p.label}
                  </button>
                ))}
                <input
                  type="text"
                  value={bootstrapDraft}
                  onChange={(e) => setBootstrapDraft(e.target.value)}
                  onBlur={() => {
                    const v = bootstrapDraft.trim();
                    if (v && v !== settings.dnsBootstrapServer) {
                      commit({ dnsBootstrapServer: v });
                    } else if (!v) {
                      setBootstrapDraft(settings.dnsBootstrapServer);
                    }
                  }}
                  className="ui-input h-7 w-36 px-2.5 font-mono text-[12px]"
                />
              </div>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.dns.strategyLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.dns.strategyDesc}
              </div>
            </div>
            <CustomSelect
              value={settings.dnsStrategy}
              groups={strategyGroups}
              onChange={(val) => commit({ dnsStrategy: val })}
              className="w-44"
            />
          </div>
        </div>

        {/* FakeIP Configuration */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.dns.secFakeIp}
          </div>
          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.dns.fakeIpLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.dns.fakeIpDesc}
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.dnsFakeIp}
              onClick={() => commit({ dnsFakeIp: !settings.dnsFakeIp })}
              className={`relative h-[18px] w-8 shrink-0 rounded-full transition-colors cursor-pointer ${
                settings.dnsFakeIp
                  ? "bg-[var(--accent-primary)]"
                  : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
              }`}
            >
              <span
                className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                style={{ left: settings.dnsFakeIp ? "16px" : "2px" }}
              />
            </button>
          </div>

          {settings.dnsFakeIp && (
            <div className="grid grid-cols-1 gap-4 px-4 py-3 sm:grid-cols-2">
              <div className="space-y-1">
                <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                  {dict.dns.fakeIp4Label}
                </label>
                <input
                  type="text"
                  value={fake4Draft}
                  onChange={(e) => setFake4Draft(e.target.value)}
                  onBlur={() => {
                    const v = fake4Draft.trim();
                    if (v && v !== settings.dnsFakeIpInet4) commit({ dnsFakeIpInet4: v });
                    else if (!v) setFake4Draft(settings.dnsFakeIpInet4);
                  }}
                  className="ui-input h-8 w-full px-3 font-mono text-[12px]"
                />
              </div>
              <div className="space-y-1">
                <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                  {dict.dns.fakeIp6Label}
                </label>
                <input
                  type="text"
                  value={fake6Draft}
                  onChange={(e) => setFake6Draft(e.target.value)}
                  onBlur={() => {
                    const v = fake6Draft.trim();
                    if (v && v !== settings.dnsFakeIpInet6) commit({ dnsFakeIpInet6: v });
                    else if (!v) setFake6Draft(settings.dnsFakeIpInet6);
                  }}
                  className="ui-input h-8 w-full px-3 font-mono text-[12px]"
                />
              </div>
            </div>
          )}
        </div>

        {/* Cache & Resolver Options */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.dns.secBehavior}
          </div>

          {(
            [
              ["dnsDisableCache", dict.dns.disableCacheLabel, dict.dns.disableCacheDesc],
              ["dnsDisableExpire", dict.dns.disableExpireLabel, dict.dns.disableExpireDesc],
              ["dnsReverseMapping", dict.dns.reverseMapLabel, dict.dns.reverseMapDesc],
            ] as const
          ).map(([field, label, desc]) => {
            const checked = settings[field];
            return (
              <div key={field} className="flex items-center justify-between gap-4 px-4 py-3">
                <div>
                  <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                    {label}
                  </div>
                  <div className="text-[11.5px] text-[var(--text-secondary)]">{desc}</div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={checked}
                  onClick={() => commit({ [field]: !checked })}
                  className={`relative h-[18px] w-8 shrink-0 rounded-full transition-colors cursor-pointer ${
                    checked
                      ? "bg-[var(--accent-primary)]"
                      : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
                  }`}
                >
                  <span
                    className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                    style={{ left: checked ? "16px" : "2px" }}
                  />
                </button>
              </div>
            );
          })}

          <div className="grid grid-cols-1 gap-4 px-4 py-3 sm:grid-cols-2">
            <div className="space-y-1">
              <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                {dict.dns.cacheCapLabel}
              </label>
              <input
                type="text"
                inputMode="numeric"
                value={cacheCapDraft}
                onChange={(e) =>
                  setCacheCapDraft(e.target.value.replace(/[^0-9]/g, "").slice(0, 6))
                }
                onBlur={() => {
                  const n = Number.parseInt(cacheCapDraft, 10);
                  if (Number.isFinite(n) && n >= 64 && n <= 65536) {
                    if (n !== settings.dnsCacheCapacity) commit({ dnsCacheCapacity: n });
                  } else {
                    setCacheCapDraft(String(settings.dnsCacheCapacity));
                  }
                }}
                className="ui-input h-8 w-full px-3 font-mono text-[12px] num-tabular"
              />
              <div className="text-[11px] text-[var(--text-secondary)]">
                {dict.dns.cacheCapDesc}
              </div>
            </div>

            <div className="space-y-1">
              <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                {dict.dns.clientSubnetLabel}
              </label>
              <input
                type="text"
                value={ecsDraft}
                onChange={(e) => setEcsDraft(e.target.value)}
                onBlur={() => {
                  const clean = ecsDraft.trim() || null;
                  if (clean !== (settings.dnsClientSubnet ?? null)) {
                    commit({ dnsClientSubnet: clean });
                  }
                }}
                placeholder={dict.dns.clientSubnetPh}
                className="ui-input h-8 w-full px-3 font-mono text-[12px]"
              />
              <div className="text-[11px] text-[var(--text-secondary)]">
                {dict.dns.clientSubnetDesc}
              </div>
            </div>
          </div>
        </div>

        {/* Static Hosts */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="flex items-center justify-between bg-[var(--bg-subtle)] px-4 py-2">
            <span className="text-[11.5px] font-medium text-[var(--text-secondary)]">
              {dict.dns.secHosts}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {dict.dns.hostsDesc}
            </span>
          </div>

          {settings.dnsHosts.length === 0 ? (
            <div className="px-4 py-3 text-[12px] text-[var(--text-tertiary)]">
              {dict.dns.hostsEmpty}
            </div>
          ) : (
            settings.dnsHosts.map((h) => (
              <div
                key={h.domain}
                className="flex items-center justify-between gap-3 px-4 py-2.5 text-[12px]"
              >
                <div className="flex min-w-0 items-center gap-3 font-mono">
                  <span className="truncate font-medium text-[var(--text-primary)]">
                    {h.domain}
                  </span>
                  <ArrowRight className="h-3 w-3 shrink-0 text-[var(--text-tertiary)]" strokeWidth={1.5} />
                  <span className="truncate text-[var(--text-secondary)]">
                    {h.ips.join(", ")}
                  </span>
                </div>
                <button
                  type="button"
                  onClick={() => handleRemoveHost(h.domain)}
                  className="ui-btn-danger-ghost h-7 w-7 p-0"
                >
                  <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                </button>
              </div>
            ))
          )}

          <div className="flex flex-wrap items-center gap-2 bg-[var(--bg-subtle)] px-4 py-2.5">
            <input
              type="text"
              value={hostDomain}
              onChange={(e) => setHostDomain(e.target.value)}
              placeholder={dict.dns.hostDomainPh}
              className="ui-input h-7 min-w-[170px] flex-1 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px]"
            />
            <input
              type="text"
              value={hostIps}
              onChange={(e) => setHostIps(e.target.value)}
              placeholder={dict.dns.hostIpsPh}
              className="ui-input h-7 min-w-[170px] flex-1 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px]"
            />
            <button
              type="button"
              onClick={handleAddHost}
              className="ui-btn-primary h-7 px-3"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.routing.btnAdd}
            </button>
          </div>
        </div>

        {/* Custom DNS Rules */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="flex items-center justify-between bg-[var(--bg-subtle)] px-4 py-2">
            <span className="text-[11.5px] font-medium text-[var(--text-secondary)]">
              {dict.dns.secRules}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {dict.dns.rulesDesc}
            </span>
          </div>

          {settings.dnsCustomRules.length === 0 ? (
            <div className="px-4 py-3 text-[12px] text-[var(--text-tertiary)]">
              {dict.dns.rulesEmpty}
            </div>
          ) : (
            settings.dnsCustomRules.map((r) => (
              <div
                key={r.id}
                className={`flex items-center justify-between gap-3 px-4 py-2.5 text-[12px] ${
                  r.enabled ? "" : "opacity-50"
                }`}
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium text-[var(--text-primary)]">
                      {r.title}
                    </span>
                    <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 text-[10.5px] text-[var(--text-secondary)]">
                      {serverLabel(r.server)}
                    </span>
                  </div>
                  <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--text-secondary)]">
                    {[r.ruleSet ? `rule_set:${r.ruleSet}` : "", ...r.domains]
                      .filter(Boolean)
                      .join(", ")}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    role="switch"
                    aria-checked={r.enabled}
                    onClick={() => handleToggleDnsRule(r.id)}
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
                    onClick={() => handleDeleteDnsRule(r.id)}
                    className="ui-btn-danger-ghost h-7 w-7 p-0"
                  >
                    <Trash2 className="h-3.5 w-3.5" strokeWidth={1.5} />
                  </button>
                </div>
              </div>
            ))
          )}

          <div className="flex flex-wrap items-center gap-2 bg-[var(--bg-subtle)] px-4 py-2.5">
            <input
              type="text"
              value={ruleTitle}
              onChange={(e) => setRuleTitle(e.target.value)}
              placeholder={dict.dns.ruleTitlePh}
              className="ui-input h-7 w-32 bg-[var(--bg-surface)] px-2.5 text-[12px]"
            />
            <input
              type="text"
              value={ruleDomains}
              onChange={(e) => setRuleDomains(e.target.value)}
              placeholder={dict.dns.ruleDomainsPh}
              className="ui-input h-7 min-w-[160px] flex-1 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px]"
            />
            <input
              type="text"
              value={ruleSetTag}
              onChange={(e) => setRuleSetTag(e.target.value)}
              placeholder={dict.dns.ruleSetPh}
              className="ui-input h-7 w-40 bg-[var(--bg-surface)] px-2.5 font-mono text-[12px]"
            />
            <CustomSelect
              value={ruleServer}
              groups={dnsRuleServerGroups}
              onChange={setRuleServer}
              size="sm"
              className="w-32 bg-[var(--bg-surface)]"
            />
            <button
              type="button"
              onClick={handleAddDnsRule}
              className="ui-btn-primary h-7 px-3"
            >
              <Plus className="h-3.5 w-3.5" strokeWidth={1.5} />
              {dict.routing.btnAdd}
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
