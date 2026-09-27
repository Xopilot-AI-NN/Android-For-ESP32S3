# Zephyr Watch memory layout (Bootloader 1.4.1)

ESP32-S3-Zero-N4R2 has 2 MiB PSRAM. Bootloader 1.4.0 only registered a 96 KiB
internal `esp-alloc` heap, so Rhai compilation could exhaust it while starting
`system_server`.

1.4.1 registers two global allocator regions:

- 96 KiB internal SRAM (first/preferred region)
- detected external PSRAM (N4R2: 2 MiB)

The Rhai userspace source String is also dropped as soon as the AST has been
compiled. Boot logs print `esp_alloc::HEAP.stats()` around runtime startup.

PSRAM must stay enabled in the `esp-hal` dependency (`features = [..., "psram"]`).
This target is specifically the N4R2 board; do not use this binary unchanged on
a board variant without PSRAM.
