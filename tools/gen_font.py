#!/usr/bin/env python3
"""Rasterize SixEyes' Google Sans Medium atlas into `mixins/clickgui/font_data.rs`.

Horizon gives a native mod no way to upload a texture or register a font, so
the ClickGUI and ArrayList draw Google Sans from a high-resolution coverage
atlas generated here: the masks are rasterized at 4x the 10 GUI-pixel em and
packed into one image. At run time the image is box-filtered to the GUI scale
and uploaded into the game's `horizon/ui-text` atlas, giving one texel per
physical pixel and one quad per glyph.

    python3 native/clickgui/tools/gen_font.py [PATH/TO/g_sans]

The atlas defaults to `<repo>/SixEyes/src/main/resources/assets/sixeyes/font/g_sans`.
"""
import json
import math
import struct
import sys
import zlib
from pathlib import Path

EM = 10.0
SUPERSAMPLE = 4
SIZE = EM * SUPERSAMPLE
FIRST = 32
LAST = 126
PAD = 1
ATLAS_WIDTH = 512
OUT = Path(__file__).resolve().parent.parent / "mixins" / "clickgui" / "font_data.rs"
DEFAULT_ATLAS = (
    Path(__file__).resolve().parents[4]
    / "SixEyes/src/main/resources/assets/sixeyes/font/g_sans"
)


def decode_png(path):
    """RGBA8, non-interlaced PNG -> raw RGBA bytes."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    pos, width, height, idat = 8, None, None, b""
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        chunk = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(
                ">IIBBBBB", chunk
            )
            if (depth, color, interlace) != (8, 6, 0):
                raise ValueError(f"{path}: expected RGBA8 without interlacing")
        elif kind == b"IDAT":
            idat += chunk
        elif kind == b"IEND":
            break
    raw = zlib.decompress(idat)
    bpp = 4
    stride = width * bpp
    out = bytearray(width * height * bpp)
    previous = bytearray(stride)
    at = 0
    for y in range(height):
        filter = raw[at]
        at += 1
        line = bytearray(raw[at : at + stride])
        at += stride
        if filter == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 255
        elif filter == 2:
            for i in range(stride):
                line[i] = (line[i] + previous[i]) & 255
        elif filter == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + previous[i]) >> 1)) & 255
        elif filter == 4:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                up = previous[i]
                up_left = previous[i - bpp] if i >= bpp else 0
                pa, pb, pc = abs(up - up_left), abs(left - up_left), abs(left + up - 2 * up_left)
                predictor = left if (pa <= pb and pa <= pc) else (up if pb <= pc else up_left)
                line[i] = (line[i] + predictor) & 255
        out[y * stride : (y + 1) * stride] = line
        previous = line
    return width, height, out


class Atlas:
    def __init__(self, directory):
        self.meta = json.loads((directory / "google_sans_medium.json").read_text())
        self.width, self.height = self.meta["atlas"]["width"], self.meta["atlas"]["height"]
        self.raw = decode_png(directory / "google_sans_medium.png")[2]
        self.ascender = self.meta["metrics"]["ascender"]
        self.glyphs = {g["unicode"]: g for g in self.meta["glyphs"]}

    def field(self, x, y):
        """Bilinear MTSDF sample, using the middle channel of the RGB field."""
        x0, y0 = math.floor(x), math.floor(y)
        fx, fy = x - x0, y - y0
        if x0 < 0:
            x0, fx = 0, 0.0
        if y0 < 0:
            y0, fy = 0, 0.0
        if x0 >= self.width - 1:
            x0, fx = self.width - 2, 1.0
        if y0 >= self.height - 1:
            y0, fy = self.height - 2, 1.0

        def pixel(ix, iy):
            i = (iy * self.width + ix) * 4
            return sorted(self.raw[i : i + 3])[1] / 255.0

        a, b = pixel(x0, y0), pixel(x0 + 1, y0)
        c, d = pixel(x0, y0 + 1), pixel(x0 + 1, y0 + 1)
        return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy

    def glyph(self, code):
        glyph = self.glyphs[code]
        advance = glyph["advance"]
        if "planeBounds" not in glyph or "atlasBounds" not in glyph:
            return advance, None
        plane, bounds = glyph["planeBounds"], glyph["atlasBounds"]
        top = self.ascender * SIZE - plane["top"] * SIZE
        bottom = self.ascender * SIZE - plane["bottom"] * SIZE
        left = plane["left"] * SIZE
        right = plane["right"] * SIZE
        x0, x1 = math.floor(left), math.ceil(right)
        y0, y1 = math.floor(top), math.ceil(bottom)
        mask = []
        for row in range(y1 - y0):
            y = y0 + row + 0.5
            em_y = self.ascender - y / SIZE
            fy = (plane["top"] - em_y) / (plane["top"] - plane["bottom"])
            v = self.height - (bounds["top"] + (bounds["bottom"] - bounds["top"]) * fy) - 0.5
            values = []
            for col in range(x1 - x0):
                x = x0 + col + 0.5
                fx = (x / SIZE - plane["left"]) / (plane["right"] - plane["left"])
                u = bounds["left"] + (bounds["right"] - bounds["left"]) * fx - 0.5
                t = min(1.0, max(0.0, (self.field(u, v) - 0.43) / 0.14))
                values.append(round(t * t * (3 - 2 * t) * 255))
            mask.append(values)
        return advance, (left, top, x1 - x0, y1 - y0, mask)


def main():
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_ATLAS
    atlas = Atlas(directory)

    shapes = []
    for code in range(FIRST, LAST + 1):
        advance, shape = atlas.glyph(code)
        if shape is None:
            shapes.append([code, advance * EM, 0.0, 0.0, 0, 0, None])
        else:
            left, top, w, h, mask = shape
            shapes.append([code, advance * EM, left, top, w, h, mask])

    # Shelf-pack the masks, tallest first, with two transparent texels between
    # neighbours so nearest sampling at a scaled edge cannot see the next glyph.
    order = sorted(
        (i for i, shape in enumerate(shapes) if shape[6] is not None),
        key=lambda i: -shapes[i][5],
    )
    width = max(ATLAS_WIDTH, max(shapes[i][4] + 2 * PAD for i in order))
    placed = {}
    x = y = row = 0
    for i in order:
        w, h = shapes[i][4], shapes[i][5]
        if x + w + 2 * PAD > width:
            x = 0
            y += row + 2 * PAD
            row = 0
        placed[i] = (x + PAD, y + PAD)
        x += w + 2 * PAD
        row = max(row, h)
    height = y + row + PAD

    pixels = bytearray(width * height)
    for i, (ax, ay) in placed.items():
        _, _, _, _, w, h, mask = shapes[i]
        for j, line in enumerate(mask):
            at = (ay + j) * width + ax
            pixels[at : at + w] = bytes(line)

    entries = []
    for i, (code, advance, left, top, w, h, mask) in enumerate(shapes):
        if mask is None:
            entries.append((code, advance, 0.0, 0.0, 0, 0, 0, 0))
            continue
        ax, ay = placed[i]
        entries.append(
            (code, advance, left / SUPERSAMPLE, top / SUPERSAMPLE, ax, ay, w, h)
        )

    lines = [
        "// Bitmap glyphs generated from SixEyes' Google Sans Medium MTSDF atlas",
        "// (`font/g_sans/google_sans_medium.json`), rasterized at 4x the 10px em",
        "// and packed into one coverage atlas.",
        "// Generated by `native/clickgui/tools/gen_font.py`; do not edit by hand.",
        "",
        f"pub(super) const ATLAS_W: u32 = {width};",
        f"pub(super) const ATLAS_H: u32 = {height};",
        "",
        "pub(super) struct Glyph {",
        "    /// Advance and the bitmap's offset in GUI pixels at a 10px em.",
        "    pub advance: f32,",
        "    pub x: f32,",
        "    pub y: f32,",
        "    /// The mask's rectangle in the 4x coverage atlas.",
        "    pub ax: u16,",
        "    pub ay: u16,",
        "    pub aw: u16,",
        "    pub ah: u16,",
        "}",
        "",
        "/// ASCII 32..126, in order.",
        "pub(super) const GLYPHS: [Glyph; 95] = [",
    ]
    for code, advance, x, y, ax, ay, aw, ah in entries:
        lines.append(
            f"    Glyph {{ advance: {advance:.4}, x: {x:.3}, y: {y:.3}, "
            f"ax: {ax}, ay: {ay}, aw: {aw}, ah: {ah} }}, // {chr(code)!r}"
        )
    lines += [
        "];",
        "",
        f"pub(super) const PIXELS: [u8; {len(pixels)}] = [",
    ]
    for i in range(0, len(pixels), 24):
        lines.append("    " + ", ".join(str(v) for v in pixels[i : i + 24]) + ",")
    lines += ["];", ""]
    OUT.write_text("\n".join(lines))
    print(f"{OUT}: {width}x{height} atlas, {len(pixels)} texels, {len(entries)} glyphs")


if __name__ == "__main__":
    main()
