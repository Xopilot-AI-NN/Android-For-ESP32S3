// Zephyr Watch early userspace entrypoint.
// Return 0 to mark the boot hand-off successful.
let api = boot_version();
let w = device_width();
let h = device_height();
if api == 1 && w > 0 && h > 0 { 0 } else { 1 }
