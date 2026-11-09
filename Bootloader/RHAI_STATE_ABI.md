# Rhai Runtime State ABI v4

Bootloader 1.5 keeps the firmware-owned persistent state as one `i32`. Rhai
owns navigation/product state and returns a semantic Material scene; Rust owns
RGB565 rendering and the ST7789 compositor.

Calls:

- `init() -> i32`
- `render(state) -> String`
- `on_event(state, kind, value) -> i32`

## State bits

- bits 0..2: screen
- bits 3..5: cursor
- bits 6..12: brightness 0..100
- bit 13: DND
- bit 14: airplane
- bits 15..16: Material palette seed (green/blue/purple/coral)

## Material scene ABI v1

`render()` uses the native helper:

`m3_scene(screen, cursor, brightness, dnd, airplane, theme)`

which returns:

`M3|screen|cursor|brightness|dnd|airplane|theme`

The semantic scene is deliberately small. It avoids allocating a large UI tree
inside Rhai and lets the Rust compositor keep the 240x280 RGB565 framebuffer in
PSRAM.
