This is esp-hal 1.0.0's ESP32-S3 linker script, from
https://github.com/esp-rs/esp-hal/blob/esp-hal-v1.0.0/esp-hal/ld/esp32s3/esp32s3.x
(MIT OR Apache-2.0, copyright esp-rs contributors).

The change is `.rotext_dummy (READONLY (TYPE = SHT_NOBITS))` with a `BYTE(0)`
type marker (NOBITS emits no data; final alignment absorbs the marker).
The original NOLOAD reservation inherits SHF_WRITE and makes the flash text
segment RWX in recent GNU ld. The explicit type preserves the NOBITS alignment
gap and removes the inappropriate writable flag. Memory origins and all HAL
section includes remain unchanged. No linker warnings are suppressed.
