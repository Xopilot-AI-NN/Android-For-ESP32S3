// Модуль команд режима загрузчика.
use esp_hal::{
    Blocking,
    delay::Delay,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

use crate::{display::BootDisplay, input::BootKeys};

/// Запускает основной цикл компонента и обслуживает его события.
pub fn run<D: BootDisplay>(
    mut rx: UsbSerialJtagRx<'_, Blocking>,
    mut tx: UsbSerialJtagTx<'_, Blocking>,
    display: &mut D,
    keys: &mut BootKeys<'_>,
    delay: &Delay,
) -> ! {
    keys.consume_boot_press();
    let mut selected = 0u8;
    let _ = display.boot_menu(1, selected);
    write_hello(&mut tx);
    write_ui_fastboot(&mut tx);
    write_line(&mut tx, "INFOAOSP Wear OS fastboot+ ready");
    write_line(&mut tx, "OKAY");

    let mut line = [0u8; 128];
    let mut len = 0usize;

    loop {
        let mut buf = [0u8; 64];
        let n = rx.drain_rx_fifo(&mut buf);
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                if len != 0 {
                    if let Ok(cmd) = core::str::from_utf8(&line[..len]) {
                        let cmd = cmd.trim();
                        if cmd == "@ZWIN|SYNC" || cmd == "@ZWIN|FASTBOOT" {
                            write_hello(&mut tx);
                            write_ui_fastboot(&mut tx);
                        } else if cmd == "@ZWIN|PING" {
                            write_line(&mut tx, "@ZWUI|PONG|2");
                        } else if cmd == "@ZWIN|POWER" || cmd == "@ZWIN|NORMAL" {
                            activate(selected);
                        } else {
                            handle_command(&mut tx, cmd, delay);
                        }
                    } else {
                        write_line(&mut tx, "FAILinvalid utf8");
                    }
                    len = 0;
                }
            } else if len < line.len() {
                line[len] = b;
                len += 1;
            } else {
                len = 0;
                write_line(&mut tx, "FAILcommand too long");
            }
        }

        let rotation = keys.poll_rotation();
        if rotation != 0 {
            selected = (i32::from(selected) + i32::from(rotation)).rem_euclid(4) as u8;
            let _ = display.boot_menu(1, selected);
        }
        if let Some(event) = keys.poll_button_event() {
            match event {
                crate::runtime::RuntimeEvent::Press => activate(selected),
                crate::runtime::RuntimeEvent::Home | crate::runtime::RuntimeEvent::Back => {
                    activate(0)
                }
                _ => {}
            }
        }
        delay.delay_millis(10);
    }
}

/// Разбирает команду fastboot и выполняет разрешённое действие.
fn handle_command(tx: &mut UsbSerialJtagTx<'_, Blocking>, cmd: &str, delay: &Delay) {
    match cmd {
        "reboot-recovery" => reboot(tx, "entering recovery", delay, crate::runtime::SystemAction::Recovery),
        "poweroff" => reboot(tx, "powering off", delay, crate::runtime::SystemAction::PowerOff),
        "help" => {
            for s in [
                "INFOcommands: devices getvar:all getvar:version getvar:product",
                "INFOgetvar:slot-count getvar:has-slot:boot/init_boot/vendor_boot/vbmeta",
                "INFOpins memory continue reboot reboot-bootloader reboot-recovery poweroff",
                "INFOhost update: fastboot_tool.py update path/to/firmware-ELF",
                "INFOdesktop: @ZWIN|SYNC @ZWIN|PING @ZWIN|POWER",
            ] {
                write_line(tx, s);
            }
            write_line(tx, "OKAY");
        }
        "devices" => write_line(tx, "OKAYZW-ESP32S3\tfastboot"),
        "getvar:version" => write_line(tx, concat!("OKAY", env!("CARGO_PKG_VERSION"))),
        "getvar:product" => write_line(tx, "OKAYAOSP Wear OS"),
        "getvar:slot-count" => write_line(tx, "OKAY2"),
        "getvar:has-slot:boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:init_boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:vendor_boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:vbmeta" => write_line(tx, "OKAYyes"),
        "getvar:super-partition-name" => write_line(tx, "OKAYsuper"),
        "getvar:all" => {
            for s in [
                concat!("INFOversion:", env!("CARGO_PKG_VERSION")),
                "INFOproduct:AOSP Wear OS",
                "INFOboard:ESP32-S3-Zero-N4R2",
                "INFOsecure:no",
                "INFOslots:a,b",
                "INFOslot-count:2",
                if cfg!(feature = "pc-block-boot") { "INFOstorage:gpt-pc-block" } else { "INFOstorage:gpt-microsd" },
                "INFOboot-partitions:boot,init_boot,vendor_boot,vbmeta",
                "INFOdynamic-partitions:super",
                "INFOavb:avb0-sha256,unsigned-development-orange",
                "INFOflash:internal ELF via fastboot_tool.py update (espflash)",
                "INFOpartitions:PC image via tools/partition_tool.py; stop block server first",
                display_info(),
                "INFOdesktop:usb-serial-jtag-live-surface-v2",
                "INFOruntime:rhai",
            ] {
                write_line(tx, s);
            }
            write_line(tx, "OKAY");
        }
        "pins" => {
            write_line(tx, display_pins());
            write_line(
                tx,
                "INFOusb:type-c native usb-serial-jtag gpio19/20 internal",
            );
            write_line(tx, "INFOsd:miso=7,clk=8,mosi=9,cs=10");
            write_line(tx, "INFOinput:a=11,b=12,power=13");
            write_line(tx, "INFOstatus-led=21");
            write_line(tx, "OKAY");
        }
        "memory" => {
            write_line(tx, "INFOheap:esp-alloc 96KiB internal + N4R2 PSRAM");
            write_line(
                tx,
                "INFOdesktop:command-mirror, no full framebuffer required",
            );
            write_line(tx, "INFOpsram:available to later system runtime");
            write_line(tx, "OKAY");
        }
        "continue" => reboot(tx, "release power; continuing via clean reset", delay, crate::runtime::SystemAction::Reboot),
        "reboot" => reboot(tx, "release power; rebooting", delay, crate::runtime::SystemAction::Reboot),
        "reboot-bootloader" => reboot(tx, "entering fastboot", delay, crate::runtime::SystemAction::Fastboot),
        _ => write_line(tx, "FAILunknown command"),
    }
}

/// Передаёт приветствие отладочного протокола.
fn write_hello(tx: &mut UsbSerialJtagTx<'_, Blocking>) {
    #[cfg(feature = "display-st7789")]
    write_line(tx, "@ZWUI|HELLO|2|240|280|AOSP Wear OS|ESP32-S3-Zero-N4R2");
    #[cfg(feature = "display-ssd1306")]
    write_line(tx, "@ZWUI|HELLO|2|128|64|AOSP Wear OS|ESP32-S3-Zero-N4R2");
}

/// Передаёт состояние fastboot настольному интерфейсу.
fn write_ui_fastboot(tx: &mut UsbSerialJtagTx<'_, Blocking>) {
    write_line(
        tx,
        "@ZWUI|STATUS|FASTBOOT MODE|USB: READY|PWR: REBOOT|CMD: HELP",
    );
}

/// Запрашивает перезагрузку в выбранный режим.
fn reboot(tx: &mut UsbSerialJtagTx<'_, Blocking>, reason: &str, delay: &Delay, mode: crate::runtime::SystemAction) -> ! {
    write_line(tx, "INFOreset requested");
    let mut msg = [0u8; 64];
    let prefix = b"OKAY";
    msg[..prefix.len()].copy_from_slice(prefix);
    let bytes = reason.as_bytes();
    let n = bytes.len().min(msg.len() - prefix.len());
    msg[prefix.len()..prefix.len() + n].copy_from_slice(&bytes[..n]);
    let _ = tx.write(&msg[..prefix.len() + n]);
    let _ = tx.write(b"\r\n");
    let _ = tx.flush_tx();
    delay.delay_millis(120);
    crate::power::request(mode);
}

/// Передаёт строку протокола с завершением строки.
fn write_line(tx: &mut UsbSerialJtagTx<'_, Blocking>, text: &str) {
    let _ = tx.write(text.as_bytes());
    let _ = tx.write(b"\r\n");
    let _ = tx.flush_tx();
}

#[cfg(feature = "display-ssd1306")]
/// Выводит сведения о системе на выбранном backend.
fn display_info() -> &'static str {
    "INFOdisplay:ssd1306-128x64"
}
#[cfg(feature = "display-st7789")]
/// Выводит сведения о системе на выбранном backend.
fn display_info() -> &'static str {
    "INFOdisplay:st7789v3-240x280-rgb565"
}

#[cfg(feature = "display-ssd1306")]
/// Показывает действующую распиновку для диагностики.
fn display_pins() -> &'static str {
    "INFOdisplay:ssd1306,sda=5,scl=6"
}
#[cfg(feature = "display-st7789")]
/// Показывает действующую распиновку для диагностики.
fn display_pins() -> &'static str {
    "INFOdisplay:st7789v3,sck=1,mosi=2,rst=3,dc=4,cs=5,bl=6"
}

/// Выбирает активный загрузочный слот.
fn activate(selected: u8) -> ! {
    use crate::runtime::SystemAction;
    crate::power::request(match selected {
        1 => SystemAction::Fastboot,
        2 => SystemAction::Recovery,
        3 => SystemAction::PowerOff,
        _ => SystemAction::Reboot,
    })
}
