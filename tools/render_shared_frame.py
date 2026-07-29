#!/usr/bin/env python3
"""Render a Desktop TUI shared-frame file to a transparent PNG."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

MAGIC = b"DTUI001\0"
HEADER_SIZE = 32


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frame", type=Path, help="Shared-frame .bin file")
    parser.add_argument("output", type=Path, help="Output PNG file")
    parser.add_argument("--font", type=Path, default=Path("/usr/share/fonts/TTF/DejaVuSansMono.ttf"))
    parser.add_argument("--font-size", type=int, default=16)
    parser.add_argument("--cell-width", type=int, default=10)
    parser.add_argument("--cell-height", type=int, default=18)
    return parser.parse_args()


def read_frame(path: Path) -> tuple[int, int, int, bytes]:
    with path.open("rb") as stream:
        for _ in range(8):
            stream.seek(0)
            header_before = stream.read(HEADER_SIZE)
            if len(header_before) != HEADER_SIZE or header_before[:8] != MAGIC:
                raise ValueError(f"{path} is not a Desktop TUI shared frame")

            width, height = struct.unpack_from("<II", header_before, 8)
            state_before = struct.unpack_from("<Q", header_before, 16)[0]
            cell_size = struct.unpack_from("<I", header_before, 24)[0]
            if width == 0 or height == 0 or cell_size != 8:
                raise ValueError("unsupported shared-frame geometry")

            buffer_size = width * height * cell_size
            buffer_index = state_before & 1
            offset = HEADER_SIZE + buffer_index * buffer_size
            stream.seek(offset)
            cells = stream.read(buffer_size)
            stream.seek(16)
            state_after = struct.unpack("<Q", stream.read(8))[0]

            if state_before == state_after and len(cells) == buffer_size:
                return width, height, cell_size, cells

    raise RuntimeError("shared frame changed repeatedly while it was being read")


def render(args: argparse.Namespace) -> None:
    width, height, cell_size, cells = read_frame(args.frame)
    font = ImageFont.truetype(str(args.font), args.font_size)
    image = Image.new(
        "RGBA",
        (width * args.cell_width, height * args.cell_height),
        (0, 0, 0, 0),
    )
    draw = ImageDraw.Draw(image)

    for index in range(width * height):
        offset = index * cell_size
        codepoint = struct.unpack_from("<I", cells, offset)[0]
        if codepoint in (0, 32):
            continue

        red, green, blue, alpha = cells[offset + 4 : offset + 8]
        x = (index % width) * args.cell_width
        y = (index // width) * args.cell_height - 2
        draw.text((x, y), chr(codepoint), font=font, fill=(red, green, blue, alpha))

    args.output.parent.mkdir(parents=True, exist_ok=True)
    image.save(args.output, optimize=True)


if __name__ == "__main__":
    render(parse_args())
