# Bootloader 1.4 -> Zephyr Android userspace ABI

Bootloader 1.4 extends the boot ABI from a single `/init.rhai` demo to an AOSP-like userspace assembled from logical partitions in `super`.

## Boot chain

1. Probe a synchronous 512-byte block device (SPI microSD or USB-PC block backend).
2. Validate protective MBR/GPT.
3. Read AOSP-style `bootloader_control` from `misc` and select A/B slot.
4. Verify `boot`, `init_boot` and `vendor_boot` with AVB hash descriptors from the selected `vbmeta`.
5. Validate Android boot image v4 headers and extract `/init.rhai` from the `init_boot` newc ramdisk.
6. Parse Android liblp geometry/metadata from physical `super`.
7. Select matching logical partitions (`*_a` or `*_b`).
8. Extract the following runtime components:

```text
/vendor/etc/zephyr/vendor_runtime.rhai
/odm/etc/zephyr/odm_runtime.rhai
/system_ext/etc/zephyr/system_ext_runtime.rhai
/system/framework/zephyr-framework.rhai
/product/etc/zephyr/product_runtime.rhai
```

9. Compile the combined Rhai bundle once and keep its `Scope` alive.
10. Call `init()`, then `render()`.
11. Dispatch crown/viewer events through `on_event(kind, value)` and render the returned system state.
12. Mark the selected A/B slot successful only after userspace has started.

## Rhai ABI v2

Native functions exported by the boot/runtime layer:

```text
boot_version()       -> i32  (2)
android_api_level()  -> i32  (1)
runtime_abi()        -> string
device_width()       -> i32
device_height()      -> i32
```

Required userspace functions:

```text
init() -> i32
on_event(kind: string, value: i32) -> i32
render() -> string
```

`render()` returns exactly four pipe-separated fields:

```text
TITLE|LINE1|LINE2|LINE3
```

The current native display protocol is deliberately small while the real compositor evolves.

## liblp bring-up subset

The parser accepts liblp metadata major version 10, minor 0..2. Bootloader 1.4 intentionally requires one contiguous LINEAR extent for every logical partition used during early userspace bring-up. This keeps the Rust early-boot parser small while preserving Android `super`/slot semantics. More complex extent layouts can be added later without changing the userspace tree.
