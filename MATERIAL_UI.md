# Zephyr Material UI v0.6

The SystemUI renderer is native Rust/RGB565 while Rhai owns navigation and state.
The physical ST7789 framebuffer lives in PSRAM; the PC viewer mirrors the same semantic scene.

Runtime scene ABI:

```text
@ZWUI|M3|screen|cursor|brightness|dnd|airplane|theme|locked|wifi|bt|adb|notes|time_minutes|swap_pages
```

v0.6 follows the Wear OS 6 / Material 3 Expressive visual language used on current Pixel Watch:

- true-black AMOLED-style background;
- watch-face color drives the system accent;
- large centered clock;
- pill/glanceable controls that stretch toward the display edges;
- expressive selected rows and quick-settings tiles;
- lock screen -> watch face -> app list / notifications / quick settings.

The development ST7789 is rectangular 240x280, so geometry is adapted rather than pretending it is a round Pixel Watch panel.
