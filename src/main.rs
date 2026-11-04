// Модуль точки входа прошивки.
#![no_std]
#![no_main]

extern crate alloc;
use esp_backtrace as _;

#[cfg(all(feature = "display-ssd1306", feature = "display-st7789"))]
compile_error!("enable exactly one display backend: display-ssd1306 OR display-st7789");
#[cfg(not(any(feature = "display-ssd1306", feature = "display-st7789")))]
compile_error!("enable a display backend: display-ssd1306 OR display-st7789");

use crate::watch_println as println;
use embedded_sdmmc::BlockDevice;
#[cfg(feature = "display-ssd1306")]
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    main,
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::Rate,
    usb_serial_jtag::UsbSerialJtag,
};
#[cfg(feature = "radio-services")]
use esp_hal::{ram, timer::timg::TimerGroup};

#[path = "services/adb.rs"]
mod adb;
use aosp_wear_loader::android_storage;
#[path = "../../App/src/lib.rs"]
mod apps;
#[path = "services/background.rs"]
mod background;
#[cfg(feature = "display-st7789")]
#[path = "hal/backlight.rs"]
mod backlight;
#[cfg(feature = "ble-hid")]
#[path = "services/ble_bonds.rs"]
mod ble_bonds;
#[cfg(feature = "ble-hid")]
#[path = "services/ble_host.rs"]
mod ble_host;
use aosp_wear_loader::boot_control;
#[path = "hal/temperature.rs"]
mod temperature;
mod config;
use aosp_wear_loader::cpio;
mod desktop;
#[path = "services/device_commands.rs"]
mod device_commands;
pub mod device_info;
#[path = "services/device_name.rs"]
mod device_name;
#[path = "hal/display.rs"]
mod display;
mod fastboot;
mod font;
#[cfg(feature = "display-ssd1306")]
#[path = "hal/framebuffer.rs"]
mod framebuffer;
use aosp_wear_loader::gpt;
#[path = "hal/input.rs"]
mod input;
mod logging;
use aosp_wear_loader::lp;
mod material;
#[path = "../../App/system/app/WearMedia/controller.rs"]
mod media;
#[cfg(feature = "radio-services")]
#[path = "services/net.rs"]
mod net;
#[path = "services/page_store.rs"]
mod page_store;
#[cfg(feature = "radio-services")]
#[path = "services/radio.rs"]
mod radio;
#[path = "services/rtc.rs"]
mod rtc;
mod runtime;
#[cfg(all(not(feature = "pc-block-boot"), feature = "sd-software-spi"))]
use aosp_wear_loader::sd_bitbang;
#[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
use aosp_wear_loader::sd_init;
#[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
use aosp_wear_loader::sd_spi;
#[cfg(feature = "display-software-spi")]
#[path = "hal/soft_spi.rs"]
mod soft_spi;
mod startup;
#[path = "services/swap_config.rs"]
mod swap_config;
#[path = "services/ui_resources.rs"]
mod ui_resources;
#[cfg(feature = "pc-block-boot")]
mod usb_pc_block;
mod viewer_stream;
#[path = "services/wallpaper.rs"]
mod wallpaper;
#[path = "../../App/system/app/Weather/main.rs"]
mod weather;

use desktop::{DesktopEvent, WifiCredentials};
use desktop::{DesktopInput, DesktopMirror, DesktopSurface};
use display::BootDisplay;
#[cfg(feature = "display-ssd1306")]
use display::Ssd1306Display;
#[cfg(feature = "display-st7789")]
use display::St7789Display;
use input::BootKeys;
use input::BootMode;

esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(feature = "ble-hid")]
#[path = "services/ble_recovery.rs"]
mod ble_recovery;
mod boot_modes;
#[path = "services/companion.rs"]
mod companion;
#[path = "services/file_store.rs"]
mod file_store;
mod init;
#[path = "services/activity_pages.rs"]
mod activity_pages;
#[path = "services/persistence.rs"]
mod persistence;
#[path = "services/power.rs"]
mod power;
#[cfg(feature = "radio-services")]
#[path = "services/radio_bridge.rs"]
mod radio_bridge;
#[path = "services/state_flow.rs"]
mod state_flow;
mod text_input;

#[main]
/// Передаёт управление инициализации платы и основному циклу init::run.
fn main() -> ! {
    init::run()
}

#[path = "services/storage_info.rs"]
pub mod storage_info;

#[path = "hal/emergency.rs"]
mod emergency;

mod boot_policy;
mod block_lines;

mod panic_record;
#[path = "services/persistent_log.rs"]
mod persistent_log;

#[path = "services/google_gateway.rs"]
mod google_gateway;
#[path = "services/google_http.rs"]
mod google_http;
#[cfg(feature = "radio-services")]
#[path = "services/google_tls.rs"]
mod google_tls;
