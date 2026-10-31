//! Board initialization and system handoff.
//! See persistence.rs, radio_bridge.rs and boot_modes.rs for services.
use crate::*;
use crate::persistence::*;
use crate::state_flow::*;
#[cfg(feature = "radio-services")]
use crate::radio_bridge::*;
#[cfg(not(feature = "pc-block-boot"))]
use crate::boot_modes::*;

pub(crate) fn run() -> ! {
    // Keep a small internal heap for latency-sensitive allocations, then add
    // the N4R2 board's 2 MiB PSRAM as a second global allocator region.
    // Rhai's parser/AST is allocation-heavy and the previous 96 KiB-only
    // heap exhausted while compiling the ~8 KiB AOSP userspace bundle.
    esp_alloc::heap_allocator!(size: 96 * 1024);
    let hal_cfg = esp_hal::Config::default()
        .with_cpu_clock(CpuClock::max())
        .with_psram(esp_hal::psram::PsramConfig::default());
    let p = esp_hal::init(hal_cfg);
    println!("performance: ESP32-S3 CPU=240MHz (CpuClock::max); USB-PC/radio profile active");

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
    println!("device={} manufacturer={} board={} target=esp32s3", config::DEVICE, config::MANUFACTURER, config::MODEL);

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
        .with_sck(p.GPIO1)
        .with_mosi(p.GPIO2);
        let cs = Output::new(p.GPIO5, Level::High, OutputConfig::default());
        let dc = Output::new(p.GPIO4, Level::Low, OutputConfig::default());
        let rst = Output::new(p.GPIO3, Level::High, OutputConfig::default());
        let bl = Output::new(p.GPIO6, Level::Low, OutputConfig::default());
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
        let mut pager_used: u32 = 0;
        if let Some(p) = pager {
            println!("zpager: {} slots / {} KiB managed backing store", p.slots(), p.capacity_bytes() / 1024);
        } else {
            println!("zpager: swap partition unavailable");
        }

        // ESP32-S3 has no battery-backed wall-clock RTC.  Keep the last known
        // Wear time as a holdover value so the watch face is still useful
        // immediately after a reboot; a paired-phone/manual update remains the
        // authoritative source and will replace it when available.
        if let Some(pager) = pager.as_ref() {
            if let Some(config) = load_rtc_config(pager, session.block_device()) {
                rtc::restore_config(config);
                println!("time: restored software RTC settings");
            }
            if let Some(minutes) = load_time_holdover(pager, session.block_device()) {
                rtc::restore_holdover_minutes(minutes);
                println!("time: restored last-known wall clock {:02}:{:02}", minutes / 60, minutes % 60);
            }
            if let Some(saved) = load_system_settings(pager, session.block_device()) {
                let restored = (os.state() & !SYSTEM_SETTINGS_MASK) | (saved & SYSTEM_SETTINGS_MASK);
                if let Ok(frame) = os.replace_state(restored) { first_frame = frame; }
                println!("settings: restored persistent Wear settings");
            }
            if let Ok(frame) = os.render() { first_frame = frame; }
        }

        #[cfg(feature = "radio-services")]
        if let (Some(radios), Some(pager)) = (radios.as_mut(), pager.as_ref()) {
            if let Some(credentials) = load_wifi_credentials(pager, session.block_device()) {
                println!("radio: restored saved Wi-Fi profile");
                radios.set_saved_wifi(credentials);
                // Migrate the legacy slot-16 record to the reserved metadata
                // range. This is idempotent and prevents Activity paging from
                // ever overwriting credentials again.
                let _ = save_wifi_credentials(pager, session.block_device(), credentials);
            }
        }
        let mut persisted_settings = os.state() & SYSTEM_SETTINGS_MASK;
        let mut persisted_time = rtc::snapshot_minutes();
        let mut persisted_rtc_config = rtc::config_snapshot();
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
        let mut ui_refresh_ticks: u8 = 0;

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
                            ((os.state() >> 21) & 1),
                            ((os.state() >> 22) & 1),
                            ((os.state() >> 23) & 1),
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
                                let _ = set_runtime_flag(&mut os, 21, true).map(|frame| {
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
                                if let Ok(frame) = set_runtime_flag(&mut os, 21, true) {
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
                            if let Ok(frame) = set_runtime_flag(&mut os, 21, false) {
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
                                if let Ok(frame) = set_runtime_flag(&mut os, 22, true) {
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
                                if let Ok(frame) = set_runtime_flag(&mut os, 22, true) {
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
                            if let Ok(frame) = set_runtime_flag(&mut os, 22, false) {
                                let _ = display.runtime(&frame);
                                session.block_device().viewer_runtime(&frame);
                            }
                            session.block_device().viewer_adb_output("bt", if ok { "disconnect requested" } else { "not connected" });
                        }
                        None
                    }
                    DesktopEvent::CompanionTime(minutes) => {
                        rtc::set_companion_minutes(minutes);
                        session.block_device().viewer_adb_output("time", "companion time accepted");
                        Some(runtime::RuntimeEvent::Sync)
                    }
                    DesktopEvent::Swipe(direction) => Some(runtime::RuntimeEvent::Swipe(direction as i32)),
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
            ui_refresh_ticks = ui_refresh_ticks.wrapping_add(1);
            if ui_refresh_ticks >= 50 {
                ui_refresh_ticks = 0;
                let screen = (os.state() & 0x1f) as u8;
                if matches!(screen, 0 | 1 | 6 | 10 | 11) {
                    if let Ok(mut frame) = os.render() {
                        frame.set_swap_pages(pager_used.count_ones().min(255) as u8);
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                        let _ = display.runtime(&frame);
                        session.block_device().viewer_runtime(&frame);
                    }
                }
            }
            #[cfg(feature = "radio-services")]
            if let Some(radios) = radios.as_mut() {
                if let Some(mut frame) = service_radio_runtime(radios, &mut os, pager.as_ref(), session.block_device(), &mut pager_used) {
                    frame.set_swap_pages(pager_used.count_ones().min(255) as u8);
                    radios.sync_frame(&frame);
                    let _ = display.runtime(&frame);
                    session.block_device().viewer_runtime(&frame);
                }
            }

            if let Some(pager) = pager.as_ref() {
                let settings = os.state() & SYSTEM_SETTINGS_MASK;
                if settings != persisted_settings && save_system_settings(pager, session.block_device(), os.state()) {
                    persisted_settings = settings;
                    println!("settings: persistent state updated");
                }
                if let Some(now) = rtc::snapshot_minutes() {
                    let changed = persisted_time != Some(now);
                    let jumped = persisted_time.map(|old| ((now as i32 - old as i32).rem_euclid(1440)) > 1).unwrap_or(true);
                    if changed && (jumped || now % 5 == 0) && save_time_holdover(pager, session.block_device(), now) {
                        persisted_time = Some(now);
                    }
                }
                let rtc_config = rtc::config_snapshot();
                if rtc_config != persisted_rtc_config && save_rtc_config(pager, session.block_device(), rtc_config) {
                    persisted_rtc_config = rtc_config;
                    println!("time: software RTC settings updated");
                }
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
        if let Err(err) = spi.write(&[0xFF; 10]) { // 80 idle clocks, CS high.
            println!("microSD: idle clock transfer failed: {:?}", err);
            let _ = display.error("SD SPI ERROR", "CHECK GPIO7-10");
            enter_fastboot(display, desktop_input, &mut keys, &delay);
        }
        println!("microSD: SPI2 MISO=7 SCK=8 MOSI=9 CS=10");
        let device = ExclusiveDevice::new(spi, cs, delay).expect("SD CS pin");

        let _ = display.boot_progress("LOADING", 2);
        let mut session = match android_storage::load_system(device, delay, |card| {
            // embedded-sdmmc initializes the card while still at <=400 kHz. Once
            // initialization has completed, re-clock the SPI bus for image/GPT I/O.
            card.spi(|sd_device| {
                let fast = SpiConfig::default()
                    .with_frequency(Rate::from_mhz(config::SD_DATA_MHZ))
                    .with_mode(Mode::_0);
                match sd_device.bus_mut().apply_config(&fast) {
                    Ok(()) => println!("microSD: switched to {} MHz", config::SD_DATA_MHZ),
                    Err(err) => println!("microSD: staying at init speed; switch failed: {:?}", err),
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
        let mut pager_used: u32 = 0;
        if let Some(p) = pager {
            println!("zpager: {} slots / {} KiB managed backing store", p.slots(), p.capacity_bytes() / 1024);
        } else {
            println!("zpager: swap partition unavailable");
        }

        // ESP32-S3 has no battery-backed wall-clock RTC.  Keep the last known
        // Wear time as a holdover value so the watch face is still useful
        // immediately after a reboot; a paired-phone/manual update remains the
        // authoritative source and will replace it when available.
        if let Some(pager) = pager.as_ref() {
            if let Some(config) = load_rtc_config(pager, session.block_device()) {
                rtc::restore_config(config);
                println!("time: restored software RTC settings");
            }
            if let Some(minutes) = load_time_holdover(pager, session.block_device()) {
                rtc::restore_holdover_minutes(minutes);
                println!("time: restored last-known wall clock {:02}:{:02}", minutes / 60, minutes % 60);
            }
            if let Some(saved) = load_system_settings(pager, session.block_device()) {
                let restored = (os.state() & !SYSTEM_SETTINGS_MASK) | (saved & SYSTEM_SETTINGS_MASK);
                if let Ok(frame) = os.replace_state(restored) { first_frame = frame; }
                println!("settings: restored persistent Wear settings");
            }
            if let Ok(frame) = os.render() { first_frame = frame; }
        }

        #[cfg(feature = "radio-services")]
        if let (Some(radios), Some(pager)) = (radios.as_mut(), pager.as_ref()) {
            if let Some(credentials) = load_wifi_credentials(pager, session.block_device()) {
                println!("radio: restored saved Wi-Fi profile");
                radios.set_saved_wifi(credentials);
                // Migrate the legacy slot-16 record to the reserved metadata
                // range. This is idempotent and prevents Activity paging from
                // ever overwriting credentials again.
                let _ = save_wifi_credentials(pager, session.block_device(), credentials);
            }
        }
        let mut persisted_settings = os.state() & SYSTEM_SETTINGS_MASK;
        let mut persisted_time = rtc::snapshot_minutes();
        let mut persisted_rtc_config = rtc::config_snapshot();
        first_frame.set_swap_pages(0);
        #[cfg(feature = "radio-services")]
        if let Some(radios) = radios.as_mut() { radios.sync_frame(&first_frame); }
        println!("system_server handoff succeeded");
        if let Err(err) = session.mark_successful() {
            println!("warning: failed to mark slot successful: {:?}", err);
        }
        let _ = display.runtime(&first_frame);

        let mut last_power = keys.power_pressed();
        let mut power_hold_ticks: u16 = 0;
        let mut ui_refresh_ticks: u8 = 0;
        loop {
            let rotation = keys.poll_rotation();
            if rotation != 0 {
                if let Ok(frame) = dispatch_with_pager(&mut os, runtime::RuntimeEvent::Rotate(rotation as i32), pager.as_ref(), session.block_device(), &mut pager_used) {
                    #[cfg(feature = "radio-services")]
                    if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                    let _ = display.runtime(&frame);
                }
            }
            // Encoder-only hardware contract: short press selects, a normal
            // long press goes Back, and a very long press returns Home.  The
            // decision is made on release so a long hold cannot accidentally
            // trigger both Back and Home while the button is still down.
            let power = keys.power_pressed();
            if power {
                power_hold_ticks = power_hold_ticks.saturating_add(1);
            } else if last_power {
                let action = if power_hold_ticks >= 90 {
                    runtime::RuntimeEvent::Home       // ~1.8 s
                } else if power_hold_ticks >= 35 {
                    runtime::RuntimeEvent::Back       // ~0.7 s
                } else {
                    runtime::RuntimeEvent::Press
                };
                if let Ok(frame) = dispatch_with_pager(&mut os, action, pager.as_ref(), session.block_device(), &mut pager_used) {
                    #[cfg(feature = "radio-services")]
                    if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                    let _ = display.runtime(&frame);
                }
                power_hold_ticks = 0;
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
                                let _ = set_runtime_flag(&mut os, 21, true);
                            }
                        }
                        None
                    }
                    DesktopEvent::WifiDisconnect => {
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { radios.disconnect_wifi(); }
                        let _ = set_runtime_flag(&mut os, 21, false);
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
                        let _ = set_runtime_flag(&mut os, 22, false);
                        None
                    }
                    DesktopEvent::CompanionTime(minutes) => {
                        rtc::set_companion_minutes(minutes);
                        Some(runtime::RuntimeEvent::Sync)
                    }
                    DesktopEvent::Swipe(direction) => Some(runtime::RuntimeEvent::Swipe(direction as i32)),
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
            ui_refresh_ticks = ui_refresh_ticks.wrapping_add(1);
            if ui_refresh_ticks >= 50 {
                ui_refresh_ticks = 0;
                let screen = (os.state() & 0x1f) as u8;
                if matches!(screen, 0 | 1 | 6 | 10 | 11) {
                    if let Ok(mut frame) = os.render() {
                        frame.set_swap_pages(pager_used.count_ones().min(255) as u8);
                        #[cfg(feature = "radio-services")]
                        if let Some(radios) = radios.as_mut() { radios.sync_frame(&frame); }
                        let _ = display.runtime(&frame);
                    }
                }
            }
            #[cfg(feature = "radio-services")]
            if let Some(radios) = radios.as_mut() {
                if let Some(mut frame) = service_radio_runtime(radios, &mut os, pager.as_ref(), session.block_device(), &mut pager_used) {
                    frame.set_swap_pages(pager_used.count_ones().min(255) as u8);
                    radios.sync_frame(&frame);
                    let _ = display.runtime(&frame);
                }
            }

            if let Some(pager) = pager.as_ref() {
                let settings = os.state() & SYSTEM_SETTINGS_MASK;
                if settings != persisted_settings && save_system_settings(pager, session.block_device(), os.state()) {
                    persisted_settings = settings;
                    println!("settings: persistent state updated");
                }
                if let Some(now) = rtc::snapshot_minutes() {
                    let changed = persisted_time != Some(now);
                    let jumped = persisted_time.map(|old| ((now as i32 - old as i32).rem_euclid(1440)) > 1).unwrap_or(true);
                    if changed && (jumped || now % 5 == 0) && save_time_holdover(pager, session.block_device(), now) {
                        persisted_time = Some(now);
                    }
                }
                let rtc_config = rtc::config_snapshot();
                if rtc_config != persisted_rtc_config && save_rtc_config(pager, session.block_device(), rtc_config) {
                    persisted_rtc_config = rtc_config;
                    println!("time: software RTC settings updated");
                }
            }
            delay.delay_millis(20);
        }
    }
}
