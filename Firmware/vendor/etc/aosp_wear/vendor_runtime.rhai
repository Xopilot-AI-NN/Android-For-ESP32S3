// Модуль vendor_runtime.
// /vendor: SoC/vendor-facing HAL description.
fn vendor_hal_level() { 1 }
// Возвращает название платы, ожидаемое vendor-конфигурацией.
fn vendor_board() { "ESP32-S3-ZERO" }
// Возвращает обозначение используемого стека накопителя.
fn vendor_storage_stack() { "GPT+LIBLP" }
// Возвращает обозначение USB-транспорта vendor runtime.
fn vendor_usb_stack() { "USB-PC-BLOCK" }
