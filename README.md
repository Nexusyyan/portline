<p align="center">
  <img src="assets/social-preview.png" alt="Portline" width="100%" />
</p>

<p align="center">
  <strong>English</strong> | <a href="README.zh-CN.md">简体中文</a> | <a href="DESIGN.md">Design System</a>
</p>

<p align="center">
  <img alt="Release" src="https://img.shields.io/github/v/release/Nexusyyan/portline?style=flat-square&color=18181b&labelColor=f4f4f5" />
  <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Nexusyyan/portline/ci.yml?style=flat-square&color=18181b&labelColor=f4f4f5" />
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-18181b?style=flat-square&labelColor=f4f4f5" />
  <img alt="Stack" src="https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20v2-18181b?style=flat-square&labelColor=f4f4f5" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT-18181b?style=flat-square&labelColor=f4f4f5" />
</p>

# Portline

**Portline** is a calm, high-density cross-platform desktop proxy workspace built with **Rust, Tauri v2, and React**, targeting the strict **`sing-box v1.14+`** native schema.

It combines V2rayN-grade scattered node management efficiency, zero-jargon plain-language split routing, multi-port local inbounds, transactional OS proxy/DNS recovery (**NetGuard**), and modern live observability in a lightweight single-file native executable.

## Key Features

- **High-Density Virtualized Node Pool**: Import share URIs (`VLESS Reality/WS/gRPC/HTTPUpgrade`, `VMess`, `Trojan`, `Shadowsocks`, `Hysteria2`, `TUIC`, `AnyTLS`, `WireGuard`), Base64 subscriptions, `sing-box` JSON, or Clash YAML. Double-click any node for **0ms zero-downtime hot-switching** via the local Control API.
- **Zero-Jargon Routing & Multi-Port Inbounds**: Configure plain-language rules (`When visiting X -> Use exit Y`) with a live domain route simulator, dynamic fastest-by-region groups (`auto:US`, `auto:JP`, `auto:HK`, `auto:SG`, `auto:ALL`), and optional extra local ports (e.g., `:10809` pinned to US nodes for CLI/Docker).
- **Strict `sing-box v1.14+` Compiler + Reachability Tree-Shaking**: Generates zero-deprecation native JSON (`http_clients`, typed `dns.servers` with `evaluate` + `match_response: true`, WireGuard `endpoints`, `action: sniff / hijack-dns / reject`). Out of 2,000+ stored nodes, only reachable nodes are compiled into the runtime config.
- **Preflight Rollback Guarantee**: Every config change is validated with `sing-box check -c` before swapping. Invalid configurations are blocked cleanly without disconnecting active traffic.
- **Isolated Zero-Disruption Latency Testing**: Concurrent TCPing plus isolated headless no-TUN real-delay (`generate_204`) probing bound to a Windows Job Object so active connections stay uninterrupted and zero orphan processes remain.
- **NetGuard Transactional Safety**: Snapshots OS proxy/DNS settings before enabling system proxy and restores automatically on disable, exit, or crash recovery on next boot.
- **Portable Mode**: Drop `portable.json` (`{"portable":true}`) next to `portline.exe` to keep all SQLite data, runtime configs, and logs inside `./portline-data/`.

## Downloads

Pre-built Windows releases are published automatically on [GitHub Releases](https://github.com/Nexusyyan/portline/releases):

- **Portable Bundle (`portline-vX.Y.Z-x86_64-windows-portable.zip`)**: Extract and run; includes `portline.exe`, `portable.json`, `LICENSE`, and documentation. All data stays in `./portline-data/`.
- **Standalone Executable (`portline-vX.Y.Z-x86_64-windows.exe`)**: Single-file native binary storing data in OS AppData by default.
- **Integrity Check**: Verify downloaded files against `SHA256SUMS.txt` attached to every release.

## Architecture

```text
portline/
├── crates/
│   ├── portline-core/       # NodeIR, URI/Base64/JSON/YAML parsers, v1.14+ compiler, Control API, NetGuard, SQLite, tester, supervisor
│   ├── portline-ipc/        # Typed JSON-RPC IPC contract + session token + SDDL ACL
│   └── portline-service/    # Privileged service daemon with GUI heartbeat watchdog & NetGuard rollback
├── src-tauri/               # Non-elevated Tauri v2 desktop host, native tray, single-instance guard
├── src/                     # React 19 + TypeScript + Tailwind v4 bilingual UI (en / zh-CN)
└── DESIGN.md                # Canonical Antigravity Minimalist Neutral design system
```

## Development & Build

### Prerequisites

- Rust stable (`rustc` + `cargo` + `clippy`)
- Node.js 22+ and `pnpm` 11+
- WebView2 Runtime (pre-installed on Windows 10/11)

### Commands

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test
pnpm build
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Standalone single-file Release executable
pnpm tauri build --no-bundle
```

## Compliance & License

- **License**: Portline is licensed under the [MIT License](LICENSE).
- **Clean Out-of-Process Isolation**: Portline does **not** statically or dynamically link `libbox` or fork any proxy core source code. It downloads and invokes official, unmodified `sing-box v1.14+` executables strictly out-of-process with SHA-256 verification. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
