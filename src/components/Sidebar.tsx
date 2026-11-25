import {
  Activity,
  Code2,
  GitBranch,
  Play,
  Server,
  Settings,
  ShieldCheck,
  Square,
} from "lucide-react";
import type { Dict } from "../i18n";
import type { CoreInfo, NavTab, NodeIR } from "../types";
import { delayColorVar, PROTO_BADGES } from "../utils/nodes";

interface SidebarProps {
  dict: Dict;
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  nodeCount: number;
  activeRuleCount: number;
  connectionCount: number;
  mainNode: NodeIR | null;
  mixedPort: number;
  coreInfo: CoreInfo;
  coreBusy: boolean;
  onToggleCore: () => void;
}

export function Sidebar({
  dict,
  activeTab,
  onSelectTab,
  nodeCount,
  activeRuleCount,
  connectionCount,
  mainNode,
  mixedPort,
  coreInfo,
  coreBusy,
  onToggleCore,
}: SidebarProps) {
  const navItems: Array<{
    id: NavTab;
    label: string;
    icon: typeof Server;
    badge?: string;
    dot?: boolean;
  }> = [
    { id: "nodes", label: dict.nav.nodes, icon: Server, badge: String(nodeCount) },
    { id: "routing", label: dict.nav.routing, icon: GitBranch, badge: String(activeRuleCount) },
    { id: "monitor", label: dict.nav.monitor, icon: Activity, badge: String(connectionCount) },
    { id: "core", label: dict.nav.core, icon: Code2, dot: coreInfo.installed },
    { id: "settings", label: dict.nav.settings, icon: Settings },
  ];

  const delay = mainNode ? (mainNode.realDelayMs ?? mainNode.tcpingMs ?? null) : null;

  return (
    <aside className="flex w-[212px] shrink-0 flex-col justify-between gap-3 bg-[var(--bg-sidebar)] px-2.5 py-2.5 select-none">
      <nav className="space-y-1">
        {navItems.map((item) => {
          const Icon = item.icon;
          const active = activeTab === item.id;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelectTab(item.id)}
              className={`flex w-full items-center justify-between rounded-[9px] px-3 py-2 text-[12.5px] transition-colors cursor-pointer ${
                active
                  ? "bg-[var(--bg-selected)] font-medium text-[var(--text-primary)]"
                  : "font-normal text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              }`}
            >
              <span className="flex items-center gap-2.5">
                <Icon
                  className="h-[15px] w-[15px]"
                  style={{
                    color: active ? "var(--text-primary)" : "var(--text-tertiary)",
                  }}
                  strokeWidth={1.5}
                />
                <span>{item.label}</span>
              </span>
              {item.badge !== undefined && (
                <span className="font-mono text-[11px] num-tabular text-[var(--text-tertiary)]">
                  {item.badge}
                </span>
              )}
              {item.dot !== undefined && (
                <span
                  className="h-1.5 w-1.5 rounded-full"
                  style={{
                    backgroundColor: item.dot ? "var(--success-dot)" : "var(--text-tertiary)",
                  }}
                />
              )}
            </button>
          );
        })}
      </nav>

      {/* Bottom Active Main Node & NetGuard Summary Card */}
      <div className="ui-card space-y-2.5 p-3">
        <div className="flex items-center justify-between text-[11px] text-[var(--text-secondary)]">
          <span>{dict.nav.mainNodeLabel}</span>
          <span className="font-mono num-tabular">:{mixedPort}</span>
        </div>

        <div className="truncate text-[12.5px] font-semibold text-[var(--text-primary)]">
          {mainNode ? mainNode.remark : dict.nav.noNodeSelected}
        </div>

        {mainNode && (
          <div className="flex items-center justify-between text-[11px]">
            <span className="rounded-[5px] border border-[var(--border-hairline)] bg-[var(--bg-subtle)] px-1.5 py-0.5 font-mono text-[var(--text-secondary)]">
              {PROTO_BADGES[mainNode.protocol]}
            </span>
            <span
              className="font-mono font-medium num-tabular"
              style={{ color: delayColorVar(delay) }}
            >
              {delay != null ? `${delay} ms` : "--"}
            </span>
          </div>
        )}

        <button
          type="button"
          disabled={coreBusy}
          onClick={onToggleCore}
          className={`w-full ${coreInfo.running ? "ui-btn-secondary" : "ui-btn-primary"} h-7 text-[11.5px]`}
        >
          {coreInfo.running ? (
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

        <div className="flex items-center justify-between border-t border-[var(--border-hairline)] pt-2 text-[11px] text-[var(--text-secondary)]">
          <span className="inline-flex items-center gap-1">
            <ShieldCheck className="h-3.5 w-3.5 text-[var(--text-tertiary)]" strokeWidth={1.5} />
            {dict.nav.netguard}
          </span>
          <span
            className="font-medium"
            style={{
              color: coreInfo.netguardArmed ? "var(--success-text)" : "var(--text-secondary)",
            }}
          >
            {coreInfo.netguardArmed ? dict.nav.netguardArmed : dict.nav.netguardReady}
          </span>
        </div>
      </div>
    </aside>
  );
}
