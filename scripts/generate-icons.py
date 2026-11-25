"""Generates crisp anti-aliased Portline PNG and multi-size ICO icons from vector SDF geometry."""
from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path


def cubic_bezier_points(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
    steps: int = 56,
) -> list[tuple[float, float]]:
    pts: list[tuple[float, float]] = []
    for i in range(steps + 1):
        t = i / steps
        u = 1.0 - t
        x = u**3 * p0[0] + 3 * u**2 * t * p1[0] + 3 * u * t**2 * p2[0] + t**3 * p3[0]
        y = u**3 * p0[1] + 3 * u**2 * t * p1[1] + 3 * u * t**2 * p2[1] + t**3 * p3[1]
        pts.append((x, y))
    return pts


def dist_point_to_segment(
    px: float, py: float, ax: float, ay: float, bx: float, by: float
) -> float:
    abx = bx - ax
    aby = by - ay
    apx = px - ax
    apy = py - ay
    denom = abx * abx + aby * aby
    if denom <= 1e-12:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, (apx * abx + apy * aby) / denom))
    cx = ax + t * abx
    cy = ay + t * aby
    return math.hypot(px - cx, py - cy)


def sdf_rounded_rect(
    px: float, py: float, cx: float, cy: float, hx: float, hy: float, r: float
) -> float:
    qx = abs(px - cx) - (hx - r)
    qy = abs(py - cy) - (hy - r)
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    inside = min(max(qx, qy), 0.0)
    return outside + inside - r


def clamp01(v: float) -> float:
    return 0.0 if v < 0.0 else (1.0 if v > 1.0 else v)


def smooth_coverage(sdf: float, aa: float) -> float:
    return clamp01(0.5 - sdf / aa)


def build_switchboard_segments() -> list[tuple[float, float, float, float]]:
    # 32x32 viewBox matching logo.svg:
    # M10.6 10 H15 C17.8 10 18.2 16 21.4 16
    # M10.6 22 H15 C17.8 22 18.2 16 21.4 16
    segs: list[tuple[float, float, float, float]] = [
        (10.6, 10.0, 15.0, 10.0),
        (10.6, 22.0, 15.0, 22.0),
    ]
    top_curve = cubic_bezier_points((15.0, 10.0), (17.8, 10.0), (18.2, 16.0), (21.4, 16.0))
    bot_curve = cubic_bezier_points((15.0, 22.0), (17.8, 22.0), (18.2, 16.0), (21.4, 16.0))
    for curve in (top_curve, bot_curve):
        for a, b in zip(curve, curve[1:]):
            segs.append((a[0], a[1], b[0], b[1]))
    return segs


def render_icon_rgba(size: int) -> bytes:
    segs = build_switchboard_segments()
    circles = [(8.0, 10.0, 2.6), (8.0, 22.0, 2.6), (24.0, 16.0, 2.6)]

    # Slight stroke thickening at 16/24/32px keeps taskbar and tray crisp.
    stroke_boost = 1.24 if size <= 24 else (1.12 if size <= 32 else 1.0)
    # In logo.svg: transform="translate(176, 176) scale(21)" inside 1024x1024.
    # Normalize coordinate space to 1024 units -> map into 32x32 emblem space.
    emblem_offset = 176.0
    emblem_scale = 21.0
    half_stroke_32 = (2.2 / 2.0) * stroke_boost
    aa_1024 = 1024.0 / size

    ink = (0x18, 0x18, 0x1B)
    rim = (0x18, 0x18, 0x1B)
    rim_alpha = 0.14
    rim_width = max(14.0, 1024.0 / size * 1.15)

    out = bytearray(size * size * 4)
    idx = 0
    for py_i in range(size):
        sy = (py_i + 0.5) * 1024.0 / size
        grad_t = clamp01((sy - 64.0) / 896.0)
        bg_r = 255.0 * (1.0 - grad_t) + 245.0 * grad_t
        bg_g = 255.0 * (1.0 - grad_t) + 245.0 * grad_t
        bg_b = 255.0 * (1.0 - grad_t) + 247.0 * grad_t

        for px_i in range(size):
            sx = (px_i + 0.5) * 1024.0 / size

            tile_d = sdf_rounded_rect(sx, sy, 512.0, 512.0, 456.0, 456.0, 224.0)
            tile_a = smooth_coverage(tile_d, aa_1024)
            if tile_a <= 0.0:
                idx += 4
                continue

            # Subtle border rim along the ceramic-white squircle edge
            rim_d = abs(tile_d + rim_width * 0.5) - rim_width * 0.5
            rim_cov = smooth_coverage(rim_d, aa_1024) * rim_alpha

            # Map into 32x32 emblem space
            ex = (sx - emblem_offset) / emblem_scale
            ey = (sy - emblem_offset) / emblem_scale

            min_d32 = min(dist_point_to_segment(ex, ey, *s) for s in segs)
            for cx, cy, cr in circles:
                cd = abs(math.hypot(ex - cx, ey - cy) - cr)
                if cd < min_d32:
                    min_d32 = cd

            emblem_sdf_1024 = (min_d32 - half_stroke_32) * emblem_scale
            emblem_a = smooth_coverage(emblem_sdf_1024, aa_1024)

            # Composite ceramic white background -> subtle rim -> deep charcoal emblem
            r = bg_r * (1.0 - rim_cov) + rim[0] * rim_cov
            g = bg_g * (1.0 - rim_cov) + rim[1] * rim_cov
            b = bg_b * (1.0 - rim_cov) + rim[2] * rim_cov

            r = r * (1.0 - emblem_a) + ink[0] * emblem_a
            g = g * (1.0 - emblem_a) + ink[1] * emblem_a
            b = b * (1.0 - emblem_a) + ink[2] * emblem_a

            out[idx] = int(round(r))
            out[idx + 1] = int(round(g))
            out[idx + 2] = int(round(b))
            out[idx + 3] = int(round(tile_a * 255.0))
            idx += 4

    return bytes(out)


def encode_png(width: int, height: int, rgba: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw_rows = bytearray()
    stride = width * 4
    for y in range(height):
        raw_rows.append(0)
        start = y * stride
        raw_rows.extend(rgba[start : start + stride])

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_rows), level=9)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", idat)
        + chunk(b"IEND", b"")
    )


def encode_ico(png_entries: list[tuple[int, bytes]]) -> bytes:
    header = struct.pack("<HHH", 0, 1, len(png_entries))
    offset = 6 + 16 * len(png_entries)
    dirs = bytearray()
    body = bytearray()
    for size, png_bytes in png_entries:
        dim = 0 if size >= 256 else size
        dirs.extend(
            struct.pack(
                "<BBBBHHII",
                dim,
                dim,
                0,
                0,
                1,
                32,
                len(png_bytes),
                offset,
            )
        )
        body.extend(png_bytes)
        offset += len(png_bytes)
    return header + bytes(dirs) + bytes(body)


def main() -> None:
    repo = Path(__file__).resolve().parents[1]
    icons_dir = repo / "src-tauri" / "icons"
    icons_dir.mkdir(parents=True, exist_ok=True)

    rendered: dict[int, bytes] = {}
    for sz in (16, 24, 32, 48, 64, 128, 256, 512):
        rgba = render_icon_rgba(sz)
        rendered[sz] = encode_png(sz, sz, rgba)

    (icons_dir / "32x32.png").write_bytes(rendered[32])
    (icons_dir / "64x64.png").write_bytes(rendered[64])
    (icons_dir / "128x128.png").write_bytes(rendered[128])
    (icons_dir / "128x128@2x.png").write_bytes(rendered[256])
    (icons_dir / "icon.png").write_bytes(rendered[512])

    ico_bytes = encode_ico([(sz, rendered[sz]) for sz in (16, 24, 32, 48, 64, 128, 256)])
    (icons_dir / "icon.ico").write_bytes(ico_bytes)
    print(f"Generated Portline icons in {icons_dir}")


if __name__ == "__main__":
    main()
