#!/usr/bin/env python3
"""Export the flo/local typographic mark using its bundled Micro 5 font."""
from pathlib import Path
from shutil import copyfile

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
WEB = ROOT / "apps/web/public"
APP = ROOT / "apps/android/app/src/main"
FONT = WEB / "fonts/micro-5.ttf"
PAPER, PANEL, ACCENT, INK = "#eee3fa", "#fcf5ff", "#e3c7f5", "#303647"
WORDMARK = "flo/local"


def mark(size: int, width: float, transparent: bool = False) -> Image.Image:
    image = (Image.new("RGBA", (size, size)) if transparent
             else Image.new("RGB", (size, size), PAPER))
    for font_size in range(size, 0, -1):
        font = ImageFont.truetype(str(FONT), font_size)
        left, top, right, bottom = font.getbbox(WORDMARK)
        if right - left <= int(size * width):
            break

    draw = ImageDraw.Draw(image)
    draw.fontmode = "1"
    text_width, text_height = right - left, bottom - top
    x, y = (size - text_width) // 2, (size - text_height) // 2
    stroke = max(1, size // 96)
    margin = max(2, size // 32)
    bar = max(3, size // 20)
    x1, y1 = x - margin, y - margin - bar
    x2, y2 = x + text_width + margin, y + text_height + margin
    shadow = stroke * 2
    draw.rectangle((x1 + shadow, y1 + shadow, x2 + shadow, y2 + shadow), fill=INK)
    draw.rectangle((x1, y1, x2, y2), fill=PANEL, outline=INK, width=stroke)
    draw.rectangle((x1 + stroke, y1 + stroke, x2 - stroke, y1 + bar), fill=ACCENT)
    draw.line((x1, y1 + bar, x2, y1 + bar), fill=INK, width=stroke)
    draw.text((x - left, y - top), WORDMARK, font=font, fill=INK)
    return image


def save(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, optimize=True)


def main() -> None:
    for size in (192, 512):
        save(mark(size, 0.78), WEB / f"icons/flo-local-{size}.png")
    save(mark(512, 0.67), WEB / "icons/flo-local-maskable-512.png")
    save(mark(180, 0.78), WEB / "icons/flo-local-apple-180.png")
    save(mark(432, 0.47, transparent=True), APP / "res/drawable-nodpi/flo_launcher_foreground.png")
    for density, size in (("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)):
        save(mark(size, 0.78), APP / f"res/mipmap-{density}/ic_launcher.png")
    font = APP / "res/font/micro_5.ttf"
    font.parent.mkdir(parents=True, exist_ok=True)
    copyfile(FONT, font)
    license_file = APP / "assets/OFL-Micro-5.txt"
    license_file.parent.mkdir(parents=True, exist_ok=True)
    copyfile(WEB / "fonts/OFL-Micro-5.txt", license_file)


if __name__ == "__main__":
    main()
