use esp_hal::{
    Blocking,
    delay::Delay,
    system,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

use crate::{display::BootDisplay, input::BootKeys};

pub fn run<D: BootDisplay>(
    mut rx: UsbSerialJtagRx<'_, Blocking>,
    mut tx: UsbSerialJtagTx<'_, Blocking>,
    display: &mut D,
    keys: &mut BootKeys<'_>,
    delay: &Delay,
) -> !
{
    let _ = display.fastboot();
    write_hello(&mut tx);
    write_ui_fastboot(&mut tx);
    write_line(&mut tx, "INFOZephyr Watch fastboot+ ready");
    write_line(&mut tx, "OKAY");

    let mut line = [0u8; 128];
    let mut len = 0usize;
    let mut last_power = keys.power_pressed();

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
                            reboot(&mut tx, "desktop requested normal boot", delay);
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

        let power = keys.power_pressed();
        if power && !last_power {
            write_line(&mut tx, "INFOreboot requested by button");
            write_line(&mut tx, "OKAY");
            delay.delay_millis(80);
            system::software_reset();
        }
        last_power = power;
        delay.delay_millis(10);
    }
}

fn handle_command(tx: &mut UsbSerialJtagTx<'_, Blocking>, cmd: &str, delay: &Delay) {
    match cmd {
        "help" => {
            for s in [
                "INFOcommands: devices getvar:all getvar:version getvar:product",
                "INFOgetvar:slot-count getvar:has-slot:boot/init_boot/vendor_boot/vbmeta",
                "INFOpins memory continue reboot reboot-bootloader",
                "INFOdesktop: @ZWIN|SYNC @ZWIN|PING @ZWIN|POWER",
            ] {
                write_line(tx, s);
            }
            write_line(tx, "OKAY");
        }
        "devices" => write_line(tx, "OKAYZW-ESP32S3\tfastboot"),
        "getvar:version" => write_line(tx, concat!("OKAY", env!("CARGO_PKG_VERSION"))),
        "getvar:product" => write_line(tx, "OKAYZephyr Watch"),
        "getvar:slot-count" => write_line(tx, "OKAY2"),
        "getvar:has-slot:boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:init_boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:vendor_boot" => write_line(tx, "OKAYyes"),
        "getvar:has-slot:vbmeta" => write_line(tx, "OKAYyes"),
        "getvar:super-partition-name" => write_line(tx, "OKAYsuper"),
        "getvar:all" => {
            for s in [
                concat!("INFOversion:", env!("CARGO_PKG_VERSION")),
                "INFOproduct:Zephyr Watch",
                "INFOboard:ESP32-S3-Zero-N4R2",
                "INFOsecure:no",
                "INFOslots:a,b",
                "INFOslot-count:2",
                "INFOstorage:gpt-microsd",
                "INFOboot-partitions:boot,init_boot,vendor_boot,vbmeta",
                "INFOdynamic-partitions:super",
                "INFOavb:avb0-sha256,unsigned-development-orange",
                "INFOflash:partition-write-not-yet-exposed-over-fastboot+",
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
            write_line(tx, "INFOusb:type-c native usb-serial-jtag gpio19/20 internal");
            write_line(tx, "INFOsd:miso=7,clk=8,mosi=9,cs=10");
            write_line(tx, "INFOinput:a=11,b=12,power=13");
            write_line(tx, "INFOstatus-led=21");
            write_line(tx, "OKAY");
        }
        "memory" => {
            write_line(tx, "INFOheap:esp-alloc 96KiB internal + N4R2 PSRAM");
            write_line(tx, "INFOdesktop:command-mirror, no full framebuffer required");
            write_line(tx, "INFOpsram:available to later system runtime");
            write_line(tx, "OKAY");
        }
        "continue" => reboot(tx, "release power; continuing via clean reset", delay),
        "reboot" => reboot(tx, "release power; rebooting", delay),
        "reboot-bootloader" => reboot(tx, "hold power through reset to re-enter fastboot", delay),
        _ => write_line(tx, "FAILunknown command"),
    }
}

fn write_hello(tx: &mut UsbSerialJtagTx<'_, Blocking>) {
    #[cfg(feature = "display-st7789")]
    write_line(tx, "@ZWUI|HELLO|2|240|280|Zephyr Watch|ESP32-S3-Zero-N4R2");
    #[cfg(feature = "display-ssd1306")]
    write_line(tx, "@ZWUI|HELLO|2|128|64|Zephyr Watch|ESP32-S3-Zero-N4R2");
}

fn write_ui_fastboot(tx: &mut UsbSerialJtagTx<'_, Blocking>) {
    write_line(tx, "@ZWUI|STATUS|FASTBOOT MODE|USB: READY|PWR: REBOOT|CMD: HELP");
}

fn reboot(tx: &mut UsbSerialJtagTx<'_, Blocking>, reason: &str, delay: &Delay) -> ! {
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
    system::software_reset();
}

fn write_line(tx: &mut UsbSerialJtagTx<'_, Blocking>, text: &str) {
    let _ = tx.write(text.as_bytes());
    let _ = tx.write(b"\r\n");
    let _ = tx.flush_tx();
}

#[cfg(feature = "display-ssd1306")]
fn display_info() -> &'static str { "INFOdisplay:ssd1306-128x64" }
#[cfg(feature = "display-st7789")]
fn display_info() -> &'static str { "INFOdisplay:st7789v3-240x280-rgb565" }

#[cfg(feature = "display-ssd1306")]
fn display_pins() -> &'static str { "INFOdisplay:ssd1306,sda=5,scl=6" }
#[cfg(feature = "display-st7789")]
fn display_pins() -> &'static str { "INFOdisplay:st7789v3,bl=1,cs=2,dc=3,rst=4,mosi=5,sck=6" }
