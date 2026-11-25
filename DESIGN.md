# Portline Design System

## Direction

**Personality:** Precision & Restraint (Antigravity-aligned minimalist desktop utility)

**Foundation:** Pure neutral (strictly zero cool-blue/cyan cast)

**Depth:** Hairline borders (`rgba(0, 0, 0, 0.06)`) with zero default shadows; whisper diffuse shadows reserved exclusively for floating containers, popovers, toasts, and modals.

**Signature Architecture:** Continuous `#F9F9FA` L-shaped chrome (single-row frameless TitleBar + compact 212px Sidebar) cradling an inset pure-white `#FFFFFF` workspace canvas with a `14px` top-left continuous radius and `1px` hairline edge. Zero-jargon plain-language UI shields users from underlying `sing-box` v1.14 strategy groups while offering transparent access to the compiled JSON.

## Brand Identity & Logo

- **Brand Name:** `Portline` (`portline.exe`, `portline-core`, `com.portline.desktop`, `portline-data/`)
- **Master Asset:** `src-tauri/icons/logo.svg` (`1024x1024` geometric vector)
- **Synced Exports:** `src-tauri/icons/{icon.ico, icon.png, 32x32.png, 64x64.png, 128x128.png, 128x128@2x.png}`, `public/favicon.svg`, `assets/social-preview.{svg,png}` (`1280x640` GitHub Social Preview), and `src/components/BrandLogo.tsx` (used in `TitleBar.tsx` and `SettingsView.tsx`).
- **Tile Geometry (Desktop/Taskbar):** Luminous ceramic-white rounded square (`#FFFFFF` to `#F6F6F8`, `rx=224` on `1024` grid, 89% optical fill) with a crisp `rgba(24, 24, 27, 0.12)` rim so it stays sharp on both Light and Dark taskbars.
- **Emblem Geometry (Switchboard Fork):** Three clean circular port terminals (two left ingress ports at `(8, 10)` and `(8, 22)`, one right egress terminal at `(24, 16)` on a `32x32` viewBox) joined by twin smooth cubic switchboard paths in deep charcoal `#18181B`, directly expressing multi-port to smart-line convergence.
- **In-App Mark (`BrandLogo`):** Crisp `18px x 18px` monochrome `currentColor` SVG (`strokeWidth="2.1"`, `strokeLinecap="round"`, `strokeLinejoin="round"`) adapting seamlessly to Light and Dark themes.

## Color Tokens

Canonical source: `src/index.css`

### Light (`:root`)
```css
--bg-chrome: #f9f9fa;
--bg-sidebar: #f9f9fa;
--bg-canvas: #ffffff;
--bg-surface: #ffffff;
--bg-subtle: #f6f6f8;
--bg-hover: #f2f2f4;
--bg-selected: #eaeaef;
--bg-track: #efeff2;

--border-hairline: rgba(0, 0, 0, 0.06);
--border-strong: rgba(0, 0, 0, 0.12);
--border-focus: rgba(24, 24, 27, 0.28);

--text-primary: #1a1a1d;      /* Never #000000 */
--text-secondary: #6e6e77;    /* WCAG AA >= 4.5:1 on #FFFFFF and #F9F9FA */
--text-tertiary: #9a9aa3;

--accent-primary: #18181b;
--accent-primary-hover: #27272a;
--accent-primary-fg: #ffffff;

--danger-text: #dc2626;       /* Revealed on hover or active error states */
--danger-bg-hover: #fef2f2;
--danger-border: rgba(220, 38, 38, 0.18);
--success-text: #15803d;
--success-dot: #10b981;
--success-bg: #f0fdf4;
--warning-text: #b45309;
--warning-dot: #f59e0b;
--warning-bg: #fefce8;
```

### Dark (`.dark`)
```css
--bg-chrome: #111113;
--bg-sidebar: #111113;
--bg-canvas: #161618;
--bg-surface: #1c1c1f;
--bg-subtle: #1c1c1f;
--bg-hover: #232327;
--bg-selected: #2a2a2f;
--bg-track: #27272b;

--border-hairline: rgba(255, 255, 255, 0.07);
--border-strong: rgba(255, 255, 255, 0.14);
--border-focus: rgba(244, 244, 245, 0.32);

--text-primary: #f4f4f5;
--text-secondary: #a1a1aa;
--text-tertiary: #71717a;

--accent-primary: #f4f4f5;
--accent-primary-hover: #e4e4e7;
--accent-primary-fg: #18181b;

--danger-text: #f87171;
--danger-bg-hover: rgba(220, 38, 38, 0.14);
--success-text: #4ade80;
--success-dot: #34d399;
--warning-text: #fbbf24;
--warning-dot: #fbbf24;
```

## Geometry, Spacing & Elevation

- **Spacing Base:** `4px` (`4, 6, 8, 12, 16, 20, 24`)
- **Layout Metrics:** Frameless TitleBar `44px`, Sidebar `212px`, Virtualized node row `44px`
- **Radii:** Controls & ghost buttons `8px`, nav items & inputs `9px-10px`, cards & popovers `14px`, modals `16px`, status pills & toggles `9999px`
- **Elevation:** Default cards/rows `none`; floating popovers/toasts `0 4px 16px rgba(0,0,0,0.04)`; modals `0 12px 36px rgba(0,0,0,0.08), 0 0 0 1px rgba(0,0,0,0.06)`

## Typography & Iconography

- **Sans Stack:** `"Segoe UI Variable Text", "Segoe UI", "Inter", "Microsoft YaHei UI", system-ui, sans-serif`
- **Mono Stack:** `"Cascadia Code", "JetBrains Mono", "Consolas", ui-monospace, monospace`
- **Scale:** `11.5px` badges/meta, `12px-12.5px` controls/table rows, `13px` body, `15px` modal headings, `16px` page titles (`-0.015em` tracking)
- **Weights:** `400` body/inactive, `500` labels/rows/active nav, `600` primary card headings
- **Numeric Discipline:** `.num-tabular` (`font-variant-numeric: tabular-nums`) mandatory on all latencies, ports, node counts, memory metrics, traffic rates, and timestamps
- **Icons:** Monochrome `lucide-react` icons strictly at `strokeWidth={1.5}` (zero emojis in UI chrome)

## Core Surface Patterns

- **Single-Row Frameless TitleBar:** Combines brand mark, core status pill, `Smart Rules / Global / Direct` segmented control, `System Proxy` & `TUN` pills, compact live throughput, `ZH/EN` toggle, theme cycle, and native window controls on one unified `44px` strip.
- **High-Density Node Pool Table (`NodesView`):** Group tabs, search box, latency sort, virtualized rows (`@tanstack/react-virtual`) supporting double-click 0ms hot-switch, rule usage tags, TCPing + Real Delay readouts, and non-blocking progress bar.
- **Zero-Jargon Simple Routing (`RoutingView`):** One-line plain-language rules (`When visiting... -> Use this exit`) with instant domain simulator highlighting matched row, clear fallback card, and collapsible optional Local Inbound Ports drawer (`:10808` + extra dedicated ports).
- **Observability (`MonitorView`):** Live 60s throughput canvas, 3 neutral KPI tiles, and flat secondary tabs switching between Active Connections and capped 500-line Ring-Buffer Logs.
- **Hover-Reveal Destructive Controls:** Neutral muted ink at rest; soft `--danger-bg-hover` wash + `--danger-text` on hover (`180ms` ease-out).
- **Auto-Saving Settings (`SettingsView`):** Every preference persists immediately with a subtle `Auto-saved` status confirmation.
