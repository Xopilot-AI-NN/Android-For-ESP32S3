# Native USB host / hub / flash-drive backend

The software architecture is prepared so a USB Mass Storage device can later
implement the same block-device role as microSD or `pc-block-boot`.

## Software stack

Current Rust ESP32-S3 support provides:

```text
esp-hal USB OTG host
        |
        v
embassy-usb-host
        |
        +-- Hub class
        |
        +-- MSC Bulk-Only Transport
              |
              v
            SCSI LUN
              |
              v
        512-byte block backend
              |
              v
       GPT -> A/B -> AVB -> boot images
```

The AOSP/GPT code is intentionally generic over a block device, so storage
policy does not need to be rewritten for USB MSC.

## Hardware required for the final board

A standards-compliant watch board needs one of these designs:

1. A separate USB host connector with a controlled 5 V VBUS switch, while the
   charging/debug USB-C remains a device port; or
2. A USB-C DRP/PD role controller plus VBUS source/sink power-path management;
   or
3. An external USB PHY/controller when simultaneous debug-device and host USB
   operation is required.

A self-powered hub is useful because it supplies downstream flash-drive power,
but a hub alone does not change the fixed USB-C role wiring of the current
ESP32-S3-Zero board.

## Planned firmware backend

When the final hardware exists, add `usb-msc-boot` next to the current storage
backends. It should enumerate the root device/hub, locate a USB Mass Storage
SCSI/BBB interface, open LUN 0, expose 512-byte reads/writes, and call the same
`android_storage::load_block_device(...)` entry point.
