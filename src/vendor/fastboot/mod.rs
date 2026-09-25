use core::fmt::Write;

use esp_hal::{Blocking, delay::Delay, usb_serial_jtag::UsbSerialJtag};
use esp_println::println;

use crate::{
    components::{board::Board, boot::BootError},
    vendor::{led::RgbColor, ram::RAM},
    xtensa_lx7_cpu_to_gpu::XtensaLx7CpuToGpu,
};

// ── Command handlers ─────────────────────────────────────────────────────────

mod boot;
mod led;
mod logs;
mod memory;
mod pins;
mod reboot;
mod rtc;
mod status;

// ── Static command registry ───────────────────────────────────────────────────

static REGISTRY: &[CommandEntry] = &[
    CommandEntry { name: boot::CMD_NAME,   aliases: boot::CMD_ALIASES,   help: boot::CMD_HELP,   run: boot::run },
    CommandEntry { name: led::CMD_NAME,    aliases: led::CMD_ALIASES,    help: led::CMD_HELP,    run: led::run },
    CommandEntry { name: logs::CMD_NAME,   aliases: logs::CMD_ALIASES,   help: logs::CMD_HELP,   run: logs::run },
    CommandEntry { name: memory::CMD_NAME, aliases: memory::CMD_ALIASES, help: memory::CMD_HELP, run: memory::run },
    CommandEntry { name: pins::CMD_NAME,   aliases: pins::CMD_ALIASES,   help: pins::CMD_HELP,   run: pins::run },
    CommandEntry { name: reboot::CMD_NAME, aliases: reboot::CMD_ALIASES, help: reboot::CMD_HELP, run: reboot::run },
    CommandEntry { name: rtc::CMD_NAME,    aliases: rtc::CMD_ALIASES,    help: rtc::CMD_HELP,    run: rtc::run },
    CommandEntry { name: status::CMD_NAME, aliases: status::CMD_ALIASES, help: status::CMD_HELP, run: status::run },
];

// ── Types shared with command handlers ────────────────────────────────────────

#[allow(dead_code)]
pub type Arg = [u8; 32];

#[allow(dead_code)]
pub fn bytes_to_arg(bytes: &[u8]) -> Arg {
    let mut arr = [0u8; 32];
    let len = bytes.len().min(32);
    arr[..len].copy_from_slice(&bytes[..len]);
    arr
}

#[allow(dead_code)]
pub fn arg_str(arg: &Arg) -> &[u8] {
    let end = arg.iter().position(|&b| b == 0).unwrap_or(32);
    &arg[..end]
}

// ── FastbootExit ──────────────────────────────────────────────────────────────

pub enum FastbootExit {
    ContinueBoot,
    Reboot,
}

// ── LedMode ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum LedMode {
    Fastboot,
    Manual,
}

// ── Command context (passed to every handler) ─────────────────────────────────

pub struct CmdCtx<'r, 'd> {
    pub fb: &'r mut Fastboot<'d>,
    pub board: &'r mut Board<'d>,
    pub delay: &'r Delay,
    pub ram: &'r RAM,
    pub cpu_to_gpu: &'r XtensaLx7CpuToGpu,
    pub stream_logs: &'r mut bool,
    pub led_mode: &'r mut LedMode,
}

// ── CommandEntry ──────────────────────────────────────────────────────────────

pub type CmdFn = for<'r, 'd> fn(arg: &[u8], ctx: &mut CmdCtx<'r, 'd>) -> Option<FastbootExit>;

pub struct CommandEntry {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub help: &'static str,
    pub run: CmdFn,
}

impl CommandEntry {
    pub fn matches(&self, verb: &[u8]) -> bool {
        self.name.as_bytes() == verb
            || self.aliases.iter().any(|a| a.as_bytes() == verb)
    }
}

// ── Fastboot ──────────────────────────────────────────────────────────────

pub struct Fastboot<'d> {
    console: UsbSerialJtag<'d, Blocking>,
    line: [u8; 64],
    len: usize,
}

impl<'d> Fastboot<'d> {
    pub fn new(console: UsbSerialJtag<'d, Blocking>) -> Self {
        Self { console, line: [0; 64], len: 0 }
    }

    pub fn print_banner(&mut self) {
        self.write_line("");
        self.write_line("fastboot+ ready  |  type `help` for commands");
        self.write_prompt();
    }

    pub fn print_help(&mut self) {
        self.write_line("── Commands ─────────────────────────────────");
        for entry in REGISTRY {
            if entry.aliases.is_empty() {
                self.write_fmt_line(format_args!("  {:20}{}", entry.name, entry.help));
            } else {
                let mut buf = [0u8; 32];
                let mut pos = 0usize;
                for &b in entry.name.as_bytes() {
                    if pos < buf.len() { buf[pos] = b; pos += 1; }
                }
                for alias in entry.aliases {
                    if pos < buf.len() { buf[pos] = b'|'; pos += 1; }
                    for &b in alias.as_bytes() {
                        if pos < buf.len() { buf[pos] = b; pos += 1; }
                    }
                }
                let name_str = core::str::from_utf8(&buf[..pos]).unwrap_or("?");
                self.write_fmt_line(format_args!("  {:20}{}", name_str, entry.help));
            }
        }
        self.write_line("  help                list commands");
        self.write_prompt();
    }

    pub fn write_line(&mut self, line: &str) {
        let _ = writeln!(self.console, "{line}");
        let _ = self.console.flush_tx();
    }

    pub fn write_fmt_line(&mut self, args: core::fmt::Arguments<'_>) {
        let _ = self.console.write_fmt(args);
        let _ = writeln!(self.console);
        let _ = self.console.flush_tx();
    }

    pub fn write_prompt(&mut self) {
        let _ = write!(self.console, "fastboot+> ");
        let _ = self.console.flush_tx();
    }

    pub fn poll_command(&mut self) -> Option<&[u8]> {
        while let Ok(byte) = self.console.read_byte() {
            match byte {
                b'\r' | b'\n' => {
                    if self.len == 0 {
                        self.write_prompt();
                        continue;
                    }
                    let end = self.len;
                    self.len = 0;
                    return Some(&self.line[..end]);
                }
                0x08 | 0x7F => { self.len = self.len.saturating_sub(1); }
                b if (b == b' ' || b.is_ascii_graphic()) && self.len < self.line.len() => {
                    self.line[self.len] = b;
                    self.len += 1;
                }
                _ => {}
            }
        }
        None
    }
}

// ── Main fastboot+ loop ───────────────────────────────────────────────────────

fn flash_error_then_restore(board: &mut Board<'_>, delay: &Delay) {
    board.status_led.show_error(delay);
    delay.delay_millis(1000);
    board.status_led.show_fastboot(delay);
}

pub fn enter_fastboot<'a>(
    board: &mut Board<'a>,
    delay: &Delay,
    fastboot: &mut Fastboot<'a>,
    reason: BootError,
    ram: &RAM,
    cpu_to_gpu: &XtensaLx7CpuToGpu,
) -> FastbootExit {
    println!();
    println!("========== FASTBOOT+ ==========");
    println!("Reason: {}", reason.message());
    println!("Waiting in diagnostics mode — type `help` over USB serial.");
    println!("===============================");

    fastboot.print_banner();
    fastboot.write_line(reason.message());
    fastboot.write_prompt();

    let mut ticker = 0u32;
    let mut stream_logs = false;
    let mut led_mode = LedMode::Fastboot;

    board.status_led.show_fastboot(delay);

    loop {
        // Копируем входную строку сразу, чтобы не держать &self.line
        let raw: Option<([u8; 64], usize)> = match fastboot.poll_command() {
            Some(r) => {
                let mut buf = [0u8; 64];
                let len = r.len().min(64);
                buf[..len].copy_from_slice(&r[..len]);
                Some((buf, len))
            }
            None => None,
        };

        if let Some((buf, len)) = raw {
            let input = trim_ascii(&buf[..len]);
            let (verb, arg) = split_first_word(input);

            if verb == b"help" {
                fastboot.print_help();
            } else {
                let mut found = false;
                for entry in REGISTRY {
                    if entry.matches(verb) {
                        found = true;
                        let mut ctx = CmdCtx {
                            fb: fastboot,
                            board,
                            delay,
                            ram,
                            cpu_to_gpu,
                            stream_logs: &mut stream_logs,
                            led_mode: &mut led_mode,
                        };
                        if let Some(exit) = (entry.run)(trim_ascii(arg), &mut ctx) {
                            return exit;
                        }
                        break;
                    }
                }
                if !found {
                    fastboot.write_line("ERR unknown command — type `help` for list");
                    flash_error_then_restore(board, delay);
                    fastboot.write_prompt();
                }
            }
        }

        if board.poll_encoder_color_adjust(delay).is_some() {
            led_mode = LedMode::Manual;
        }
        let _ = board.poll_torch_toggle(delay);

        ticker = ticker.wrapping_add(1);
        if stream_logs && ticker.is_multiple_of(10) {
            println!(
                "[fastboot+] sd_present={} sd_status={} encoder_active={} power_pressed={}",
                board.sd_card_present(),
                board.sd_status().as_str(),
                board.encoder_active(),
                board.power_button_pressed(),
            );
        }

        delay.delay_millis(50);
    }
}

// ── Shared parse helpers ──────────────────────────────────────────────────────

pub fn split_first_word(bytes: &[u8]) -> (&[u8], &[u8]) {
    match bytes.iter().position(|b| b.is_ascii_whitespace()) {
        Some(pos) => (&bytes[..pos], &bytes[pos + 1..]),
        None => (bytes, b""),
    }
}

pub fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    let end   = bytes.iter().rposition(|b| !b.is_ascii_whitespace()).map(|i| i + 1).unwrap_or(0);
    if start >= end { b"" } else { &bytes[start..end] }
}

pub fn parse_u8(bytes: &[u8]) -> Option<u8> {
    if bytes.is_empty() { return None; }
    let mut v: u16 = 0;
    for &b in bytes {
        if !b.is_ascii_digit() { return None; }
        v = v * 10 + u16::from(b - b'0');
        if v > 255 { return None; }
    }
    Some(v as u8)
}

pub fn parse_rgb_arg(arg: &[u8]) -> Option<(RgbColor, u8)> {
    let mut parts = arg.split(|b| b.is_ascii_whitespace()).filter(|p| !p.is_empty());
    let r = parse_u8(parts.next()?)?;
    let g = parse_u8(parts.next()?)?;
    let b = parse_u8(parts.next()?)?;
    let brightness = match parts.next() {
        Some(brt) => { let v = parse_u8(brt)?; if v > 10 { return None; } v }
        None => 10,
    };
    if parts.any(|p| !p.is_empty()) { return None; }
    Some((RgbColor::new(r, g, b), brightness))
}