#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

extern crate alloc;
use esp_backtrace as _;

#[cfg(all(feature = "display-ssd1306", feature = "display-st7789"))]
compile_error!("enable exactly one display backend: display-ssd1306 OR display-st7789");
#[cfg(not(any(feature = "display-ssd1306", feature = "display-st7789")))]
compile_error!("enable a display backend: display-ssd1306 OR display-st7789");

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    main,
    spi::{Mode, master::{Config as SpiConfig, Spi}},
    system,
    time::Rate,
    usb_serial_jtag::UsbSerialJtag,
};
#[cfg(feature = "display-ssd1306")]
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_println::println;

mod android_boot;
mod android_storage;
mod avb;
mod boot_control;
mod gpt;
mod config;
mod desktop;
mod display;
mod fastboot;
mod font;
#[cfg(feature = "display-ssd1306")]
mod framebuffer;
mod input;
mod runtime;

use desktop::{DesktopEvent, DesktopInput, DesktopMirror, DesktopSurface};
use display::BootDisplay;
#[cfg(feature = "display-ssd1306")]
use display::Ssd1306Display;
#[cfg(feature = "display-st7789")]
use display::St7789Display;
use input::{BootKeys, BootMode};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 96 * 1024);
    let hal_cfg = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let p = esp_hal::init(hal_cfg);
    let delay = Delay::new();

    println!("{} bootloader {}", config::PRODUCT, config::BOOTLOADER_VERSION);
    println!("board={} target=esp32s3", config::MODEL);

    #[cfg(feature = "display-ssd1306")]
    let physical_display = {
        let i2c = I2c::new(
            p.I2C0,
            I2cConfig::default().with_frequency(Rate::from_khz(400)),
        )
        .expect("I2C config")
        .with_sda(p.GPIO5)
        .with_scl(p.GPIO6);
        Ssd1306Display::new(i2c).expect("SSD1306 not responding on GPIO5/6 address 0x3C")
    };

    #[cfg(feature = "display-st7789")]
    let physical_display = {
        let spi = Spi::new(
            p.SPI3,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(config::TFT_MHZ))
                .with_mode(Mode::_0),
        )
        .expect("SPI3 TFT config")
        .with_sck(p.GPIO6)
        .with_mosi(p.GPIO5);
        let cs = Output::new(p.GPIO2, Level::High, OutputConfig::default());
        let dc = Output::new(p.GPIO3, Level::Low, OutputConfig::default());
        let rst = Output::new(p.GPIO4, Level::High, OutputConfig::default());
        let bl = Output::new(p.GPIO1, Level::Low, OutputConfig::default());
        let mut init_delay = delay;
        St7789Display::new(spi, cs, dc, rst, bl, &mut init_delay).expect("ST7789V3 init failed")
    };

    // ESP32-S3-Zero exposes native USB on its Type-C connector. We keep the
    // hardware display active and mirror the same boot states over CDC.
    let usb = UsbSerialJtag::new(p.USB_DEVICE);
    let (usb_rx, usb_tx) = usb.split();
    let mut desktop_input = DesktopInput::new(usb_rx);

    #[cfg(feature = "display-st7789")]
    let mut display = DesktopMirror::new(physical_display, usb_tx, DesktopSurface::St7789);
    #[cfg(feature = "display-ssd1306")]
    let mut display = DesktopMirror::new(physical_display, usb_tx, DesktopSurface::Ssd1306);

    let mut keys = BootKeys::new(p.GPIO11, p.GPIO12, p.GPIO13);

    let _ = display.boot_logo();
    delay.delay_millis(config::BOOT_SPLASH_MS);
    let mut mode = sample_boot_mode(
        &mut display,
        &mut keys,
        &mut desktop_input,
        &delay,
        config::BOOT_KEY_SAMPLE_MS,
    );

    if mode == BootMode::Recovery {
        mode = recovery_menu(&mut display, &mut keys, &mut desktop_input, &delay);
    }
    if mode == BootMode::Fastboot {
        enter_fastboot(display, desktop_input, &mut keys, &delay);
    }

    let _ = display.boot_progress("CHECKING SD", 0);

    // SD requires >=74 clocks at <=400 kHz with CS deasserted before CMD0.
    let mut cs = Output::new(p.GPIO10, Level::High, OutputConfig::default());
    let mut spi = Spi::new(
        p.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(config::SD_INIT_KHZ))
            .with_mode(Mode::_0),
    )
    .expect("SPI2 SD config")
    .with_sck(p.GPIO8)
    .with_mosi(p.GPIO9)
    .with_miso(p.GPIO7);

    let _ = cs.set_high();
    let _ = spi.write(&[0xFF; 10]); // 80 idle clocks
    let device = ExclusiveDevice::new(spi, cs, delay).expect("SD CS pin");

    let _ = display.boot_progress("LOADING", 2);
    let mut session = match android_storage::load_system(device, delay, |card| {
        // embedded-sdmmc initializes the card while still at <=400 kHz. Once
        // initialization has completed, re-clock the SPI bus for image/GPT I/O.
        card.spi(|sd_device| {
            let fast = SpiConfig::default()
                .with_frequency(Rate::from_mhz(config::SD_DATA_MHZ))
                .with_mode(Mode::_0);
            if let Err(err) = sd_device.bus_mut().apply_config(&fast) {
                println!("warning: failed to switch SD bus to {} MHz: {:?}", config::SD_DATA_MHZ, err);
            }
        });
    }) {
        Ok(session) => session,
        Err(err) => {
            println!("android boot error: {:?}", err);
            let _ = display.error("NO SYSTEM", "USB FASTBOOT");
            enter_fastboot(display, desktop_input, &mut keys, &delay);
        }
    };

    println!("slot={} version={} entry={} rollback={} avb={}", session.image.slot.as_str(), session.image.version, session.image.entry, session.image.rollback_index, if session.image.avb_unsigned { "orange/unsigned" } else { "green" });
    let _ = display.boot_progress("STARTING", 5);

    match runtime::run(&session.image.script) {
        Ok(0) => {
            println!("runtime handoff succeeded");
            if let Err(err) = session.mark_successful() {
                println!("warning: failed to mark slot successful: {:?}", err);
            }
            let avb_state = if session.image.avb_unsigned { "AVB ORANGE" } else { "AVB GREEN" };
            let _ = display.status("ANDROID", "SYSTEM READY", session.image.slot.as_str(), avb_state);
        }
        Ok(code) => {
            println!("runtime returned failure code={code}");
            let _ = display.error("RUNTIME", "NONZERO CODE");
            enter_fastboot(display, desktop_input, &mut keys, &delay);
        }
        Err(err) => {
            println!("runtime error: {:?}", err);
            let _ = display.error("RUNTIME", "SCRIPT ERROR");
            enter_fastboot(display, desktop_input, &mut keys, &delay);
        }
    }

    // The real board remains alive. A viewer can connect after boot, request
    // SYNC, or ask the bootloader to enter Fastboot+ without any physical keys.
    loop {
        if let Some(event) = desktop_input.poll() {
            match event {
                DesktopEvent::Sync => {
                    display.hello();
                    let _ = display.status("ANDROID", "SYSTEM READY", session.image.slot.as_str(), if session.image.avb_unsigned { "AVB ORANGE" } else { "AVB GREEN" });
                }
                DesktopEvent::Ping => display.pong(),
                DesktopEvent::Fastboot => {
                    enter_fastboot(display, desktop_input, &mut keys, &delay);
                }
                DesktopEvent::Recovery => {
                    let mode = recovery_menu(&mut display, &mut keys, &mut desktop_input, &delay);
                    if mode == BootMode::Fastboot {
                        enter_fastboot(display, desktop_input, &mut keys, &delay);
                    }
                    let _ = display.status("ANDROID", "SYSTEM READY", session.image.slot.as_str(), if session.image.avb_unsigned { "AVB ORANGE" } else { "AVB GREEN" });
                }
                DesktopEvent::Power => system::software_reset(),
                DesktopEvent::Normal | DesktopEvent::Rotate(_) => {}
            }
        }
        delay.delay_millis(20);
    }
}

fn enter_fastboot<'d, D: BootDisplay>(
    mut display: DesktopMirror<'d, D>,
    input: DesktopInput<'d>,
    keys: &mut BootKeys<'_>,
    delay: &Delay,
) -> ! {
    // Send the Fastboot screen through the desktop mirror before handing the
    // CDC TX/RX halves to the Fastboot+ command loop.
    let _ = display.fastboot();
    let (mut physical_display, tx) = display.into_parts();
    fastboot::run(input.into_inner(), tx, &mut physical_display, keys, delay)
}

fn sample_boot_mode<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
    window_ms: u32,
) -> BootMode {
    let ticks = (window_ms / 10).max(1);
    for _ in 0..ticks {
        if keys.power_pressed() { return BootMode::Fastboot; }
        if keys.poll_rotation() != 0 { return BootMode::Recovery; }

        if let Some(event) = desktop.poll() {
            match event {
                DesktopEvent::Sync => {
                    display.hello();
                    let _ = display.boot_logo();
                }
                DesktopEvent::Ping => display.pong(),
                DesktopEvent::Fastboot | DesktopEvent::Power => return BootMode::Fastboot,
                DesktopEvent::Recovery | DesktopEvent::Rotate(_) => return BootMode::Recovery,
                DesktopEvent::Normal => return BootMode::Normal,
            }
        }
        delay.delay_millis(10);
    }
    BootMode::Normal
}

fn recovery_menu<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
) -> BootMode {
    let mut selected_fastboot = false;
    let mut last_power = keys.power_pressed();
    loop {
        if selected_fastboot {
            let _ = display.status("RECOVERY", "> FASTBOOT", "  NORMAL BOOT", "PRESS TO SELECT");
        } else {
            let _ = display.status("RECOVERY", "> NORMAL BOOT", "  FASTBOOT", "PRESS TO SELECT");
        }

        for _ in 0..20 {
            if keys.poll_rotation() != 0 {
                selected_fastboot = !selected_fastboot;
                break;
            }

            if let Some(event) = desktop.poll() {
                match event {
                    DesktopEvent::Sync => {
                        display.hello();
                        break;
                    }
                    DesktopEvent::Ping => display.pong(),
                    DesktopEvent::Rotate(_) => {
                        selected_fastboot = !selected_fastboot;
                        break;
                    }
                    DesktopEvent::Power => {
                        return if selected_fastboot { BootMode::Fastboot } else { BootMode::Normal };
                    }
                    DesktopEvent::Fastboot => return BootMode::Fastboot,
                    DesktopEvent::Normal => return BootMode::Normal,
                    DesktopEvent::Recovery => {}
                }
            }

            let power = keys.power_pressed();
            if power && !last_power {
                return if selected_fastboot { BootMode::Fastboot } else { BootMode::Normal };
            }
            last_power = power;
            delay.delay_millis(20);
        }
    }
}
