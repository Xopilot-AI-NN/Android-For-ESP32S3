# AOSP-ESP32S3 v0.3.2 radio memory hotfix

Fixes the ESP32-S3 panic that occurred when enabling Wi-Fi after SystemUI/Rhai had started.

## Root cause

`esp-rtos` task stacks are allocated with `esp_alloc::InternalMemory`. The original runtime had only the primary ~96 KiB internal heap, which was ~91% used by the time Wi-Fi started. The Wi-Fi task therefore failed to allocate its internal stack even though ~1.8 MiB PSRAM was still free.

## Fix

- Bootloader 1.6.2.
- Adds 64 KiB of ESP32-S3 bootloader-reclaimed SRAM as a third heap region.
- Region order is: primary internal -> PSRAM -> reclaimed internal. Normal global allocations spill into PSRAM; capability-constrained `InternalMemory` allocations can skip PSRAM and use reclaimed SRAM.
- BLE HCI initialization is deferred until Bluetooth is enabled.
- Wi-Fi/BLE check internal free SRAM before start and refuse cleanly instead of intentionally walking into an allocator panic.
- PC block server handles serial read/write loss without a Python traceback.
- Firmware remains 0.3.0-dev; image format and A/B/AVB/super/swap layout are unchanged.

Run from project root:

```bash
./flash
```
