// Модуль постоянной идентичности устройства.
//! Immutable product identity, shared by main, ADB, UI and board configuration.
//! The user's editable device name is separate from these product properties.
pub const PRODUCT_NAME: &str = "aosp_watch";
/// Постоянное название модели продукта.
pub const PRODUCT_MODEL: &str = "AOSP Watch";
/// Системный идентификатор устройства.
pub const PRODUCT_DEVICE: &str = "aosp_watch";
/// Производитель, публикуемый системными свойствами.
pub const PRODUCT_MANUFACTURER: &str = "Xopilot";
/// Марка продукта в системных свойствах.
pub const PRODUCT_BRAND: &str = "AOSP";

/// Точная модель платы, для которой собран загрузчик.
pub const BOARD: &str = "ESP32-S3-Zero-N4R2";

/// Сетевое имя устройства для протоколов подключения.
pub const HOSTNAME: &str = "aosp-wearos-watch";
/// Дружественное имя при отсутствии сохранённого пользовательского значения.
pub const DEFAULT_DEVICE_NAME: &str = "AOSP Watch";