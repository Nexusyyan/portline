<p align="center">
  <img src="assets/social-preview.png" alt="Portline" width="100%" />
</p>

<p align="center">
  <a href="README.md">English</a> | <strong>简体中文</strong> | <a href="DESIGN.md">设计规范 (DESIGN.md)</a>
</p>

<p align="center">
  <img alt="Release" src="https://img.shields.io/github/v/release/Nexusyyan/portline?style=flat-square&color=18181b&labelColor=f4f4f5" />
  <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/Nexusyyan/portline/ci.yml?style=flat-square&color=18181b&labelColor=f4f4f5" />
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-18181b?style=flat-square&labelColor=f4f4f5" />
  <img alt="Stack" src="https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20v2-18181b?style=flat-square&labelColor=f4f4f5" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT-18181b?style=flat-square&labelColor=f4f4f5" />
</p>

# Portline

**Portline** 是一款基于 **Rust + Tauri v2 + React** 打造的现代开源跨平台桌面代理客户端，面向纯净的 **`sing-box v1.14+`** 原生 Schema。

核心体验定位：**V2rayN 级高密度节点池运维效率 × 零术语人话分流与多端口入口 × 事务化防断网保护（NetGuard） × 轻量原生桌面可观测性**。

## 核心特性

- **高密度虚拟滚动节点池**：支持导入 `VLESS (Reality/WS/gRPC/HTTPUpgrade)`、`VMess`、`Trojan`、`Shadowsocks`、`Hysteria2`、`TUIC`、`AnyTLS`、`WireGuard` 分享链接、Base64 订阅、`sing-box` JSON 与 Clash YAML；双击任意节点即可经本地控制 API **0ms 零断流热切换主出口**。
- **零术语人话分流与可选多端口**：界面不暴露复杂策略组概念，直接以「当访问 X -> 走出口 Y」一行配完，内置域名去向即时模拟器、按地区自动优选最快节点（订阅刷新改名不断链），并可按需为 CLI / Docker 增开专属本地端口（如 `:10809`）。
- **严格 `sing-box v1.14+` 编译器与按需 Tree-Shaking**：原生输出 0 弃用警告配置（顶层 `http_clients`、带 `evaluate` 与 `match_response: true` 的新版 DNS 规则、`WireGuard` `endpoints`、`action: sniff / hijack-dns / reject`）；即使本地存有 2,000+ 节点，仅将实际可达节点编译进运行态配置。
- **配置预检与零断流回滚**：每次变更均先运行 `sing-box check -c` 校验；非法配置直接阻断并保留旧活跃进程，绝不踢断现有网络连接。
- **隔离式无断流测速**：支持高并发 TCPing 与独立无 TUN 临时内核批量真连接测速（Windows Job Object 托管，测完即清、零孤儿进程、零影响主连接）。
- **NetGuard 永不断网事务保护**：开启系统代理前持久化落盘系统原始代理与 DNS 快照；关闭、正常退出或异常崩溃后下次冷启动均可自动干净还原。
- **单文件便携模式**：在 `portline.exe` 同目录放置 `portable.json`（`{"portable":true}`）即可将全部数据库、运行配置与日志收敛在 `./portline-data/` 中。

## 下载安装

请前往 [GitHub Releases](https://github.com/Nexusyyan/portline/releases) 下载对应版本：

- **便携压缩包（`portline-vX.Y.Z-x86_64-windows-portable.zip`）**：解压即用，内置 `portline.exe`、`portable.json` 与许可文档，所有数据保存在 `./portline-data/`。
- **独立单文件（`portline-vX.Y.Z-x86_64-windows.exe`）**：双击直接运行，默认将数据存于系统 AppData 目录。
- **完整性校验**：每个 Release 均附带 `SHA256SUMS.txt` 校验文件。

## 本地开发与构建

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test
pnpm build
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 编译单文件便携 Release 可执行程序
pnpm tauri build --no-bundle
```

## 开源协议与合规说明

- **协议**：Portline 主仓库采用 [MIT License](LICENSE) 开源。
- **进程外干净隔离**：Portline 绝不静态或动态链接 `libbox`，亦不维护或复用任何内核 Fork 源码；仅以独立子进程方式调用带 SHA-256 校验的官方未修改 `sing-box v1.14+` 二进制文件。详见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
