#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

extern crate alloc;
use esp_backtrace as _;

#[cfg(all(feature = "display-ssd1306", feature = "display-st7789"))]
compile_error!("enable exactly one display backend: display-ssd1306 OR display-st7789");
#[cfg(not(any(feature = "display-ssd1306", feature = "display-st7789")))]
compile_error!("enable a display backend: display-ssd1306 OR display-st7789");

#[cfg(not(feature = "pc-block-boot"))]
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::BlockDevice;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    main,
    spi::{Mode, master::{Config as SpiConfig, Spi}},
    time::Rate,
    usb_serial_jtag::UsbSerialJtag,
};
#[cfg(feature = "radio-services")]
use esp_hal::{ram, timer::timg::TimerGroup};
#[cfg(feature = "display-ssd1306")]
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_println::println;

mod adb;
mod android_boot;
mod android_storage;
mod avb;
mod cpio;
mod boot_control;
mod gpt;
mod config;
mod desktop;
mod display;
#[cfg(not(feature = "pc-block-boot"))]
mod fastboot;
mod font;
#[cfg(feature = "display-ssd1306")]
mod framebuffer;
#[cfg(not(feature = "pc-block-boot"))]
mod input;
mod lp;
mod material;
#[cfg(feature = "radio-services")]
mod net;
mod page_store;
mod runtime;
mod rtc;
#[cfg(feature = "radio-services")]
mod radio;
#[cfg(feature = "ble-hid")]
mod ble_host;
#[cfg(feature = "ble-hid")]
mod ble_bonds;
#[cfg(feature = "pc-block-boot")]
mod usb_pc_block;

use desktop::{DesktopEvent, WifiCredentials};
#[cfg(not(feature = "pc-block-boot"))]
use desktop::{DesktopInput, DesktopMirror, DesktopSurface};
use display::BootDisplay;
#[cfg(feature = "display-ssd1306")]
use display::Ssd1306Display;
#[cfg(feature = "display-st7789")]
use display::St7789Display;
#[cfg(not(feature = "pc-block-boot"))]
use input::{BootKeys, BootMode};

esp_bootloader_esp_idf::esp_app_desc!();

mod boot;
mod persistence;
mod state_flow;
#[cfg(feature = "radio-services")]
mod radio_bridge;
#[cfg(not(feature = "pc-block-boot"))]
mod boot_modes;

#[main]
fn main() -> ! {
    boot::run()
}
