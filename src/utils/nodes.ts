import type { NodeIR, Protocol, RouteMode, SimpleRoutingRule } from "../types";

export type NodeSortMode = "default" | "delay" | "name";

export const PROTO_BADGES: Record<Protocol, string> = {
  vless: "VLESS",
  vmess: "VMESS",
  trojan: "TROJAN",
  shadowsocks: "SS",
  hysteria2: "HY2",
  tuic: "TUIC",
  anytls: "ANYTLS",
  wireguard: "WG",
  http: "HTTP",
  socks: "SOCKS",
};

export interface RegionBucket {
  code: string;
  count: number;
}

export function computeAvailableRegions(nodes: NodeIR[]): RegionBucket[] {
  const counts = new Map<string, number>();
  for (const n of nodes) {
    const code = (n.region || "OTHER").toUpperCase();
    if (code === "OTHER" || !code) continue;
    counts.set(code, (counts.get(code) ?? 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([code, count]) => ({ code, count }))
    .sort((a, b) => b.count - a.count || a.code.localeCompare(b.code));
}

export function filterAndSortNodes(
  nodes: NodeIR[],
  group: string,
  query: string,
  sortMode: NodeSortMode
): NodeIR[] {
  const q = query.trim().toLowerCase();
  const filtered = nodes.filter((n) => {
    if (group !== "ALL" && n.group !== group) {
      return false;
    }
    if (!q) {
      return true;
    }
    return (
      n.remark.toLowerCase().includes(q) ||
      n.server.toLowerCase().includes(q) ||
      n.protocol.toLowerCase().includes(q) ||
      PROTO_BADGES[n.protocol].toLowerCase().includes(q) ||
      n.region.toLowerCase().includes(q)
    );
  });

  if (sortMode === "delay") {
    return filtered.slice().sort((a, b) => {
      const da = a.realDelayMs ?? a.tcpingMs ?? 999999;
      const db = b.realDelayMs ?? b.tcpingMs ?? 999999;
      return da - db;
    });
  }
  if (sortMode === "name") {
    return filtered.slice().sort((a, b) => a.remark.localeCompare(b.remark));
  }
  return filtered;
}

export interface TargetResolution {
  kind: "direct" | "reject" | "auto" | "node";
  label: string;
  node: NodeIR | null;
}

export interface TargetLabels {
  direct: string;
  reject: string;
  noNode: string;
}

export function resolveTargetExit(
  target: string,
  nodes: NodeIR[],
  mainNodeId: string | null | undefined,
  labels: TargetLabels
): TargetResolution {
  if (target === "direct") {
    return { kind: "direct", label: labels.direct, node: null };
  }
  if (target === "reject") {
    return { kind: "reject", label: labels.reject, node: null };
  }
  const mainNode = nodes.find((n) => n.id === mainNodeId) ?? nodes[0] ?? null;
  if (target === "main") {
    return {
      kind: "node",
      label: mainNode ? mainNode.remark : labels.noNode,
      node: mainNode,
    };
  }
  if (target.startsWith("auto:")) {
    const key = target.slice("auto:".length).toUpperCase();
    const candidates =
      key === "ALL"
        ? nodes
        : nodes.filter(
            (n) =>
              n.region.toUpperCase() === key ||
              n.remark.toUpperCase().includes(key)
          );
    const pool = candidates.length > 0 ? candidates : nodes;
    const best =
      pool
        .slice()
        .sort(
          (a, b) =>
            (a.realDelayMs ?? a.tcpingMs ?? 999999) -
            (b.realDelayMs ?? b.tcpingMs ?? 999999)
        )[0] ?? null;
    return {
      kind: "auto",
      label: best ? best.remark : labels.noNode,
      node: best,
    };
  }
  const specific = nodes.find((n) => n.id === target) ?? mainNode;
  return {
    kind: "node",
    label: specific ? specific.remark : labels.noNode,
    node: specific,
  };
}

export interface SimResult {
  matchedRuleId: string | null;
  ruleLabel: string;
  exit: TargetResolution;
}

export function evaluateRouteSimulation(
  rawHost: string,
  routeMode: RouteMode,
  rules: SimpleRoutingRule[],
  nodes: NodeIR[],
  mainNodeId: string | null | undefined,
  labels: TargetLabels & {
    defaultFallback: string;
    directMode: string;
    globalMode: string;
    matchedPrefix: string;
  },
  useEnglishTitle: boolean,
  presetTitles?: Record<string, string>
): SimResult {
  const host = rawHost.trim().toLowerCase();
  if (routeMode === "direct") {
    return {
      matchedRuleId: null,
      ruleLabel: labels.directMode,
      exit: { kind: "direct", label: labels.direct, node: null },
    };
  }
  if (routeMode === "global") {
    return {
      matchedRuleId: null,
      ruleLabel: labels.globalMode,
      exit: resolveTargetExit("main", nodes, mainNodeId, labels),
    };
  }

  for (const rule of rules) {
    if (!rule.enabled) continue;
    const hit = rule.patterns.some((p) => {
      const needle = p.trim().toLowerCase().replace(/^\./, "");
      if (!needle) return false;
      return host === needle || host.endsWith(`.${needle}`) || host.includes(needle);
    });
    if (hit) {
      const title =
        presetTitles?.[rule.id] ?? (useEnglishTitle ? rule.titleEn : rule.title);
      return {
        matchedRuleId: rule.id,
        ruleLabel: `${labels.matchedPrefix} ${title}`,
        exit: resolveTargetExit(rule.target, nodes, mainNodeId, labels),
      };
    }
  }

  return {
    matchedRuleId: null,
    ruleLabel: labels.defaultFallback,
    exit: resolveTargetExit("main", nodes, mainNodeId, labels),
  };
}

export function pushRingBuffer<T>(items: T[], next: T, cap = 500): T[] {
  const safeCap = Math.max(1, cap);
  if (items.length < safeCap) {
    return [next, ...items];
  }
  return [next, ...items.slice(0, safeCap - 1)];
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let val = bytes;
  let idx = 0;
  while (val >= 1024 && idx < units.length - 1) {
    val /= 1024;
    idx += 1;
  }
  return `${val >= 100 || idx === 0 ? Math.round(val) : val.toFixed(1)} ${units[idx]}`;
}

export function formatSpeed(bps: number): string {
  return `${formatBytes(bps)}/s`;
}

export function delayColorVar(ms?: number | null): string {
  if (ms == null) return "var(--text-tertiary)";
  if (ms <= 95) return "var(--success-text)";
  if (ms <= 190) return "var(--warning-text)";
  return "var(--danger-text)";
}

export const SAMPLE_SHARE_URIS = [
  "vless://11111111-1111-1111-1111-111111111111@jp1.edge-node.example:443?security=reality&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=abcd&sni=www.microsoft.com&fp=chrome&flow=xtls-rprx-vision#JP-Tokyo-01-Reality",
  "hysteria2://samplepass@us1.homeip.example:8443?sni=us1.homeip.example#US-LosAngeles-Residential-01",
  "anytls://samplepass@us2.homeip.example:443?sni=us2.homeip.example#US-SanJose-Native-02",
  "tuic://22222222-2222-2222-2222-222222222222:samplepass@sg1.fastline.example:20443?congestion_control=bbr&sni=sg1.fastline.example#SG-Streaming-01",
  "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206QUFBQUFBQUFBQUFBQUFBQUFBQUFBQT09@hk1.fastline.example:10443#HK-IPLC-01",
  "wireguard://AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=@198.51.100.10:51820?publickey=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=&address=10.0.0.2/32#DE-Frankfurt-WG",
].join("\n");
