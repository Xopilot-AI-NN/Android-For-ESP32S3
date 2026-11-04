#!/usr/bin/env python3
"""Assemble the README contact sheet from unmodified 240x280 device captures.

Requires Pillow. Does not connect to or change the watch.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'doc' / 'preview'
SCREENS = [
    ('watch', 'Циферблат'),
    ('launcher', 'Меню приложений'),
    ('clock-tools', 'Инструменты часов'),
    ('settings', 'Настройки'),
    ('calendar', 'Календарь'),
    ('browser', 'Браузер'),
]


def build():
    fonts = Path('/usr/share/fonts/truetype/dejavu')
    regular = ImageFont.truetype(str(fonts / 'DejaVuSans.ttf'), 15)
    small = ImageFont.truetype(str(fonts / 'DejaVuSans.ttf'), 13)
    heading = ImageFont.truetype(str(fonts / 'DejaVuSans-Bold.ttf'), 24)
    canvas = Image.new('RGB', (840, 796), '#0d1117')
    draw = ImageDraw.Draw(canvas)
    draw.text((24, 18), 'AOSP Wear · ESP32-S3', font=heading, fill='#f0f6fc')
    draw.text((24, 53), 'Кадры подключённых часов · ST7789V3 · 240×280',
              font=small, fill='#a8b3c2')
    for index, (name, label) in enumerate(SCREENS):
        image = Image.open(DEST / 'device' / f'{name}.png').convert('RGB')
        if image.size != (240, 280):
            raise ValueError(f'{name}: expected native 240x280 capture, got {image.size}')
        x, y = 24 + index % 3 * 272, 88 + index // 3 * 334
        draw.rounded_rectangle((x, y, x + 247, y + 319), 10,
                               fill='#161b22', outline='#30363d')
        draw.text((x + 124, y + 8), label, font=regular,
                  fill='#e6edf3', anchor='mt')
        canvas.paste(image, (x + 4, y + 35))
        # Labels and frames may change; the source pixels must stay untouched.
        assert canvas.crop((x + 4, y + 35, x + 244, y + 315)).tobytes() == image.tobytes()
    draw.text((24, 764), 'RGB565 → PNG · Снято 4 ноября 2026', font=small, fill='#a8b3c2')
    target = DEST / 'firmware-device.png'
    canvas.save(target, optimize=True)
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    build()
