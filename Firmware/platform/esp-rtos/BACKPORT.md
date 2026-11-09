# Local esp-rtos 0.2.0 context-restore fix

Base: crates.io `esp-rtos` 0.2.0, upstream source revision
`5ba50dd2d21136370050dd2f7a767761e01cfeef` (`esp-rtos` directory).
The published crate's source/build files are retained, with the upstream
MIT and Apache-2.0 licenses. The application selects this copy through
`[patch.crates-io]`; nothing is changed in the user's Cargo registry cache.

Only `src/task/xtensa.rs` has a source-code change:

- New tasks start with EXCM, UM, WOE and CALLINC=call4 in PS.
- The naked idle entry uses EXCM, UM and WOE; it does not inherit CALLINC
  or other flags from the task which starts the scheduler.
- Function addresses are cast through pointers, matching upstream v0.3.0
  and avoiding warnings from the current Rust toolchain.

These are the scheduler fixes from:

- https://github.com/esp-rs/esp-hal/pull/4580
- https://github.com/esp-rs/esp-hal/pull/4591

They prevent interrupts during partial context restoration and prevent the
idle context from inheriting an inappropriate register-window state. The
upstream v0.3.0 changelog lists these as fixes for rare Xtensa crashes.
The separate xtensa-lx-rt linker-vector changes from #4580 are outside this
backport; no linker script or other dependency is changed here.

This keeps the current esp-hal 1.0.0 / esp-radio 0.17.0 interfaces. A full
migration to a newer RTOS/HAL combination is a separate change. Once the
application adopts an upstream version containing these fixes, remove this
patch and directory together.

The user's 2026-11-02 PC-mode crash was symbolized using the matching local
ELF. Its stack includes `esp_rtos_queue_receive`, `btdm_controller_task`,
and `task_wrapper`; the fault address is not a valid data pointer. This
supports a context-corruption diagnosis, but hardware confirmation that
this backport resolves the reported failure is still required.
