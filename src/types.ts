export type Protocol =
  | "vless"
  | "vmess"
  | "trojan"
  | "shadowsocks"
  | "hysteria2"
  | "tuic"
  | "anytls"
  | "wireguard";

export type RouteMode = "rule" | "global" | "direct";
export type NavTab = "nodes" | "routing" | "monitor" | "core" | "settings";
export type Locale = "zh-CN" | "en";
export type ThemeMode = "system" | "light" | "dark";

export interface TlsConfig {
  enabled: boolean;
  serverName?: string;
  insecure: boolean;
  alpn: string[];
  fingerprint?: string;
  realityPublicKey?: string;
  realityShortId?: string;
}

export interface TransportConfig {
  transportType: string;
  path?: string;
  host?: string;
  serviceName?: string;
  headers: Record<string, string>;
}

export interface NodeIR {
  id: string;
  protocol: Protocol;
  remark: string;
  server: string;
  port: number;
  group: string;
  region: string;
  uuid?: string;
  password?: string;
  method?: string;
  flow?: string;
  alterId?: number;
  security?: string;
  congestionControl?: string;
  upMbps?: number;
  downMbps?: number;
  obfsType?: string;
  obfsPassword?: string;
  wgPrivateKey?: string;
  wgPeerPublicKey?: string;
  wgPresharedKey?: string;
  wgLocalAddress: string[];
  wgMtu?: number;
  wgReserved?: number[];
  tls?: TlsConfig;
  transport?: TransportConfig;
  detour?: string;
  tcpingMs?: number | null;
  realDelayMs?: number | null;
  rawUri?: string;
  createdAt: string;
}

export interface SubscriptionGroup {
  id: string;
  name: string;
  url: string;
  autoUpdateHours: number;
  lastUpdated?: string | null;
  nodeCount: number;
}

export interface InboundPort {
  id: string;
  port: number;
  note: string;
  noteEn: string;
  target: string;
  enabled: boolean;
  allowLan: boolean;
}

export interface SimpleRoutingRule {
  id: string;
  title: string;
  titleEn: string;
  description: string;
  patterns: string[];
  processNames: string[];
  ruleSet?: string | null;
  target: string;
  enabled: boolean;
  priority: number;
}

export interface AppSettings {
  locale: string;
  theme: string;
  routeMode: RouteMode;
  mainNodeId?: string | null;
  mixedPort: number;
  controlPort: number;
  allowLan: boolean;
  systemProxyEnabled: boolean;
  tunEnabled: boolean;
  autoStartCore: boolean;
  latencyTestUrl: string;
  customJsonOverride?: string | null;
}

export interface CoreInfo {
  installed: boolean;
  running: boolean;
  version?: string | null;
  binaryPath: string;
  sha256?: string | null;
  portableMode: boolean;
  dataDir: string;
  netguardArmed: boolean;
  netguardRecoveredFromCrash: boolean;
}

export interface CompileOutput {
  config: Record<string, unknown>;
  totalNodes: number;
  reachableNodes: number;
  reachableNodeIds: string[];
  synthesizedGroups: string[];
  controlSecret: string;
}

export interface PreflightReport {
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  deprecatedWarnings: string[];
}

export interface LatencyResult {
  nodeId: string;
  kind: string;
  delayMs?: number | null;
  error?: string | null;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export interface ConnectionItem {
  id: string;
  process: string;
  destination: string;
  inbound: string;
  rule: string;
  chain: string[];
  uploadBytes: number;
  downloadBytes: number;
  start: string;
}

export interface TrafficSample {
  upBytesPerSec: number;
  downBytesPerSec: number;
  memoryBytes: number;
  activeConnections: number;
}

export interface BootstrapPayload {
  coreInfo: CoreInfo;
  settings: AppSettings;
  nodes: NodeIR[];
  subscriptions: SubscriptionGroup[];
  rules: SimpleRoutingRule[];
  extraPorts: InboundPort[];
  compiled: CompileOutput;
  logs: LogEntry[];
}

export interface LiveMonitorPayload {
  coreRunning: boolean;
  connections: ConnectionItem[];
  traffic: TrafficSample;
  logs: LogEntry[];
}

export interface ImportSummary {
  nodes: NodeIR[];
  skipped: number;
  warnings: string[];
}

export interface SelectNodeResult {
  hotSwitched: boolean;
  mainNodeId: string;
  compiled: CompileOutput;
}

export interface ToastMessage {
  id: string;
  text: string;
  tone: "ok" | "err" | "neutral";
}
