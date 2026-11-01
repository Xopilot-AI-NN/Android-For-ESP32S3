#![no_std]
#![no_main]

extern crate alloc;
use esp_backtrace as _;

#[cfg(all(feature = "display-ssd1306", feature = "display-st7789"))]
compile_error!("enable exactly one display backend: display-ssd1306 OR display-st7789");
#[cfg(not(any(feature = "display-ssd1306", feature = "display-st7789")))]
compile_error!("enable a display backend: display-ssd1306 OR display-st7789");

#[cfg(not(feature = "pc-block-boot"))]
use embedded_hal_bus::spi::ExclusiveDevice;
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
use esp_println::println;

mod adb;
mod android_boot;
mod android_storage;
mod avb;
#[cfg(feature = "display-st7789")]
mod backlight;
#[cfg(feature = "ble-hid")]
mod ble_bonds;
#[cfg(feature = "ble-hid")]
mod ble_host;
mod boot_control;
mod config;
mod cpio;
mod desktop;
mod device_commands;
mod display;
mod fastboot;
mod font;
#[cfg(feature = "display-ssd1306")]
mod framebuffer;
mod gpt;
mod input;
mod lp;
mod material;
#[cfg(feature = "radio-services")]
mod net;
mod page_store;
#[cfg(feature = "radio-services")]
mod radio;
mod rtc;
mod runtime;
#[cfg(feature = "display-software-spi")]
mod soft_spi;
#[cfg(feature = "pc-block-boot")]
mod usb_pc_block;
mod wallpaper;

use desktop::{DesktopEvent, WifiCredentials};
#[cfg(not(feature = "pc-block-boot"))]
use desktop::{DesktopInput, DesktopMirror, DesktopSurface};
use display::BootDisplay;
#[cfg(feature = "display-ssd1306")]
use display::Ssd1306Display;
#[cfg(feature = "display-st7789")]
use display::St7789Display;
use input::BootKeys;
#[cfg(not(feature = "pc-block-boot"))]
use input::BootMode;

esp_bootloader_esp_idf::esp_app_desc!();

mod boot;
#[cfg(not(feature = "pc-block-boot"))]
mod boot_modes;
mod persistence;
mod power;
#[cfg(feature = "radio-services")]
mod radio_bridge;
mod state_flow;

#[main]
fn main() -> ! {
    boot::run()
}
