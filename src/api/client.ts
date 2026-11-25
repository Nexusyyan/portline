import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  BootstrapPayload,
  CompileOutput,
  CoreInfo,
  ImportSummary,
  InboundPort,
  LatencyResult,
  LiveMonitorPayload,
  NodeIR,
  PreflightReport,
  RouteMode,
  SelectNodeResult,
  SimpleRoutingRule,
  SubscriptionGroup,
} from "../types";

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const api = {
  isDesktop: isTauriEnv,

  async getBootstrapState(): Promise<BootstrapPayload> {
    return invoke<BootstrapPayload>("get_bootstrap_state");
  },

  async importNodes(payload: string, groupName?: string): Promise<ImportSummary> {
    return invoke<ImportSummary>("import_nodes", { payload, groupName });
  },

  async importSubscription(name: string, url: string): Promise<SubscriptionGroup> {
    return invoke<SubscriptionGroup>("import_subscription", { name, url });
  },

  async refreshSubscription(id: string): Promise<SubscriptionGroup> {
    return invoke<SubscriptionGroup>("refresh_subscription", { id });
  },

  async deleteSubscription(id: string): Promise<void> {
    return invoke("delete_subscription", { id });
  },

  async saveNode(node: NodeIR): Promise<void> {
    return invoke("save_node", { node });
  },

  async deleteNode(id: string): Promise<boolean> {
    return invoke<boolean>("delete_node", { id });
  },

  async selectMainNode(id: string): Promise<SelectNodeResult> {
    return invoke<SelectNodeResult>("select_main_node", { id });
  },

  async setRouteMode(mode: RouteMode): Promise<CompileOutput> {
    return invoke<CompileOutput>("set_route_mode", { mode });
  },

  async toggleSystemProxy(enabled: boolean): Promise<CoreInfo> {
    return invoke<CoreInfo>("toggle_system_proxy", { enabled });
  },

  async toggleTun(enabled: boolean): Promise<CompileOutput> {
    return invoke<CompileOutput>("toggle_tun", { enabled });
  },

  async saveRule(rule: SimpleRoutingRule): Promise<CompileOutput> {
    return invoke<CompileOutput>("save_rule", { rule });
  },

  async deleteRule(id: string): Promise<CompileOutput> {
    return invoke<CompileOutput>("delete_rule", { id });
  },

  async saveInboundPort(port: InboundPort): Promise<CompileOutput> {
    return invoke<CompileOutput>("save_inbound_port", { port });
  },

  async deleteInboundPort(id: string): Promise<CompileOutput> {
    return invoke<CompileOutput>("delete_inbound_port", { id });
  },

  async updateSettings(settings: AppSettings): Promise<CompileOutput> {
    return invoke<CompileOutput>("update_settings", { settings });
  },

  async compilePreview(): Promise<CompileOutput> {
    return invoke<CompileOutput>("compile_preview");
  },

  async runPreflightCheck(simulateBad: boolean): Promise<PreflightReport> {
    return invoke<PreflightReport>("run_preflight_check", { simulateBad });
  },

  async startCore(): Promise<[CoreInfo, PreflightReport]> {
    return invoke<[CoreInfo, PreflightReport]>("start_core");
  },

  async stopCore(): Promise<CoreInfo> {
    return invoke<CoreInfo>("stop_core");
  },

  async downloadOfficialCore(): Promise<CoreInfo> {
    return invoke<CoreInfo>("download_official_core");
  },

  async importCoreBinary(path: string): Promise<CoreInfo> {
    return invoke<CoreInfo>("import_core_binary", { path });
  },

  async pickBinaryFile(): Promise<string | null> {
    if (!isTauriEnv()) return null;
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Executable", extensions: ["exe", "bin", "*"] }],
    });
    return typeof selected === "string" ? selected : null;
  },

  async runBatchTcping(nodeIds: string[]): Promise<LatencyResult[]> {
    return invoke<LatencyResult[]>("run_batch_tcping", { nodeIds });
  },

  async runBatchRealDelay(nodeIds: string[]): Promise<LatencyResult[]> {
    return invoke<LatencyResult[]>("run_batch_real_delay", { nodeIds });
  },

  async getLiveMonitor(): Promise<LiveMonitorPayload> {
    return invoke<LiveMonitorPayload>("get_live_monitor");
  },

  async closeConnection(id: string): Promise<void> {
    return invoke("close_connection", { id });
  },

  async closeAllConnections(): Promise<void> {
    return invoke("close_all_connections");
  },

  async clearLogs(): Promise<void> {
    return invoke("clear_logs");
  },

  async restoreNetguardNow(): Promise<CoreInfo> {
    return invoke<CoreInfo>("restore_netguard_now");
  },

  windowControls: {
    async minimize(): Promise<void> {
      if (isTauriEnv()) await getCurrentWindow().minimize();
    },
    async toggleMaximize(): Promise<void> {
      if (isTauriEnv()) await getCurrentWindow().toggleMaximize();
    },
    async isMaximized(): Promise<boolean> {
      if (!isTauriEnv()) return false;
      return getCurrentWindow().isMaximized();
    },
    async close(): Promise<void> {
      if (isTauriEnv()) await getCurrentWindow().close();
    },
    async onResized(cb: () => void): Promise<() => void> {
      if (!isTauriEnv()) return () => {};
      return getCurrentWindow().onResized(() => cb());
    },
  },
};
