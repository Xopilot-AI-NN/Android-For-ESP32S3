// Модуль аппаратной конфигурации платы.
//! Board configuration for ESP32-S3-Zero-N4R2 used by AOSP Wear OS.
//!
//! GPIO routing is intentionally expressed with typed `p.GPIOx` handles in
//! `main.rs`; numeric `PIN_*` constants cannot configure esp-hal pins and can
//! easily drift out of sync with the actual wiring.

pub use crate::device_info::{
    BOARD as MODEL, PRODUCT_DEVICE as DEVICE, PRODUCT_MANUFACTURER as MANUFACTURER,
    PRODUCT_MODEL as PRODUCT,
};
/// Версия загрузчика из метаданных Cargo.
pub const BOOTLOADER_VERSION: &str = env!("CARGO_PKG_VERSION");

// SSD1306 / I2C: SDA=GPIO5, SCL=GPIO6.
#[cfg(feature = "display-ssd1306")]
pub const OLED_ADDR: u8 = 0x3C;

// ST7789V3 / SPI3: SCK=GPIO1, MOSI=GPIO2, RST=GPIO3, DC=GPIO4,
// CS=GPIO5, BL=GPIO6. Matches the user's physical wiring.
#[cfg(all(feature = "display-st7789", not(feature = "display-diagnostic")))]
pub const TFT_MHZ: u32 = 20; // Conservative SPI clock for wired ST7789V3.
#[cfg(feature = "display-diagnostic")]
/// Частота SPI дисплея, МГц; диагностическая сборка выбирает пониженное значение.
pub const TFT_MHZ: u32 = 2; // Diagnostic: slow SPI to isolate bus/timing issues.
#[cfg(feature = "display-st7789")]
/// Вертикальное смещение видимой области ST7789.
pub const TFT_Y_OFFSET: u16 = 20;
#[cfg(feature = "display-st7789")]
/// Аппаратный порядок осей и цветов контроллера ST7789.
pub const TFT_MADCTL: u8 = 0x08; // BGR portrait: physical panel swaps red/blue in RGB mode.

// microSD / SPI2: MISO=GPIO7, CLK=GPIO8, MOSI=GPIO9, CS=GPIO10.
// Encoder / button: A=GPIO11, B=GPIO12, power/crown=GPIO13.
// GPIO21 is reserved for a future board status LED/runtime.
// Planned MPU6050 / I2C: SDA=GPIO14, SCL=GPIO15, INT=GPIO16, AD0=GND (0x68).
// Reserved wiring only: the current runtime does not initialize this sensor.
#[cfg(not(feature = "pc-block-boot"))]
pub const SD_INIT_KHZ: u32 = if cfg!(feature = "sd-checked-startup") {
    100
} else {
    400
};
#[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-software-spi")))]
/// Частота передачи данных microSD, МГц.
pub const SD_DATA_MHZ: u32 = 10; // Safer for external microSD modules and jumper wires.
/// Интервал/срок BOOT_KEY_SAMPLE в миллисекундах.
pub const BOOT_KEY_SAMPLE_MS: u32 = 2500;
