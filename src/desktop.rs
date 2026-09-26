use esp_hal::{
    Blocking,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

use crate::{
    config,
    display::{BootDisplay, DisplayError},
};

const EVENT_QUEUE_CAP: usize = 8;

/// Commands sent by the PC viewer to the real ESP32-S3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopEvent {
    Sync,
    Ping,
    Normal,
    Recovery,
    Fastboot,
    Rotate(i8),
    Power,
}

/// Non-blocking parser for the PC -> board control channel.
///
/// A small fixed event queue is used because a single USB packet can contain
/// several complete protocol lines. Older code returned the first event and
/// silently discarded the rest of the already-drained packet.
pub struct DesktopInput<'d> {
    rx: UsbSerialJtagRx<'d, Blocking>,
    line: [u8; 96],
    len: usize,
    pending: [Option<DesktopEvent>; EVENT_QUEUE_CAP],
    head: usize,
    tail: usize,
}

impl<'d> DesktopInput<'d> {
    pub fn new(rx: UsbSerialJtagRx<'d, Blocking>) -> Self {
        Self {
            rx,
            line: [0; 96],
            len: 0,
            pending: [None; EVENT_QUEUE_CAP],
            head: 0,
            tail: 0,
        }
    }

    pub fn poll(&mut self) -> Option<DesktopEvent> {
        if let Some(event) = self.pop_event() {
            return Some(event);
        }

        let mut buf = [0u8; 64];
        let n = self.rx.drain_rx_fifo(&mut buf);
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                if self.len != 0 {
                    let parsed = core::str::from_utf8(&self.line[..self.len])
                        .ok()
                        .and_then(parse_event);
                    self.len = 0;
                    if let Some(event) = parsed {
                        self.push_event(event);
                    }
                }
            } else if self.len < self.line.len() {
                self.line[self.len] = b;
                self.len += 1;
            } else {
                // Drop only the overlong line, not subsequent valid commands.
                self.len = 0;
            }
        }

        self.pop_event()
    }

    pub fn into_inner(self) -> UsbSerialJtagRx<'d, Blocking> {
        self.rx
    }

    fn push_event(&mut self, event: DesktopEvent) {
        let next = (self.tail + 1) % EVENT_QUEUE_CAP;
        if next == self.head {
            // Queue full: discard the oldest event. A fresh user command is
            // more useful than an old one for an interactive desktop link.
            self.pending[self.head] = None;
            self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        }
        self.pending[self.tail] = Some(event);
        self.tail = next;
    }

    fn pop_event(&mut self) -> Option<DesktopEvent> {
        if self.head == self.tail {
            return None;
        }
        let event = self.pending[self.head].take();
        self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        event
    }
}

fn parse_event(line: &str) -> Option<DesktopEvent> {
    match line.trim() {
        "@ZWIN|SYNC" => Some(DesktopEvent::Sync),
        "@ZWIN|PING" => Some(DesktopEvent::Ping),
        "@ZWIN|NORMAL" => Some(DesktopEvent::Normal),
        "@ZWIN|RECOVERY" => Some(DesktopEvent::Recovery),
        "@ZWIN|FASTBOOT" => Some(DesktopEvent::Fastboot),
        "@ZWIN|POWER" => Some(DesktopEvent::Power),
        "@ZWIN|ROTATE|-1" => Some(DesktopEvent::Rotate(-1)),
        "@ZWIN|ROTATE|1" => Some(DesktopEvent::Rotate(1)),
        _ => None,
    }
}

/// Mirrors every boot screen to the PC while keeping the real display active.
///
/// The USB side intentionally uses non-blocking byte writes. The watch must
/// boot even if no PC is connected or the viewer is closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopSurface {
    #[cfg(feature = "display-st7789")]
    St7789,
    #[cfg(feature = "display-ssd1306")]
    Ssd1306,
}

pub struct DesktopMirror<'d, D> {
    inner: D,
    tx: UsbSerialJtagTx<'d, Blocking>,
    surface: DesktopSurface,
}

impl<'d, D> DesktopMirror<'d, D> {
    pub fn new(inner: D, tx: UsbSerialJtagTx<'d, Blocking>, surface: DesktopSurface) -> Self {
        let mut this = Self { inner, tx, surface };
        this.hello();
        this
    }

    pub fn hello(&mut self) {
        match self.surface {
            #[cfg(feature = "display-st7789")]
            DesktopSurface::St7789 => self.send_parts(&[
                "@ZWUI", "HELLO", "2", "240", "280", config::PRODUCT, config::MODEL,
            ]),
            #[cfg(feature = "display-ssd1306")]
            DesktopSurface::Ssd1306 => self.send_parts(&[
                "@ZWUI", "HELLO", "2", "128", "64", config::PRODUCT, config::MODEL,
            ]),
        }
    }

    pub fn pong(&mut self) {
        self.send_parts(&["@ZWUI", "PONG", "2"]);
    }

    pub fn into_parts(self) -> (D, UsbSerialJtagTx<'d, Blocking>) {
        (self.inner, self.tx)
    }

    fn send_parts(&mut self, parts: &[&str]) {
        for (idx, part) in parts.iter().enumerate() {
            if idx != 0 {
                self.send_bytes(b"|");
            }
            self.send_bytes(part.as_bytes());
        }
        self.send_bytes(b"\r\n");
        let _ = self.tx.flush_tx_nb();
    }

    fn send_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            // Drop bytes when the hardware FIFO/host is unavailable. The PC
            // can reconnect at any time and issue SYNC; boot never waits for it.
            let _ = self.tx.write_byte_nb(b);
        }
    }
}

impl<D: BootDisplay> BootDisplay for DesktopMirror<'_, D> {
    fn boot_logo(&mut self) -> Result<(), DisplayError> {
        let result = self.inner.boot_logo();
        self.send_parts(&["@ZWUI", "BOOT"]);
        result
    }

    fn boot_progress(&mut self, label: &str, phase: u8) -> Result<(), DisplayError> {
        let result = self.inner.boot_progress(label, phase);
        let phase = match phase % 8 {
            0 => "0",
            1 => "1",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            _ => "7",
        };
        self.send_parts(&["@ZWUI", "PROGRESS", label, phase]);
        result
    }

    fn status(&mut self, title: &str, l1: &str, l2: &str, l3: &str) -> Result<(), DisplayError> {
        let result = self.inner.status(title, l1, l2, l3);
        self.send_parts(&["@ZWUI", "STATUS", title, l1, l2, l3]);
        result
    }

    fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError> {
        let result = self.inner.error(code, detail);
        self.send_parts(&["@ZWUI", "ERROR", code, detail]);
        result
    }
}
