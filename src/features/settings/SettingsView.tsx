import { useEffect, useMemo, useState } from "react";
import { Check, HardDrive, ShieldCheck } from "lucide-react";
import { BrandLogo } from "../../components/BrandLogo";
import { CustomSelect, type SelectGroup } from "../../components/CustomSelect";
import type { Dict } from "../../i18n";
import type { AppSettings, CoreInfo, Locale, ThemeMode } from "../../types";

interface SettingsViewProps {
  dict: Dict;
  locale: Locale;
  theme: ThemeMode;
  settings: AppSettings;
  coreInfo: CoreInfo;
  onChangeLocale: (locale: Locale) => void;
  onChangeTheme: (theme: ThemeMode) => void;
  onUpdateSettings: (next: AppSettings) => void;
  onRestoreNetguard: () => void;
}

const LOCAL_DNS_PRESETS = [
  { label: "AliDNS UDP", value: "223.5.5.5" },
  { label: "DNSPod UDP", value: "119.29.29.29" },
  { label: "AliDNS DoH", value: "https://dns.alidns.com/dns-query" },
];

const REMOTE_DNS_PRESETS = [
  { label: "Cloudflare DoH", value: "https://1.1.1.1/dns-query" },
  { label: "Google DoH", value: "https://8.8.8.8/dns-query" },
  { label: "Cloudflare DoT", value: "tls://1.1.1.1" },
];

export function SettingsView({
  dict,
  locale,
  theme,
  settings,
  coreInfo,
  onChangeLocale,
  onChangeTheme,
  onUpdateSettings,
  onRestoreNetguard,
}: SettingsViewProps) {
  const [savedTick, setSavedTick] = useState(false);

  // Free string drafts prevent fighting user selection/deletion on port & text fields.
  const [mixedPortDraft, setMixedPortDraft] = useState(String(settings.mixedPort));
  const [controlPortDraft, setControlPortDraft] = useState(String(settings.controlPort));
  const [testUrlDraft, setTestUrlDraft] = useState(settings.latencyTestUrl);
  const [dnsLocalDraft, setDnsLocalDraft] = useState(settings.dnsLocalServer);
  const [dnsRemoteDraft, setDnsRemoteDraft] = useState(settings.dnsRemoteServer);
  const [overrideDraft, setOverrideDraft] = useState(
    settings.customJsonOverride ?? ""
  );

  useEffect(() => {
    setMixedPortDraft(String(settings.mixedPort));
  }, [settings.mixedPort]);

  useEffect(() => {
    setControlPortDraft(String(settings.controlPort));
  }, [settings.controlPort]);

  useEffect(() => {
    setTestUrlDraft(settings.latencyTestUrl);
  }, [settings.latencyTestUrl]);

  useEffect(() => {
    setDnsLocalDraft(settings.dnsLocalServer);
  }, [settings.dnsLocalServer]);

  useEffect(() => {
    setDnsRemoteDraft(settings.dnsRemoteServer);
  }, [settings.dnsRemoteServer]);

  useEffect(() => {
    setOverrideDraft(settings.customJsonOverride ?? "");
  }, [settings.customJsonOverride]);

  const commit = (patch: Partial<AppSettings>) => {
    onUpdateSettings({ ...settings, ...patch });
    setSavedTick(true);
    window.setTimeout(() => setSavedTick(false), 1600);
  };

  const mixedPortNum = Number.parseInt(mixedPortDraft, 10);
  const mixedPortValid =
    mixedPortDraft.length > 0 &&
    Number.isFinite(mixedPortNum) &&
    mixedPortNum >= 1024 &&
    mixedPortNum <= 65535;

  const controlPortNum = Number.parseInt(controlPortDraft, 10);
  const controlPortValid =
    controlPortDraft.length > 0 &&
    Number.isFinite(controlPortNum) &&
    controlPortNum >= 1024 &&
    controlPortNum <= 65535;

  const commitMixedPort = () => {
    if (mixedPortValid) {
      if (mixedPortNum !== settings.mixedPort) {
        commit({ mixedPort: mixedPortNum });
      }
    } else {
      setMixedPortDraft(String(settings.mixedPort));
    }
  };

  const commitControlPort = () => {
    if (controlPortValid) {
      if (controlPortNum !== settings.controlPort) {
        commit({ controlPort: controlPortNum });
      }
    } else {
      setControlPortDraft(String(settings.controlPort));
    }
  };

  const dnsStrategyGroups: SelectGroup[] = useMemo(
    () => [
      {
        label: dict.settings.dnsStrategyLabel,
        options: [
          { value: "prefer_ipv4", label: dict.settings.dnsStrategyPrefer4 },
          { value: "ipv4_only", label: dict.settings.dnsStrategyOnly4 },
          { value: "prefer_ipv6", label: dict.settings.dnsStrategyPrefer6 },
          { value: "ipv6_only", label: dict.settings.dnsStrategyOnly6 },
        ],
      },
    ],
    [dict]
  );

  return (
    <section className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto w-full max-w-3xl space-y-5 px-6 py-5">
        <div className="flex items-center justify-between">
          <h1 className="text-[16px] font-semibold tracking-[-0.015em] text-[var(--text-primary)]">
            {dict.settings.title}
          </h1>
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

        {/* General */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.settings.secGeneral}
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.langLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.langDesc}
              </div>
            </div>
            <div className="inline-flex rounded-[8px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5 text-[12px]">
              {(["zh-CN", "en"] as Locale[]).map((l) => (
                <button
                  key={l}
                  type="button"
                  onClick={() => onChangeLocale(l)}
                  className={`rounded-[6px] px-3 py-1 font-medium cursor-pointer ${
                    locale === l
                      ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                      : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  {l === "zh-CN" ? "ZH" : "EN"}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.themeLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.themeDesc}
              </div>
            </div>
            <div className="inline-flex rounded-[8px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] p-0.5 text-[12px]">
              {(["system", "light", "dark"] as ThemeMode[]).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => onChangeTheme(m)}
                  className={`rounded-[6px] px-2.5 py-1 font-medium cursor-pointer ${
                    theme === m
                      ? "bg-[var(--accent-primary)] text-[var(--accent-primary-fg)]"
                      : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                  }`}
                >
                  {m === "system"
                    ? dict.settings.themeSystem
                    : m === "light"
                      ? dict.settings.themeLight
                      : dict.settings.themeDark}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.trayLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.trayDesc}
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.minimizeToTray}
              onClick={() => commit({ minimizeToTray: !settings.minimizeToTray })}
              className={`relative h-[18px] w-8 shrink-0 rounded-full transition-colors cursor-pointer ${
                settings.minimizeToTray
                  ? "bg-[var(--accent-primary)]"
                  : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
              }`}
            >
              <span
                className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                style={{ left: settings.minimizeToTray ? "16px" : "2px" }}
              />
            </button>
          </div>
        </div>

        {/* Network & Ports */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.settings.secNetwork}
          </div>

          <div className="grid grid-cols-1 gap-4 px-4 py-3 sm:grid-cols-2">
            <div className="space-y-1">
              <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                {dict.settings.mixedPortLabel}
              </label>
              <input
                type="text"
                inputMode="numeric"
                value={mixedPortDraft}
                onChange={(e) =>
                  setMixedPortDraft(e.target.value.replace(/[^0-9]/g, "").slice(0, 5))
                }
                onBlur={commitMixedPort}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    commitMixedPort();
                    e.currentTarget.blur();
                  }
                }}
                className="ui-input h-8 w-full px-3 font-mono text-[12px] num-tabular"
              />
              {!mixedPortValid && (
                <div
                  className="text-[11px]"
                  style={{ color: "var(--danger-text)" }}
                >
                  {dict.settings.portRangeHint}
                </div>
              )}
            </div>

            <div className="space-y-1">
              <label className="block text-[12px] font-medium text-[var(--text-primary)]">
                {dict.settings.controlPortLabel}
              </label>
              <input
                type="text"
                inputMode="numeric"
                value={controlPortDraft}
                onChange={(e) =>
                  setControlPortDraft(e.target.value.replace(/[^0-9]/g, "").slice(0, 5))
                }
                onBlur={commitControlPort}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    commitControlPort();
                    e.currentTarget.blur();
                  }
                }}
                className="ui-input h-8 w-full px-3 font-mono text-[12px] num-tabular"
              />
              {!controlPortValid && (
                <div
                  className="text-[11px]"
                  style={{ color: "var(--danger-text)" }}
                >
                  {dict.settings.portRangeHint}
                </div>
              )}
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.allowLanLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.allowLanDesc}
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.allowLan}
              onClick={() => commit({ allowLan: !settings.allowLan })}
              className={`relative h-[18px] w-8 shrink-0 rounded-full transition-colors cursor-pointer ${
                settings.allowLan
                  ? "bg-[var(--accent-primary)]"
                  : "border border-[var(--border-strong)] bg-[var(--bg-track)]"
              }`}
            >
              <span
                className="absolute top-[2px] h-3.5 w-3.5 rounded-full bg-[var(--bg-surface)] transition-all"
                style={{ left: settings.allowLan ? "16px" : "2px" }}
              />
            </button>
          </div>

          <div className="space-y-1 px-4 py-3">
            <label className="block text-[12px] font-medium text-[var(--text-primary)]">
              {dict.settings.testUrlLabel}
            </label>
            <input
              type="text"
              value={testUrlDraft}
              onChange={(e) => setTestUrlDraft(e.target.value)}
              onBlur={() => {
                const v = testUrlDraft.trim();
                if (v && v !== settings.latencyTestUrl) {
                  commit({ latencyTestUrl: v });
                } else if (!v) {
                  setTestUrlDraft(settings.latencyTestUrl);
                }
              }}
              className="ui-input h-8 w-full px-3 font-mono text-[12px]"
            />
          </div>
        </div>

        {/* DNS Resolution Settings */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.settings.secDns}
          </div>

          <div className="space-y-2 px-4 py-3">
            <div>
              <label className="block text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.dnsLocalLabel}
              </label>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.dnsLocalDesc}
              </div>
            </div>
            <input
              type="text"
              value={dnsLocalDraft}
              onChange={(e) => setDnsLocalDraft(e.target.value)}
              onBlur={() => {
                const v = dnsLocalDraft.trim();
                if (v && v !== settings.dnsLocalServer) {
                  commit({ dnsLocalServer: v });
                } else if (!v) {
                  setDnsLocalDraft(settings.dnsLocalServer);
                }
              }}
              className="ui-input h-8 w-full px-3 font-mono text-[12px]"
            />
            <div className="flex flex-wrap gap-1.5">
              {LOCAL_DNS_PRESETS.map((p) => (
                <button
                  key={p.value}
                  type="button"
                  onClick={() => {
                    setDnsLocalDraft(p.value);
                    commit({ dnsLocalServer: p.value });
                  }}
                  className={`rounded-[6px] border px-2 py-0.5 font-mono text-[11px] transition-colors cursor-pointer ${
                    dnsLocalDraft === p.value
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
            <div>
              <label className="block text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.dnsRemoteLabel}
              </label>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.dnsRemoteDesc}
              </div>
            </div>
            <input
              type="text"
              value={dnsRemoteDraft}
              onChange={(e) => setDnsRemoteDraft(e.target.value)}
              onBlur={() => {
                const v = dnsRemoteDraft.trim();
                if (v && v !== settings.dnsRemoteServer) {
                  commit({ dnsRemoteServer: v });
                } else if (!v) {
                  setDnsRemoteDraft(settings.dnsRemoteServer);
                }
              }}
              className="ui-input h-8 w-full px-3 font-mono text-[12px]"
            />
            <div className="flex flex-wrap gap-1.5">
              {REMOTE_DNS_PRESETS.map((p) => (
                <button
                  key={p.value}
                  type="button"
                  onClick={() => {
                    setDnsRemoteDraft(p.value);
                    commit({ dnsRemoteServer: p.value });
                  }}
                  className={`rounded-[6px] border px-2 py-0.5 font-mono text-[11px] transition-colors cursor-pointer ${
                    dnsRemoteDraft === p.value
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
                {dict.settings.dnsStrategyLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.dnsStrategyDesc}
              </div>
            </div>
            <CustomSelect
              value={settings.dnsStrategy}
              groups={dnsStrategyGroups}
              onChange={(val) => commit({ dnsStrategy: val })}
              className="w-44"
            />
          </div>

          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--text-primary)]">
                {dict.settings.dnsFakeIpLabel}
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.dnsFakeIpDesc}
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
        </div>

        {/* NetGuard & Storage */}
        <div className="ui-card divide-y divide-[var(--border-hairline)] overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.settings.secNetGuard}
          </div>
          <div className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-1 items-start gap-2.5">
              <ShieldCheck
                className="mt-0.5 h-4 w-4 shrink-0 text-[var(--text-tertiary)]"
                strokeWidth={1.5}
              />
              <p className="text-[12px] text-[var(--text-secondary)]">
                {dict.settings.netguardDesc}
              </p>
            </div>
            <button
              type="button"
              onClick={onRestoreNetguard}
              className="ui-btn-secondary h-8 shrink-0"
            >
              {dict.settings.restoreNow}
            </button>
          </div>

          <div className="space-y-1 px-4 py-3 text-[12px]">
            <div className="flex items-center gap-2">
              <HardDrive
                className="h-4 w-4 shrink-0 text-[var(--text-tertiary)]"
                strokeWidth={1.5}
              />
              <span className="font-medium text-[var(--text-primary)]">
                {coreInfo.portableMode
                  ? dict.settings.portableActive
                  : dict.settings.portableStandard}
              </span>
            </div>
            <div className="break-all pl-6 font-mono text-[11px] text-[var(--text-secondary)]">
              {coreInfo.dataDir}
            </div>
          </div>
        </div>

        {/* Advanced JSON Override */}
        <div className="ui-card overflow-hidden">
          <div className="bg-[var(--bg-subtle)] px-4 py-2 text-[11.5px] font-medium text-[var(--text-secondary)]">
            {dict.settings.secOverride}
          </div>
          <div className="space-y-2 p-4">
            <p className="text-[11.5px] text-[var(--text-secondary)]">
              {dict.settings.overrideDesc}
            </p>
            <textarea
              rows={3}
              value={overrideDraft}
              onChange={(e) => setOverrideDraft(e.target.value)}
              onBlur={() =>
                commit({
                  customJsonOverride: overrideDraft.trim() || null,
                })
              }
              placeholder={dict.settings.overridePlaceholder}
              className="ui-input w-full p-2.5 font-mono text-[11.5px]"
            />
          </div>
        </div>

        {/* About */}
        <div className="ui-card flex items-center justify-between gap-4 p-4">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[10px] border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--text-primary)]">
              <BrandLogo className="h-5 w-5" />
            </span>
            <div>
              <div className="text-[13px] font-semibold text-[var(--text-primary)]">
                Portline{" "}
                <span className="font-mono text-[11.5px] text-[var(--text-secondary)]">
                  v0.1.0
                </span>
              </div>
              <div className="text-[11.5px] text-[var(--text-secondary)]">
                {dict.settings.aboutDesc}
              </div>
            </div>
          </div>
          <span className="shrink-0 rounded-full border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-2.5 py-0.5 font-mono text-[11px] text-[var(--text-secondary)]">
            MIT
          </span>
        </div>
      </div>
    </section>
  );
}
