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


## 2026-11-03: main-stack overflow during PC boot

The ESP32-S3 linker assigns unused internal DRAM to the main stack. A static
96 KiB primary heap therefore reduces the stack even when PSRAM is mostly free.
The failing release ELF reserved 123,452 bytes for the main stack; `init::run`
reserved 97,232 bytes including its entry frame, and `service_disk_requests`
reserved another 22,032 bytes before GPT/USB/interrupt frames. On the connected
board this reproduced a stack-guard panic in USB disk reads after handoff.

The primary internal heap is now 64 KiB. PSRAM still receives ordinary overflow
allocations, while the separate 64 KiB reclaimed internal region remains available
to radio/RTOS allocations. Disk service functions have explicit non-inlining
boundaries so independent jobs do not accumulate temporaries in one stack frame.
ZPager scratch buffers are heap-backed and sized to the sectors used.

A heap-region line such as `Internal ... free: 96` is not total free RAM. Sum all
internal regions for radio capacity; PSRAM cannot substitute for internal-only
DMA, controller buffers or task stacks. A stack-guard panic is distinct from an
allocation failure. Preserve the exact ELF with each crash log for symbolization.

The fixed ELF provides 156,220 stack bytes. The disk dispatcher reserves 1,216
bytes and its redraw helper 4,736 bytes, leaving 53,036 bytes beyond these known
frames. `build_image.sh` now checks this path and requires at least 32 KiB for
nested service/renderer/USB/interrupt frames. This budget check is deliberately
not presented as an exhaustive stack proof.

Hardware validation: booted the corrected firmware on the connected ESP32-S3
using a copy of the PC disk image. Wi-Fi obtained DHCP, SNTP synchronized time,
BLE advertised and reached encrypted HID state 3, ADB returned `boot=completed`,
and the UI stream reached the launcher (screen 26) beyond the splash screen.
The original PC image was not modified during this check.


## RAM and ZPager optimization pass

The app controller and cached Material frame now live in reusable boxed heap
objects. The current default release reserves 68,128 bytes in `init::run`, down
from 97,232. Named init/disk/redraw frames leave 81,980 bytes in a 156,060-byte
main stack. The build-time check and hardware guard remain active.

Custom wallpaper cache entries share an Rc-owned pixel buffer: selecting the
custom wallpaper no longer duplicates 134,400 bytes. Frame transfer damage
uses at most 18 bounded regions and the reference framebuffer copies only those
regions. No visual design or framebuffer format changes are involved.

ZPG1 small records (up to 496 payload bytes) need no heap allocation. Larger
records retain the existing sector-sized heap scratch buffers. A 32-entry,
136-byte cursor cache avoids repeated cursor page reads and unchanged writes;
I/O errors remain retryable. Settings, PIN/account credentials and document
payloads are not cached by this mechanism. On-disk CRC/version/layout remain
unchanged; increasing ZPager backing still does not create transparent MMU RAM.

`memory` / `dumpsys meminfo` report heap bytes, internal free bytes, PSRAM free
bytes and active ZPager MiB. Use the physical readings rather than host stubs.


Hardware smoke test after this pass: 40 real viewer transitions between watch
face and quick settings, with interleaved memory queries, completed in 10.83 s.
No panic, command timeout or cursor-page error occurred. After warm-up heap
usage stayed at 1,007,000 bytes across samples; free bytes were 1,221,220 total,
19,256 internal and 1,201,964 PSRAM. This finite test does not establish a leak
proof for unrelated workloads. Wi-Fi/DHCP/SNTP and encrypted BLE HID were active.
