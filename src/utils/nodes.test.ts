import { describe, expect, it } from "vitest";
import en from "../locales/en.json";
import zhCN from "../locales/zh-CN.json";
import type { NodeIR, SimpleRoutingRule } from "../types";
import {
  computeAvailableRegions,
  evaluateRouteSimulation,
  filterAndSortNodes,
  formatBytes,
  formatConnectionExit,
  formatConnectionRule,
  humanizeLogMessage,
  PROTO_BADGES,
  pushRingBuffer,
} from "./nodes";

function collectLeafKeys(obj: Record<string, unknown>, prefix = ""): string[] {
  const keys: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      keys.push(...collectLeafKeys(v as Record<string, unknown>, path));
    } else {
      keys.push(path);
    }
  }
  return keys.sort();
}

const SAMPLE_NODES: NodeIR[] = [
  {
    id: "n-jp-1",
    protocol: "vless",
    remark: "JP-Tokyo-01",
    server: "jp1.example.com",
    port: 443,
    group: "VPS",
    region: "JP",
    wgLocalAddress: [],
    tcpingMs: 40,
    realDelayMs: 52,
    createdAt: "2026-11-25T00:00:00Z",
  },
  {
    id: "n-us-1",
    protocol: "hysteria2",
    remark: "US-LA-Residential",
    server: "us1.example.com",
    port: 8443,
    group: "Sub",
    region: "US",
    wgLocalAddress: [],
    tcpingMs: 120,
    realDelayMs: 145,
    createdAt: "2026-11-25T00:00:00Z",
  },
  {
    id: "n-hk-1",
    protocol: "shadowsocks",
    remark: "HK-IPLC-01",
    server: "hk1.example.com",
    port: 10443,
    group: "Sub",
    region: "HK",
    wgLocalAddress: [],
    tcpingMs: 22,
    realDelayMs: 31,
    createdAt: "2026-11-25T00:00:00Z",
  },
];

describe("Portline frontend utilities & i18n parity", () => {
  it("keeps en.json and zh-CN.json key sets strictly identical", () => {
    const enKeys = collectLeafKeys(en as Record<string, unknown>);
    const zhKeys = collectLeafKeys(zhCN as Record<string, unknown>);
    expect(zhKeys).toEqual(enKeys);
  });

  it("filters and sorts nodes by group, query, delay, and name", () => {
    const byDelay = filterAndSortNodes(SAMPLE_NODES, "ALL", "", "delay");
    expect(byDelay.map((n) => n.id)).toEqual(["n-hk-1", "n-jp-1", "n-us-1"]);

    const byGroup = filterAndSortNodes(SAMPLE_NODES, "Sub", "hy2", "default");
    expect(byGroup.map((n) => n.id)).toEqual(["n-us-1"]);

    const byName = filterAndSortNodes(SAMPLE_NODES, "ALL", "", "name");
    expect(byName.map((n) => n.id)).toEqual(["n-hk-1", "n-jp-1", "n-us-1"]);
  });

  it("caps log ring buffer strictly at configured maximum", () => {
    let buf: number[] = [];
    for (let i = 0; i < 650; i++) {
      buf = pushRingBuffer(buf, i, 500);
    }
    expect(buf.length).toBe(500);
    expect(buf[0]).toBe(649);
    expect(buf[499]).toBe(150);
  });

  it("evaluates plain-language route simulator accurately across modes", () => {
    const rules: SimpleRoutingRule[] = [
      {
        id: "r-ai",
        title: "AI Platforms",
        titleEn: "AI Platforms",
        description: "chatgpt.com",
        patterns: ["chatgpt.com", "openai.com"],
        processNames: [],
        target: "auto:US",
        enabled: true,
        priority: 1,
      },
    ];
    const labels = {
      direct: "Direct",
      reject: "Block",
      noNode: "None",
      defaultFallback: "Fallback",
      directMode: "Direct Mode",
      globalMode: "Global Mode",
      matchedPrefix: "Matched:",
    };

    const hit = evaluateRouteSimulation(
      "chatgpt.com",
      "rule",
      rules,
      SAMPLE_NODES,
      "n-jp-1",
      labels,
      true
    );
    expect(hit.matchedRuleId).toBe("r-ai");
    expect(hit.exit.node?.id).toBe("n-us-1");

    const fallback = evaluateRouteSimulation(
      "youtube.com",
      "rule",
      rules,
      SAMPLE_NODES,
      "n-jp-1",
      labels,
      true
    );
    expect(fallback.matchedRuleId).toBeNull();
    expect(fallback.exit.node?.id).toBe("n-jp-1");

    const direct = evaluateRouteSimulation(
      "chatgpt.com",
      "direct",
      rules,
      SAMPLE_NODES,
      "n-jp-1",
      labels,
      true
    );
    expect(direct.exit.kind).toBe("direct");
  });

  it("formats byte sizes cleanly", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(10 * 1024 * 1024)).toBe("10.0 MB");
  });

  it("derives available regions strictly from imported nodes and supports HTTP/SOCKS", () => {
    const auNodes: NodeIR[] = [
      { ...SAMPLE_NODES[0], id: "a1", protocol: "http", region: "AU" },
      { ...SAMPLE_NODES[0], id: "a2", protocol: "socks", region: "AU" },
      { ...SAMPLE_NODES[0], id: "a3", protocol: "vless", region: "OTHER" },
    ];
    const buckets = computeAvailableRegions(auNodes);
    expect(buckets).toEqual([{ code: "AU", count: 2 }]);
    expect(PROTO_BADGES.http).toBe("HTTP");
    expect(PROTO_BADGES.socks).toBe("SOCKS");
  });

  it("formats connection exits, rules, and log messages into human-readable labels", () => {
    const liveNodes: NodeIR[] = [
      {
        ...SAMPLE_NODES[0],
        id: "n-vless-c36e1afb",
        protocol: "vless",
        remark: "vmiss REALITY (cfedge)",
        region: "US",
      },
      {
        ...SAMPLE_NODES[0],
        id: "n-tuic-bf855003",
        protocol: "tuic",
        remark: "AU oracle-arm TUIC-22372",
        region: "AU",
      },
    ];
    const nodeById = new Map(liveNodes.map((n) => [n.id, n]));
    const exitLabels = {
      direct: "Direct",
      reject: "Blocked",
      mainExit: "Main Exit",
      autoAll: "Auto All",
      autoRegionPrefix: "Auto",
      regions: { AU: "Australia", US: "United States" },
    };

    const mainConn = formatConnectionExit(["n-vless-c36e1afb", "main-exit"], nodeById, exitLabels);
    expect(mainConn.label).toBe("vmiss REALITY (cfedge)");
    expect(mainConn.protocol).toBe("VLESS");
    expect(mainConn.groupHint).toBe("Main Exit");

    const autoConn = formatConnectionExit(["n-tuic-bf855003", "auto-au"], nodeById, exitLabels);
    expect(autoConn.label).toBe("AU oracle-arm TUIC-22372");
    expect(autoConn.protocol).toBe("TUIC");
    expect(autoConn.groupHint).toBe("Auto Australia");

    const directConn = formatConnectionExit(["direct"], nodeById, exitLabels);
    expect(directConn.kind).toBe("direct");
    expect(directConn.label).toBe("Direct");

    const rules: SimpleRoutingRule[] = [
      {
        id: "r-ai",
        title: "AI",
        titleEn: "AI Platforms",
        description: "OpenAI",
        patterns: ["openai.com", "chatgpt.com", "claude.ai"],
        processNames: [],
        target: "auto:AU",
        enabled: true,
        priority: 1,
      },
    ];
    const ruleLabels = {
      defaultFallback: "Default Fallback",
      directMode: "Direct Mode",
      globalMode: "Global Mode",
      lanDirect: "LAN Direct",
      portRulePrefix: "Port",
      domainRuleFallback: "Domain Rule",
      processRuleFallback: "Process Rule",
    };

    const aiRule = formatConnectionRule(
      "domain_suffix=[openai.com chatgpt.com claude.ai...] => route(auto-au)",
      rules,
      [],
      ruleLabels,
      { "r-ai": "AI Platforms" },
      true
    );
    expect(aiRule.label).toBe("AI Platforms");
    expect(aiRule.kind).toBe("rule");

    const finalRule = formatConnectionRule("final", rules, [], ruleLabels, {}, true);
    expect(finalRule.label).toBe("Default Fallback");
    expect(finalRule.kind).toBe("fallback");

    const directModeRule = formatConnectionRule("clash_mode=direct => route(direct)", rules, [], ruleLabels, {}, true);
    expect(directModeRule.label).toBe("Direct Mode");
    expect(directModeRule.kind).toBe("mode");

    const globalModeRule = formatConnectionRule("clash_mode=global => route(main-exit)", rules, [], ruleLabels, {}, true);
    expect(globalModeRule.label).toBe("Global Mode");
    expect(globalModeRule.kind).toBe("mode");

    const lanRule = formatConnectionRule("ip_is_private=true => route(direct)", rules, [], ruleLabels, {}, true);
    expect(lanRule.label).toBe("LAN Direct");

    const humanized = humanizeLogMessage(
      "outbound/vless[n-vless-c36e1afb]: outbound connection via [auto-au]",
      nodeById,
      exitLabels
    );
    expect(humanized).toBe(
      "outbound/vless[vmiss REALITY (cfedge)]: outbound connection via [Auto Australia]"
    );
  });
});
