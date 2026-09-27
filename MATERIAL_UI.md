# Zephyr Material UI / Remote Scene v4

`Runtime State ABI v5` carries:

```text
screen cursor brightness dnd airplane theme locked wifi bt adb notifications
```

Rhai owns navigation/policy. Rust renders the physical ST7789 RGB565 surface
from the compact Material scene and stores its framebuffer in PSRAM. The PC
viewer mirrors the semantic scene over protocol v4:

```text
@ZWUI|M3|screen|cursor|brightness|dnd|airplane|theme|locked|wifi|bt|adb|notes|time_minutes|swap_pages
```

SystemUI flow in v0.3:

```text
Lock screen -> Watch face -> Launcher
                |    |        |
             Messages Quick   Settings / Clock / Connect / About
```

The lock/watchface time is centered. Battery telemetry is deliberately not
invented: development builds display `USB` until a real battery gauge HAL is
wired.
