#!/usr/bin/env python3
"""生成 macOS DMG 安装窗口背景图（开发期工具，不参与打包流水线）。

产物: src-tauri/dmg/background.png
规格: 1320x800 px = 660x400 pt @2x。Finder 绘制背景图时 1 图像像素 = 1 设备像素，
     因此 Retina 屏上恰好铺满 660x400 pt 的窗口。

几何对齐（与 tauri.conf.json 的 dmg 配置保持一致）:
  windowSize           660 x 400 pt
  appPosition          180,170  (MonoSub.app 图标中心)
  applicationFolder    480,170  (Applications 图标中心)
  iconSize             128 pt    -> 图标占 106..234 pt，标签在 238..258 pt

修改本文件后重跑:
  /Users/lei/.workbuddy/binaries/python/envs/default/bin/python \
    scripts/release/make-dmg-background.py

依赖: Pillow（仅开发期需要，构建与运行不需要）。
"""

from __future__ import annotations

import pathlib

from PIL import Image, ImageDraw, ImageFont

# ---- 几何常量（单位：pt，绘制时统一乘 SCALE）----
SCALE = 2
W_PT, H_PT = 660, 400
W, H = W_PT * SCALE, H_PT * SCALE

APP_POS = (180, 200)
FOLDER_POS = (480, 200)
ICON_SIZE = 128

# ---- 配色 ----
BG_TOP = (250, 251, 252)
BG_BOTTOM = (228, 231, 237)
GLOW = (255, 255, 255)
TITLE_COLOR = (42, 47, 54)
ARROW_COLOR = (170, 178, 192)

FONT_LATIN = "/System/Library/Fonts/SFNS.ttf"


def pt(value: float) -> int:
    return int(round(value * SCALE))


def load_font(path: str, size_pt: float, index: int = 0) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, pt(size_pt), index=index)


def vertical_gradient(size: tuple[int, int], top: tuple[int, int, int],
                      bottom: tuple[int, int, int]) -> Image.Image:
    """逐行插值生成垂直渐变（比逐像素循环快两个数量级）。"""
    width, height = size
    strip = Image.new("RGB", (1, height))
    px = strip.load()
    for y in range(height):
        t = y / max(height - 1, 1)
        px[0, y] = tuple(int(round(top[i] + (bottom[i] - top[i]) * t)) for i in range(3))
    return strip.resize((width, height), Image.BILINEAR)


def add_soft_glow(base: Image.Image, center: tuple[int, int],
                  radii: tuple[int, int], alpha: int) -> None:
    """在图标行后面加一层柔和高光，让 App 图标有轻微浮起感。

    先在 1/8 分辨率上按径向衰减逐像素生成，再放大 —— 衰减曲线天然平滑，
    不会像「椭圆蒙版 + 高斯模糊」那样在边界留下一圈淡淡的等值线。
    """
    cx, cy = center
    rx, ry = radii
    sw, sh = W // 8, H // 8
    small = Image.new("L", (sw, sh), 0)
    px = small.load()
    scx, scy = cx / 8, cy / 8
    srx, sry = max(rx / 8, 1), max(ry / 8, 1)
    for y in range(sh):
        dy = (y - scy) / sry
        for x in range(sw):
            dx = (x - scx) / srx
            d = (dx * dx + dy * dy) ** 0.5
            if d < 1:
                # smoothstep 衰减，中心平缓、边缘收敛到 0
                t = 1 - d
                px[x, y] = int(alpha * t * t * (3 - 2 * t))
    mask = small.resize((W, H), Image.BICUBIC)
    layer = Image.new("RGB", (W, H), GLOW)
    base.paste(layer, (0, 0), mask)


def draw_arrow(draw: ImageDraw.ImageDraw) -> None:
    """两根图标之间的引导箭头：圆头箭杆 + 实心三角箭头。"""
    y = pt(APP_POS[1])
    x_start = pt(APP_POS[0] + ICON_SIZE / 2 + 16)
    x_end = pt(FOLDER_POS[0] - ICON_SIZE / 2 - 16)

    width = pt(3)
    head_len = pt(13)
    head_half = pt(8)

    # 箭杆：终点收在三角底边中点，靠圆形端点抹平接缝
    draw.line([(x_start, y), (x_end - head_len + 1, y)],
              fill=ARROW_COLOR, width=width)
    r = width / 2
    for cx in (x_start, x_end - head_len + 1):
        draw.ellipse((cx - r, y - r, cx + r, y + r), fill=ARROW_COLOR)
    # 三角箭头
    draw.polygon([(x_end, y),
                  (x_end - head_len, y - head_half),
                  (x_end - head_len, y + head_half)], fill=ARROW_COLOR)


def centered_text(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.FreeTypeFont,
                  center_x: int, baseline_y: int, fill: tuple[int, int, int],
                  tracking: float = 0) -> None:
    """按视觉包围盒居中绘制（Pillow 默认按墨迹盒，含字距补偿）。"""
    if tracking == 0:
        bbox = draw.textbbox((0, 0), text, font=font)
        x = center_x - (bbox[2] + bbox[0]) / 2
        draw.text((x, baseline_y), text, font=font, fill=fill)
        return

    # 手动字距
    total = sum(draw.textlength(ch, font=font) for ch in text) + tracking * pt(1) * (len(text) - 1)
    x = center_x - total / 2
    for ch in text:
        draw.text((x, baseline_y), ch, font=font, fill=fill)
        x += draw.textlength(ch, font=font) + tracking * pt(1)


def main() -> None:
    image = vertical_gradient((W, H), BG_TOP, BG_BOTTOM).convert("RGB")
    add_soft_glow(image, (pt((APP_POS[0] + FOLDER_POS[0]) / 2), pt(APP_POS[1] + 6)),
                  (pt(250), pt(120)), 150)

    draw = ImageDraw.Draw(image)
    draw_arrow(draw)

    # 只用品牌名 + 箭头，不放任何自然语言文案：
    # 应用本身是中英双语，背景图里硬编码中文会在英文环境下变成乱入。
    title_font = load_font(FONT_LATIN, 22)
    centered_text(draw, "MonoSub", title_font, W // 2, pt(34), TITLE_COLOR, tracking=0.4)

    out = pathlib.Path(__file__).resolve().parents[2] / "src-tauri" / "dmg" / "background.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    image.save(out, "PNG", optimize=True, dpi=(144, 144))
    print(f"wrote {out} ({W}x{H} px = {W_PT}x{H_PT} pt @2x, {out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
