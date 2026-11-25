# Portline -- Repository Agent Guide

Cross-platform desktop proxy client built with Rust, Tauri v2, and React for the `sing-box` v1.14+ native schema.

## Stack & Layout

- **Stack**: Rust + Tauri v2 + React 19 + TypeScript + Tailwind CSS v4 + `pnpm` (single root Cargo Workspace)
- **Key paths**:
  - `crates/portline-core/` -- Core domain models, URI/Base64/JSON/YAML parsers, `sing-box` v1.14+ tree-shaking compiler, Clash API control client, isolated batch latency tester, NetGuard proxy/DNS snapshot manager, SQLite storage, `.srs` rule cache, core downloader, and process supervisor
  - `crates/portline-ipc/` -- Typed JSON-RPC IPC protocol with session token auth (Windows Named Pipe SDDL / Unix socket)
  - `crates/portline-service/` -- Privileged service daemon with GUI heartbeat watchdog and NetGuard rollback
  - `src-tauri/` -- Tauri v2 non-elevated desktop host, tray menu, single-instance guard, and IPC commands
  - `src/` -- React + TypeScript UI (`src/locales/{en,zh-CN}.json`)
  - `DESIGN.md` -- Canonical UI design system and Switchboard Fork brand specification
  - `.github/workflows/{ci,release}.yml` -- Automated CI and idempotent Release

## Verification Commands

```powershell
# 1. Pure 7-bit ASCII source check
pwsh -NoProfile -File "$env:USERPROFILE\.agents\skills\project-pipeline\scripts\check_ascii.ps1" -Root "."

# 2. Frontend typecheck, tests, and build
pnpm typecheck
pnpm test
pnpm build

# 3. Rust lint and full workspace tests
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 4. Standalone portable Release build
pnpm tauri build --no-bundle
```

## Project-Specific Constraints

- Keep all `*.rs`, `*.ts`, `*.tsx`, `*.js`, `*.jsx`, `*.py`, `*.ps1`, and `*.yml` files 100% pure 7-bit ASCII English; put Simplified Chinese UI copy only in `src/locales/zh-CN.json`.
- Follow `DESIGN.md` for all UI tokens, chrome geometry, and icons.
- Never include `sing-box` in product names, repository names, crate names, or bundle identifiers (`com.portline.desktop`).
- Never link `libbox` or copy `hiddify-app` code; invoke only unmodified external official `sing-box` v1.14+ binaries out-of-process.
- Always run `sing-box check -c` preflight validation before swapping active runtime configs, and always snapshot OS proxy/DNS settings via `NetGuard` before mutating network state.
- Keep the Windows GUI strictly non-elevated; isolate privileged TUN operations to `portline-service`.
