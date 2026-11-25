# Third-Party Notices

Portline itself is licensed under the MIT License (see `LICENSE`).

Portline interacts with external, unmodified binaries strictly via out-of-process execution and does **not** statically or dynamically link GPL/LGPL proxy core source code into its application binaries.

## External Runtime Components (Out-of-Process Mere Aggregate)

### 1. sing-box
- **Project**: https://github.com/SagerNet/sing-box
- **License**: GNU General Public License v3.0 or later (`GPL-3.0-or-later`) with `name-use-assertion`
- **Corresponding Source**: Unmodified upstream source code for the exact release version (`v1.14.4` and newer) is publicly available at https://github.com/SagerNet/sing-box/releases.
- **Usage**: Invoked strictly as an unmodified standalone external executable (`core/sing-box.exe` / `sing-box`) via out-of-process CLI execution (`sing-box check -c` / `sing-box run -c`). Portable release archives may aggregate the unmodified official executable alongside Portline as a separate file (GPL-3.0 Section 5 mere aggregation) so first-time users in restricted networks can run immediately without requiring prior GitHub access. Portline is an independent third-party desktop client and is not affiliated with or endorsed by the SagerNet project.

### 2. Wintun (Windows TUN Mode, Optional)
- **Project**: https://www.wintun.net/
- **License**: GNU General Public License v2.0 (`GPL-2.0-only`) with Prebuilt Binaries Exception
- **Usage**: When TUN mode is enabled on Windows, the official signed `wintun.dll` is managed by the external runtime / privileged service without static linking into Portline.

## Open-Source Dependencies

Rust crates and npm packages used to build Portline are governed by their respective permissive open-source licenses (`MIT`, `Apache-2.0`, `BSD-3-Clause`, `0BSD`, `ISC`). Refer to `Cargo.lock` and `pnpm-lock.yaml` for the complete dependency graph.
