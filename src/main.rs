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
mod fastboot;
mod font;
#[cfg(feature = "display-ssd1306")]
mod framebuffer;
mod input;
mod lp;
mod material;
#[cfg(feature = "radio-services")]
mod net;
mod page_store;
mod runtime;
#[cfg(feature = "radio-services")]
mod radio;
#[cfg(feature = "pc-block-boot")]
mod usb_pc_block;

use desktop::{DesktopEvent, DesktopInput, DesktopMirror, WifiCredentials};
#[cfg(not(feature = "pc-block-boot"))]
use desktop::DesktopSurface;
use display::BootDisplay;
#[cfg(feature = "display-ssd1306")]
use display::Ssd1306Display;
#[cfg(feature = "display-st7789")]
use display::St7789Display;
use input::{BootKeys, BootMode};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    // Keep a small internal heap for latency-sensitive allocations, then add
    // the N4R2 board's 2 MiB PSRAM as a second global allocator region.
    // Rhai's parser/AST is allocation-heavy and the previous 96 KiB-only
    // heap exhausted while compiling the ~8 KiB AOSP userspace bundle.
    esp_alloc::heap_allocator!(size: 96 * 1024);
    let hal_cfg = esp_hal::Config::default()
        .with_cpu_clock(CpuClock::max())
        .with_psram(esp_hal::psram::PsramConfig::default());
    let p = esp_hal::init(hal_cfg);

    let (_, psram_size) = esp_hal::psram::psram_raw_parts(&p.PSRAM);
    println!("PSRAM detected: {} bytes", psram_size);
    if psram_size != 0 {
        esp_alloc::psram_allocator!(p.PSRAM, esp_hal::psram);
    } else {
        println!("WARNING: PSRAM was not initialized; Rhai may run out of memory");
    }

    // Keep a second INTERNAL heap region exclusively useful to capability-
    // constrained allocations (esp-rtos task stacks, radio/DMA internals).
    // It is deliberately registered *after* PSRAM: ordinary GlobalAlloc
    // allocations spill from the primary 96 KiB heap into PSRAM first, while
    // esp_alloc::InternalMemory skips PSRAM and can use this reclaimed RAM.
    // This prevents Rhai/SystemUI from consuming the memory Wi-Fi needs later.
    #[cfg(feature = "radio-services")]
    {
        esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
        println!("radio: +64 KiB reclaimed internal heap reserved for RTOS stacks");
    }

    #[cfg(feature = "radio-services")]
    let mut radios = {
        // esp-radio's binary Wi-Fi/BLE drivers require the esp-rtos scheduler
        // to be running before esp_radio::init().  Xtensa ESP32-S3 needs only
        // the timer source; RISC-V chips additionally use a software interrupt.
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);
        radio::RadioServices::new(p.WIFI, p.BT)
    };

    let delay = Delay::new();

    println!("heap initialized:\n{}", esp_alloc::HEAP.stats());

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

    #[cfg(feature = "pc-block-boot")]
    {
        // Solder-free development mode: the same USB-C cable powers the board
        // and carries raw 512-byte disk requests to usb_block_server.py. This
        // deliberately uses USB-Serial-JTAG device mode, not USB Host/MSC.
        let usb = UsbSerialJtag::new(p.USB_DEVICE);
        let (usb_rx, usb_tx) = usb.split();
        let mut display = physical_display;
        let _ = display.boot_logo();
        delay.delay_millis(250);
        let _ = display.boot_progress("PC BLOCK DISK", 0);
        println!("storage=usb-pc-block waiting for usb_block_server.py");

        let device = usb_pc_block::UsbPcBlockDevice::new(usb_rx, usb_tx, delay);
        // Mirror boot state through the block server's local viewer bridge.
        // desktop_viewer.py never opens /dev/ttyACM0 in this mode.
        device.viewer_hello();
        device.viewer_boot();
        device.viewer_progress("PC BLOCK DISK", 0);

        let mut session = match android_storage::load_block_device(device) {
            Ok(session) => session,
            Err(err) => {
                println!("pc block android boot error: {:?}", err);
                let _ = display.error("PC DISK ERROR", "CHECK SERVER");
                loop { delay.delay_millis(1000); }
            }
        };

        println!(
            "slot={} version={} entry={} rollback={} avb={} storage=usb-pc-block",
            session.image.slot.as_str(),
            session.image.version,
            session.image.entry,
            session.image.rollback_index,
            if session.image.avb_unsigned { "orange/unsigned" } else { "green" },
        );
        let _ = display.boot_progress("MOUNTING SUPER", 4);
        session.block_device().viewer_progress("MOUNTING SUPER", 4);

        let bundle = match session.load_userspace_bundle() {
            Ok(bundle) => bundle,
            Err(err) => {
                println!("userspace load error: {:?}", err);
                let _ = display.error("SUPER ERROR", "USERSPACE");
                session.block_device().viewer_error("SUPER ERROR", "USERSPACE");
                loop { delay.delay_millis(1000); }
            }
        };
        println!("userspace bundle={} bytes", bundle.len());
        let _ = display.boot_progress("SYSTEM SERVER", 6);
        session.block_device().viewer_progress("SYSTEM SERVER", 6);

        println!("heap before Rhai compile:\n{}", esp_alloc::HEAP.stats());
        let (mut os, mut first_frame) = match runtime::Runtime::start(bundle) {
            Ok(v) => v,
            Err(err) => {
                println!("runtime start error: {:?}", err);
                let _ = display.error("RUNTIME", "SCRIPT ERROR");
                session.block_device().viewer_error("RUNTIME", "SCRIPT ERROR");
                loop { delay.delay_millis(1000); }
            }
        };

        println!("heap after Rhai startup:\n{}", esp_alloc::HEAP.stats());
        let pager = page_store::PageStore::open(session.block_device()).ok();
        let mut pager_used: u16 = 0;
        if let Some(p) = pager {
            println!("zpager: {} slots / {} KiB managed backing store", p.slots(), p.capacity_bytes() / 1024);
        } else {
            println!("zpager: swap partition unavailable");
        }
        #[cfg(feature = "radio-services")]
        if let (Some(radios), Some(pager)) = (radios.as_mut(), pager.as_ref()) {
            if let Some(credentials) = load_wifi_credentials(pager, session.block_device()) {
                println!("radio: restored saved Wi-Fi profile");
                radios.set_saved_wifi(credentials);
            }
        }
        first_frame.set_swap_pages(0);
        #[cfg(feature = "radio-services")]
        if let Some(radios) = radios.as_mut() { radios.sync_frame(&first_frame); }
        println!("system_server handoff succeeded");
        if let Err(err) = session.mark_successful() {
            println!("warning: failed to mark slot successful: {:?}", err);
        }
        let _ = display.runtime(&first_frame);
        session.block_device().viewer_runtime(&first_frame);

        // From this point the PC server may forward viewer commands. The
        // viewer talks to the block-server bridge and never reopens ttyACM0.
        session.block_device().viewer_interactive();

        loop {
            if let Some(event) = session.block_device().poll_desktop_event() {
                let runtime_event = match event {
                    DesktopEvent::Sync => {
                        session.block_device().viewer_hello();
                        Some(runtime::RuntimeEvent::Sync)
                    }
                    DesktopEvent::Ping => {
                        session.block_device().viewer_pong();
                        None
                    }
                    // PC-block mode keeps storage ownership, so these screens
                    // are informational instead of destructive reboots.
                    DesktopEvent::Fastboot => {
                        session.block_device().viewer_status(
                            "FASTBOOT", "PC BLOCK ACTIVE", "REBOOT MODE TODO", "NO RESET",
                        );
                        None
                    }
                    DesktopEvent::Recovery => {
                        session.block_device().viewer_status(
                            "RECOVERY", "PC BLOCK ACTIVE", "REBOOT MODE TODO", "NO RESET",
                        );
                        None
                    }
                    DesktopEvent::Power => Some(runtime::RuntimeEvent::Press),
                    DesktopEvent::Normal => Some(runtime::RuntimeEvent::Home),
                    DesktopEvent::AdbGetProp => {
                        session.block_device().viewer_adb_output(
                            "getprop",
                            adb::getprop(),
                        );
                        None
                    }
                    DesktopEvent::AdbServices => {
                        session.block_device().viewer_adb_output(
                            "services",
                            adb::services(),
                        );
                        None
                    }
                    DesktopEvent::AdbPackages => {
                        session.block_device().viewer_adb_output(
                            "packages",
                            adb::packages(),
                        );
                        None
                    }
                    DesktopEvent::AdbDumpsys => {
                        let mut line = heapless::String::<256>::new();
                        let _ = core::fmt::write(&mut line, format_args!(
                            "boot=completed slot={} pager_pages={} wifi={} bt={} wadb={}",
                            session.image.slot.as_str(),
                            pager_used.count_ones(),
                            ((os.state() >> 20) & 1),
                            ((os.state() >> 21) & 1),
                            ((os.state() >> 22) & 1),
                        ));
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_ref() {
                            let _ = core::fmt::write(&mut line, format_args!(" net={:?}", radios.network_state()));
                            if let Some(ip) = radios.ip_address() { let _ = core::fmt::write(&mut line, format_args!(" ip={}", ip)); }
                        }
                        session.block_device().viewer_adb_output("dumpsys", line.as_str());
                        None
                    }
                    DesktopEvent::WifiScan => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            let ok = radios.scan_wifi();
                            if ok {
                                let _ = set_runtime_flag(&mut os, 20, true).map(|frame| {
                                    let _ = display.runtime(&frame);
                                    session.block_device().viewer_runtime(&frame);
                                });
                            }
                            let mut line = heapless::String::<600>::new();
                            radios.format_wifi_scan(&mut line);
                            session.block_device().viewer_adb_output("wifi", line.as_str());
                        }
                        #[cfg(not(feature = "radio-services"))]
                        session.block_device().viewer_adb_output("wifi", "radio-services disabled");
                        None
                    }
                    DesktopEvent::WifiStatus => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_ref() {
                            let mut line = heapless::String::<256>::new();
                            radios.format_wifi_status(&mut line);
                            session.block_device().viewer_adb_output("wifi", line.as_str());
                        }
                        #[cfg(not(feature = "radio-services"))]
                        session.block_device().viewer_adb_output("wifi", "radio-services disabled");
                        None
                    }
                    DesktopEvent::WifiConnect(credentials) => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            let ok = radios.connect_wifi(credentials);
                            if ok {
                                if let Some(pager) = pager.as_ref() {
                                    if save_wifi_credentials(pager, session.block_device(), credentials) {
                                        println!("radio: saved Wi-Fi profile to ZPager");
                                    }
                                }
                                if let Ok(frame) = set_runtime_flag(&mut os, 20, true) {
                                    let _ = display.runtime(&frame);
                                    session.block_device().viewer_runtime(&frame);
                                }
                                session.block_device().viewer_adb_output("wifi", "association requested");
                            } else {
                                session.block_device().viewer_adb_output("wifi", "connect request failed");
                            }
                        }
                        None
                    }
                    DesktopEvent::WifiDisconnect => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            radios.disconnect_wifi();
                            if let Ok(frame) = set_runtime_flag(&mut os, 20, false) {
                                let _ = display.runtime(&frame);
                                session.block_device().viewer_runtime(&frame);
                            }
                            session.block_device().viewer_adb_output("wifi", "disconnected");
                        }
                        None
                    }
                    DesktopEvent::BtScan => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            let ok = radios.start_ble_scan();
                            if ok {
                                if let Ok(frame) = set_runtime_flag(&mut os, 21, true) {
                                    let _ = display.runtime(&frame);
                                    session.block_device().viewer_runtime(&frame);
                                }
                                session.block_device().viewer_adb_output("bt", "active scan started; wait 6 seconds then run bt-status");
                            } else {
                                session.block_device().viewer_adb_output("bt", "scan start failed");
                            }
                        }
                        None
                    }
                    DesktopEvent::BtStatus => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_ref() {
                            let mut line = heapless::String::<600>::new();
                            radios.format_bt_status(&mut line);
                            let _ = line.push_str("; ");
                            radios.format_bt_scan(&mut line);
                            session.block_device().viewer_adb_output("bt", line.as_str());
                        }
                        #[cfg(not(feature = "radio-services"))]
                        session.block_device().viewer_adb_output("bt", "radio-services disabled");
                        None
                    }
                    DesktopEvent::BtConnect(address) => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            if radios.connect_ble(address) {
                                if let Ok(frame) = set_runtime_flag(&mut os, 21, true) {
                                    let _ = display.runtime(&frame);
                                    session.block_device().viewer_runtime(&frame);
                                }
                                session.block_device().viewer_adb_output("bt", "connection requested");
                            } else {
                                session.block_device().viewer_adb_output("bt", "connection request failed");
                            }
                        }
                        None
                    }
                    DesktopEvent::BtDisconnect => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            let ok = radios.disconnect_ble();
                            session.block_device().viewer_adb_output("bt", if ok { "disconnect requested" } else { "not connected" });
                        }
                        None
                    }
                    DesktopEvent::Rotate(delta) => Some(runtime::RuntimeEvent::Rotate(delta as i32)),
                };

                if let Some(runtime_event) = runtime_event {
                    match dispatch_with_pager(&mut os, runtime_event, pager.as_ref(), session.block_device(), &mut pager_used) {
                        Ok(frame) => {
                            #[cfg(feature = "radio-services")]
                            if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                            let _ = display.runtime(&frame);
                            session.block_device().viewer_runtime(&frame);
                        }
                        Err(err) => {
                            println!("runtime event error: {:?}", err);
                            session.block_device().viewer_error("RUNTIME", "EVENT FAILED");
                        }
                    }
                }
            }
            #[cfg(feature = "radio-services")]
            if let Some(radios) = radios.as_mut() {
                radios.tick(((os.state() >> 22) & 1) != 0);
            }
            delay.delay_millis(20);
        }
    }

    #[cfg(not(feature = "pc-block-boot"))]
    {
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
        let _ = display.boot_progress("MOUNTING SUPER", 4);

        let bundle = match session.load_userspace_bundle() {
            Ok(bundle) => bundle,
            Err(err) => {
                println!("userspace load error: {:?}", err);
                let _ = display.error("SUPER ERROR", "USERSPACE");
                enter_fastboot(display, desktop_input, &mut keys, &delay);
            }
        };
        println!("userspace bundle={} bytes", bundle.len());
        let _ = display.boot_progress("SYSTEM SERVER", 6);

        println!("heap before Rhai compile:\n{}", esp_alloc::HEAP.stats());
        let (mut os, mut first_frame) = match runtime::Runtime::start(bundle) {
            Ok(v) => v,
            Err(err) => {
                println!("runtime start error: {:?}", err);
                let _ = display.error("RUNTIME", "SCRIPT ERROR");
                enter_fastboot(display, desktop_input, &mut keys, &delay);
            }
        };

        println!("heap after Rhai startup:\n{}", esp_alloc::HEAP.stats());
        let pager = page_store::PageStore::open(session.block_device()).ok();
        let mut pager_used: u16 = 0;
        if let Some(p) = pager {
            println!("zpager: {} slots / {} KiB managed backing store", p.slots(), p.capacity_bytes() / 1024);
        } else {
            println!("zpager: swap partition unavailable");
        }
        #[cfg(feature = "radio-services")]
        if let (Some(radios), Some(pager)) = (radios.as_mut(), pager.as_ref()) {
            if let Some(credentials) = load_wifi_credentials(pager, session.block_device()) {
                println!("radio: restored saved Wi-Fi profile");
                radios.set_saved_wifi(credentials);
            }
        }
        first_frame.set_swap_pages(0);
        #[cfg(feature = "radio-services")]
        if let Some(radios) = radios.as_mut() { radios.sync_frame(&first_frame); }
        println!("system_server handoff succeeded");
        if let Err(err) = session.mark_successful() {
            println!("warning: failed to mark slot successful: {:?}", err);
        }
        let _ = display.runtime(&first_frame);

        let mut last_power = keys.power_pressed();
        loop {
            let rotation = keys.poll_rotation();
            if rotation != 0 {
                if let Ok(frame) = dispatch_with_pager(&mut os, runtime::RuntimeEvent::Rotate(rotation as i32), pager.as_ref(), session.block_device(), &mut pager_used) {
                    #[cfg(feature = "radio-services")]
                    if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                    let _ = display.runtime(&frame);
                }
            }
            let power = keys.power_pressed();
            if power && !last_power {
                if let Ok(frame) = dispatch_with_pager(&mut os, runtime::RuntimeEvent::Press, pager.as_ref(), session.block_device(), &mut pager_used) {
                    #[cfg(feature = "radio-services")]
                    if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                    let _ = display.runtime(&frame);
                }
            }
            last_power = power;

            if let Some(event) = desktop_input.poll() {
                let runtime_event = match event {
                    DesktopEvent::Sync => {
                        display.hello();
                        Some(runtime::RuntimeEvent::Sync)
                    }
                    DesktopEvent::Ping => { display.pong(); None }
                    DesktopEvent::Fastboot => {
                        enter_fastboot(display, desktop_input, &mut keys, &delay);
                    }
                    DesktopEvent::Recovery => {
                        let mode = recovery_menu(&mut display, &mut keys, &mut desktop_input, &delay);
                        if mode == BootMode::Fastboot {
                            enter_fastboot(display, desktop_input, &mut keys, &delay);
                        }
                        Some(runtime::RuntimeEvent::Sync)
                    }
                    DesktopEvent::Power => Some(runtime::RuntimeEvent::Press),
                    DesktopEvent::Normal => Some(runtime::RuntimeEvent::Home),
                    DesktopEvent::AdbGetProp | DesktopEvent::AdbServices | DesktopEvent::AdbPackages | DesktopEvent::AdbDumpsys => {
                        println!("ZADB command received on direct viewer transport; use wireless adb or pc-block ZADB for output");
                        None
                    }
                    DesktopEvent::WifiScan => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { let _ = radios.scan_wifi(); }
                        None
                    }
                    DesktopEvent::WifiStatus => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_ref() {
                            let mut line = heapless::String::<256>::new();
                            radios.format_wifi_status(&mut line);
                            println!("wifi: {}", line);
                        }
                        None
                    }
                    DesktopEvent::WifiConnect(credentials) => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() {
                            if radios.connect_wifi(credentials) {
                                if let Some(pager) = pager.as_ref() { let _ = save_wifi_credentials(pager, session.block_device(), credentials); }
                                let _ = set_runtime_flag(&mut os, 20, true);
                            }
                        }
                        None
                    }
                    DesktopEvent::WifiDisconnect => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { radios.disconnect_wifi(); }
                        let _ = set_runtime_flag(&mut os, 20, false);
                        None
                    }
                    DesktopEvent::BtScan => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { let _ = radios.start_ble_scan(); }
                        None
                    }
                    DesktopEvent::BtStatus => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_ref() {
                            let mut line = heapless::String::<600>::new();
                            radios.format_bt_status(&mut line);
                            let _ = line.push_str("; ");
                            radios.format_bt_scan(&mut line);
                            println!("bt: {}", line);
                        }
                        None
                    }
                    DesktopEvent::BtConnect(address) => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { let _ = radios.connect_ble(address); }
                        None
                    }
                    DesktopEvent::BtDisconnect => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { let _ = radios.disconnect_ble(); }
                        None
                    }
                    DesktopEvent::Rotate(delta) => Some(runtime::RuntimeEvent::Rotate(delta as i32)),
                };
                if let Some(runtime_event) = runtime_event {
                    match dispatch_with_pager(&mut os, runtime_event, pager.as_ref(), session.block_device(), &mut pager_used) {
                        Ok(frame) => {
                            #[cfg(feature = "radio-services")]
                            if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                            let _ = display.runtime(&frame);
                        }
                        Err(err) => {
                            println!("runtime event error: {:?}", err);
                            let _ = display.error("RUNTIME", "EVENT FAILED");
                        }
                    }
                }
            }
            #[cfg(feature = "radio-services")]
            if let Some(radios) = radios.as_mut() {
                radios.tick(((os.state() >> 22) & 1) != 0);
            }
            delay.delay_millis(20);
        }
    }
}

const WIFI_CRED_SLOT: u32 = 16;
const WIFI_CRED_TAG: u32 = 0x5a57_4946; // "ZWIF"

fn load_wifi_credentials<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<WifiCredentials> {
    let mut raw = [0u8; 98];
    let n = pager.read(dev, WIFI_CRED_SLOT, WIFI_CRED_TAG, &mut raw).ok()?;
    WifiCredentials::decode_page(&raw[..n])
}

fn save_wifi_credentials<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    credentials: WifiCredentials,
) -> bool {
    let mut raw = [0u8; 98];
    let n = credentials.encode_page(&mut raw);
    pager.write(dev, WIFI_CRED_SLOT, WIFI_CRED_TAG, &raw[..n]).is_ok()
}

fn set_runtime_flag(
    os: &mut runtime::Runtime,
    bit: u32,
    enabled: bool,
) -> Result<runtime::RuntimeFrame, runtime::RuntimeError> {
    let mask = 1i32 << bit;
    let state = if enabled { os.state() | mask } else { os.state() & !mask };
    os.replace_state(state)
}

fn dispatch_with_pager<D: BlockDevice>(
    os: &mut runtime::Runtime,
    event: runtime::RuntimeEvent,
    pager: Option<&page_store::PageStore>,
    dev: &D,
    used: &mut u16,
) -> Result<runtime::RuntimeFrame, runtime::RuntimeError> {
    let old_state = os.state();
    let old_screen = (old_state & 0x0f) as u32;

    let mut frame = os.dispatch(event)?;
    let new_state = os.state();
    let new_screen = (new_state & 0x0f) as u32;
    if new_screen != old_screen {
        if let Some(pager) = pager {
            // Only evict an Activity when it actually leaves the foreground.
            // Crown movement inside one screen must not turn the SD/USB flash
            // into a write-every-20ms pseudo-swap device.
            let old_tag = 0x5a50_0000u32 | old_screen;
            if pager.write_i32(dev, old_screen, old_tag, old_state).is_ok() {
                if old_screen < 16 { *used |= 1u16 << old_screen; }
            }

            let new_tag = 0x5a50_0000u32 | new_screen;
            if let Ok(saved) = pager.read_i32(dev, new_screen, new_tag) {
                // Page-in only Activity-local cursor bits. Global settings and
                // connectivity flags stay from the current system state.
                let restored = (new_state & !0x00f0) | (saved & 0x00f0);
                if let Ok(restored_frame) = os.replace_state(restored) { frame = restored_frame; }
            }
        }
    }
    frame.set_swap_pages(used.count_ones().min(255) as u8);
    Ok(frame)
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
                DesktopEvent::AdbGetProp | DesktopEvent::AdbServices | DesktopEvent::AdbPackages | DesktopEvent::AdbDumpsys
                | DesktopEvent::WifiScan | DesktopEvent::WifiStatus | DesktopEvent::WifiConnect(_) | DesktopEvent::WifiDisconnect
                | DesktopEvent::BtScan | DesktopEvent::BtStatus | DesktopEvent::BtConnect(_) | DesktopEvent::BtDisconnect => {},
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
                    DesktopEvent::Recovery => {},
                    DesktopEvent::AdbGetProp | DesktopEvent::AdbServices | DesktopEvent::AdbPackages | DesktopEvent::AdbDumpsys
                | DesktopEvent::WifiScan | DesktopEvent::WifiStatus | DesktopEvent::WifiConnect(_) | DesktopEvent::WifiDisconnect
                | DesktopEvent::BtScan | DesktopEvent::BtStatus | DesktopEvent::BtConnect(_) | DesktopEvent::BtDisconnect => {},
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
