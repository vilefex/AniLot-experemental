#!/usr/bin/env python3
"""Генерация иконок всех платформ из assets/icon.png.

  make_icons.py ico <out.ico>
  make_icons.py macos <out.iconset>
  make_icons.py ios <out_dir>
  make_icons.py android <res_dir>
"""
import os
import sys

from PIL import Image

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "icon.png")


def load():
    img = Image.open(SRC).convert("RGBA")
    side = min(img.size)
    left, top = (img.width - side) // 2, (img.height - side) // 2
    return img.crop((left, top, left + side, top + side))


def save(img, path, px):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    img.resize((px, px), Image.LANCZOS).save(path)


def flat(img):
    """iOS не принимает прозрачность в иконках приложения."""
    bg = Image.new("RGB", img.size, (14, 14, 16))
    bg.paste(img, mask=img.split()[3])
    return bg


def main():
    kind, out = sys.argv[1], sys.argv[2]
    img = load()
    if kind == "ico":
        img.resize((256, 256), Image.LANCZOS).save(out, sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    elif kind == "macos":
        for base in (16, 32, 128, 256, 512):
            save(img, f"{out}/icon_{base}x{base}.png", base)
            save(img, f"{out}/icon_{base}x{base}@2x.png", base * 2)
    elif kind == "ios":
        rgb = flat(img)
        for name, px in (("AppIcon60x60@2x.png", 120), ("AppIcon60x60@3x.png", 180), ("AppIcon76x76@2x~ipad.png", 152), ("AppIcon83.5x83.5@2x~ipad.png", 167), ("AppIcon1024.png", 1024)):
            save(rgb, f"{out}/{name}", px)
    elif kind == "android":
        for dpi, px in (("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)):
            save(img, f"{out}/mipmap-{dpi}/ic_launcher.png", px)
            save(img, f"{out}/mipmap-{dpi}/ic_launcher_round.png", px)
    else:
        raise SystemExit("unknown kind")


if __name__ == "__main__":
    main()
