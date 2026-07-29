#!/usr/bin/env python3
"""Build README screenshots and animation from real Desktop TUI frames."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

SANS_FONT = Path("/usr/share/fonts/noto/NotoSans-Regular.ttf")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frames", type=Path, help="Directory containing numbered PNG frames")
    parser.add_argument("stills", type=Path, help="Directory containing model-only.png and system.png")
    parser.add_argument("output", type=Path, help="Documentation output directory")
    return parser.parse_args()


def desktop_background(
    size: tuple[int, int],
    *,
    panel: bool = True,
    dock: bool = True,
) -> Image.Image:
    width, height = size
    image = Image.new("RGBA", size, (8, 10, 16, 255))

    glow = Image.new("RGBA", (400, 225), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse((-90, 78, 200, 310), fill=(31, 70, 102, 185))
    glow_draw.ellipse((205, -80, 500, 230), fill=(83, 47, 112, 160))
    glow_draw.ellipse((120, 45, 380, 290), fill=(119, 58, 36, 95))
    glow = glow.filter(ImageFilter.GaussianBlur(48))
    glow = glow.resize(size, Image.Resampling.BICUBIC)
    image.alpha_composite(glow)

    draw = ImageDraw.Draw(image)
    if panel:
        draw.rounded_rectangle(
            (12, 10, width - 12, 42),
            radius=11,
            fill=(9, 11, 18, 224),
            outline=(39, 43, 58, 220),
            width=1,
        )
        font = ImageFont.truetype(str(SANS_FONT), max(11, width // 145))
        clock = "21:08"
        clock_width = draw.textlength(clock, font=font)
        draw.text(
            ((width - clock_width) / 2, 18),
            clock,
            font=font,
            fill=(207, 212, 228, 230),
        )
        for index, color in enumerate(
            [(127, 143, 202), (110, 197, 176), (215, 147, 95), (190, 135, 190)]
        ):
            x = width - 106 + index * 22
            draw.ellipse((x, 20, x + 8, 28), fill=(*color, 235))

    if dock:
        dock_width = min(272, width // 5)
        dock_x = (width - dock_width) // 2
        dock_y = height - 58
        draw.rounded_rectangle(
            (dock_x, dock_y, dock_x + dock_width, dock_y + 42),
            radius=14,
            fill=(10, 12, 20, 218),
            outline=(45, 49, 65, 225),
            width=1,
        )
        icon_colors = [
            (146, 168, 255),
            (202, 205, 218),
            (228, 145, 82),
            (111, 190, 174),
            (180, 137, 205),
        ]
        spacing = dock_width // (len(icon_colors) + 1)
        for index, color in enumerate(icon_colors, start=1):
            x = dock_x + spacing * index
            draw.rounded_rectangle(
                (x - 8, dock_y + 12, x + 8, dock_y + 28),
                radius=4,
                fill=(*color, 230),
            )

    return image


def contain(image: Image.Image, box: tuple[int, int]) -> Image.Image:
    copy = image.copy()
    copy.thumbnail(box, Image.Resampling.LANCZOS)
    return copy


def trim_transparent(image: Image.Image, padding: int = 24) -> Image.Image:
    alpha = image.getchannel("A")
    bounds = alpha.getbbox()
    if bounds is None:
        return image

    left, top, right, bottom = bounds
    return image.crop(
        (
            max(0, left - padding),
            max(0, top - padding),
            min(image.width, right + padding),
            min(image.height, bottom + padding),
        )
    )


def compose_desktop(layer: Image.Image, size: tuple[int, int]) -> Image.Image:
    image = desktop_background(size)
    width, height = size
    rendered = contain(layer, (width - 80, height - 112))
    image.alpha_composite(
        rendered,
        ((width - rendered.width) // 2, 48 + (height - 106 - rendered.height) // 2),
    )
    return image.convert("RGB")


def build_hero(frames: list[Path], output: Path) -> None:
    middle = Image.open(frames[len(frames) // 2]).convert("RGBA")
    hero = compose_desktop(middle, (1600, 900))
    hero.save(output / "hero.webp", "WEBP", quality=88, method=6)


def build_animation(frames: list[Path], output: Path) -> None:
    animation = [
        compose_desktop(Image.open(frame).convert("RGBA"), (1200, 675))
        for frame in frames
    ]
    animation[0].save(
        output / "demo.gif",
        save_all=True,
        append_images=animation[1:],
        duration=90,
        loop=0,
        optimize=True,
        disposal=2,
    )


def build_template_overview(stills: Path, output: Path) -> None:
    canvas = desktop_background((1500, 820), panel=False, dock=False)
    draw = ImageDraw.Draw(canvas)
    title_font = ImageFont.truetype(str(SANS_FONT), 20)
    label_font = ImageFont.truetype(str(SANS_FONT), 15)
    title = "One widget, different canvases"
    draw.text((54, 42), title, font=title_font, fill=(234, 237, 247, 255))
    transparency = "WALLPAPER VISIBLE THROUGH EMPTY CELLS"
    transparency_width = draw.textlength(transparency, font=label_font)
    draw.text(
        (1446 - transparency_width, 46),
        transparency,
        font=label_font,
        fill=(156, 170, 221, 255),
    )

    sources = [
        ("MODEL + SYSTEM", Image.open(stills / "model-system.png").convert("RGBA")),
        ("MODEL ONLY", Image.open(stills / "model-only.png").convert("RGBA")),
        ("SYSTEM ONLY", Image.open(stills / "system.png").convert("RGBA")),
    ]
    frames = [
        (54, 102, 1392, 330),
        (54, 500, 672, 246),
        (774, 500, 672, 246),
    ]

    for (label, source), (x, y, frame_width, frame_height) in zip(sources, frames):
        draw.text((x, y - 29), label, font=label_font, fill=(184, 197, 240, 255))
        draw.rounded_rectangle(
            (x, y, x + frame_width, y + frame_height),
            radius=16,
            outline=(77, 86, 116, 235),
            width=1,
        )
        preview = contain(trim_transparent(source), (frame_width - 48, frame_height - 40))
        canvas.alpha_composite(
            preview,
            (
                x + (frame_width - preview.width) // 2,
                y + (frame_height - preview.height) // 2,
            ),
        )

    canvas.convert("RGB").save(output / "templates.webp", "WEBP", quality=88, method=6)


def main() -> None:
    args = parse_args()
    frames = sorted(args.frames.glob("[0-9][0-9].png"))
    if not frames:
        raise SystemExit(f"no numbered PNG frames found in {args.frames}")

    args.output.mkdir(parents=True, exist_ok=True)
    build_hero(frames, args.output)
    build_animation(frames, args.output)

    model_system = args.stills / "model-system.png"
    model_system.write_bytes(frames[len(frames) // 2].read_bytes())
    build_template_overview(args.stills, args.output)


if __name__ == "__main__":
    main()
