//! Board configuration for ESP32-S3-Zero-N4R2 used by Zephyr Watch.
//!
//! GPIO routing is intentionally expressed with typed `p.GPIOx` handles in
//! `main.rs`; numeric `PIN_*` constants cannot configure esp-hal pins and can
//! easily drift out of sync with the actual wiring.

pub const PRODUCT: &str = "Zephyr Watch";
pub const MODEL: &str = "ESP32-S3-Zero-N4R2";
pub const BOOTLOADER_VERSION: &str = env!("CARGO_PKG_VERSION");

// SSD1306 / I2C: SDA=GPIO5, SCL=GPIO6.
#[cfg(feature = "display-ssd1306")]
pub const OLED_ADDR: u8 = 0x3C;

// ST7789V3 / SPI3: BL=GPIO1, CS=GPIO2, DC=GPIO3, RST=GPIO4,
// MOSI=GPIO5, SCK=GPIO6.
#[cfg(feature = "display-st7789")]
pub const TFT_MHZ: u32 = 40;
#[cfg(feature = "display-st7789")]
pub const TFT_Y_OFFSET: u16 = 20;
#[cfg(feature = "display-st7789")]
pub const TFT_MADCTL: u8 = 0x00; // RGB portrait; use 0x08 if panel needs BGR

// microSD / SPI2: MISO=GPIO7, CLK=GPIO8, MOSI=GPIO9, CS=GPIO10.
// Encoder / button: A=GPIO11, B=GPIO12, power/crown=GPIO13.
// GPIO21 is reserved for a future board status LED/runtime.
pub const SD_INIT_KHZ: u32 = 400;
pub const SD_DATA_MHZ: u32 = 20;
pub const MAX_SCRIPT_BYTES: usize = 24 * 1024;
pub const BOOT_SPLASH_MS: u32 = 950;
pub const BOOT_KEY_SAMPLE_MS: u32 = 450;
