#[cfg(not(feature = "pc-block-boot"))]
use esp_hal::{
    Blocking,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

#[cfg(not(feature = "pc-block-boot"))]
use crate::{
    config,
    display::{BootDisplay, DisplayError},
    runtime::RuntimeFrame,
};

#[cfg(not(feature = "pc-block-boot"))]
const EVENT_QUEUE_CAP: usize = 8;
#[cfg(not(feature = "pc-block-boot"))]
const CONTROL_LINE_CAP: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WifiCredentials {
    ssid: [u8; 32],
    ssid_len: u8,
    password: [u8; 64],
    password_len: u8,
}

impl WifiCredentials {
    pub const fn empty() -> Self {
        Self { ssid: [0; 32], ssid_len: 0, password: [0; 64], password_len: 0 }
    }

    pub fn ssid(&self) -> Option<&str> {
        core::str::from_utf8(&self.ssid[..self.ssid_len as usize]).ok()
    }

    pub fn password(&self) -> Option<&str> {
        core::str::from_utf8(&self.password[..self.password_len as usize]).ok()
    }

    pub fn from_parts(ssid: &str, password: &str) -> Option<Self> {
        let sb = ssid.as_bytes(); let pb = password.as_bytes();
        if sb.is_empty() || sb.len() > 32 || pb.len() > 64 { return None; }
        let mut out = Self::empty();
        out.ssid[..sb.len()].copy_from_slice(sb); out.ssid_len = sb.len() as u8;
        out.password[..pb.len()].copy_from_slice(pb); out.password_len = pb.len() as u8;
        Some(out)
    }

    pub fn encode_page(&self, out: &mut [u8; 98]) -> usize {
        out[0] = self.ssid_len;
        out[1] = self.password_len;
        let sl = self.ssid_len as usize;
        let pl = self.password_len as usize;
        out[2..2 + sl].copy_from_slice(&self.ssid[..sl]);
        out[2 + sl..2 + sl + pl].copy_from_slice(&self.password[..pl]);
        2 + sl + pl
    }

    pub fn decode_page(raw: &[u8]) -> Option<Self> {
        if raw.len() < 2 { return None; }
        let sl = raw[0] as usize;
        let pl = raw[1] as usize;
        if sl > 32 || pl > 64 || raw.len() != 2 + sl + pl { return None; }
        let mut out = Self::empty();
        out.ssid[..sl].copy_from_slice(&raw[2..2 + sl]);
        out.password[..pl].copy_from_slice(&raw[2 + sl..]);
        out.ssid_len = sl as u8;
        out.password_len = pl as u8;
        out.ssid()?;
        out.password()?;
        Some(out)
    }

    fn from_hex(ssid: &str, password: &str) -> Option<Self> {
        let mut out = Self::empty();
        out.ssid_len = decode_hex(ssid, &mut out.ssid)? as u8;
        out.password_len = decode_hex(password, &mut out.password)? as u8;
        out.ssid()?;
        out.password()?;
        Some(out)
    }
}

fn decode_hex(src: &str, out: &mut [u8]) -> Option<usize> {
    if src.len() % 2 != 0 || src.len() / 2 > out.len() { return None; }
    let bytes = src.as_bytes();
    for i in 0..bytes.len() / 2 {
        out[i] = (hex_nibble(bytes[i * 2])? << 4) | hex_nibble(bytes[i * 2 + 1])?;
    }
    Some(bytes.len() / 2)
}

fn hex_nibble(v: u8) -> Option<u8> {
    match v {
        b'0'..=b'9' => Some(v - b'0'),
        b'a'..=b'f' => Some(v - b'a' + 10),
        b'A'..=b'F' => Some(v - b'A' + 10),
        _ => None,
    }
}

/// Commands sent by the PC viewer/ZADB client to the real ESP32-S3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopEvent {
    Sync,
    Ping,
    Normal,
    Recovery,
    Fastboot,
    Rotate(i8),
    Swipe(i8),
    CompanionTime(u16),
    Power,
    AdbGetProp,
    AdbServices,
    AdbPackages,
    AdbDumpsys,
    WifiScan,
    WifiStatus,
    WifiConnect(WifiCredentials),
    WifiDisconnect,
    BtScan,
    BtStatus,
    BtConnect([u8; 6]),
    BtDisconnect,
}

/// Non-blocking parser for the PC -> board control channel.
#[cfg(not(feature = "pc-block-boot"))]
pub struct DesktopInput<'d> {
    rx: UsbSerialJtagRx<'d, Blocking>,
    line: [u8; CONTROL_LINE_CAP],
    len: usize,
    pending: [Option<DesktopEvent>; EVENT_QUEUE_CAP],
    head: usize,
    tail: usize,
}

#[cfg(not(feature = "pc-block-boot"))]
impl<'d> DesktopInput<'d> {
    pub fn new(rx: UsbSerialJtagRx<'d, Blocking>) -> Self {
        Self {
            rx,
            line: [0; CONTROL_LINE_CAP],
            len: 0,
            pending: [None; EVENT_QUEUE_CAP],
            head: 0,
            tail: 0,
        }
    }

    pub fn poll(&mut self) -> Option<DesktopEvent> {
        if let Some(event) = self.pop_event() { return Some(event); }
        let mut buf = [0u8; 64];
        let n = self.rx.drain_rx_fifo(&mut buf);
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                if self.len != 0 {
                    let parsed = core::str::from_utf8(&self.line[..self.len]).ok().and_then(parse_event);
                    self.len = 0;
                    if let Some(event) = parsed { self.push_event(event); }
                }
            } else if self.len < self.line.len() {
                self.line[self.len] = b;
                self.len += 1;
            } else {
                self.len = 0;
            }
        }
        self.pop_event()
    }

    pub fn into_inner(self) -> UsbSerialJtagRx<'d, Blocking> { self.rx }

    fn push_event(&mut self, event: DesktopEvent) {
        let next = (self.tail + 1) % EVENT_QUEUE_CAP;
        if next == self.head {
            self.pending[self.head] = None;
            self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        }
        self.pending[self.tail] = Some(event);
        self.tail = next;
    }

    fn pop_event(&mut self) -> Option<DesktopEvent> {
        if self.head == self.tail { return None; }
        let event = self.pending[self.head].take();
        self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        event
    }
}

pub(crate) fn parse_event(line: &str) -> Option<DesktopEvent> {
    let line = line.trim();
    match line {
        "@ZWIN|SYNC" => return Some(DesktopEvent::Sync),
        "@ZWIN|PING" => return Some(DesktopEvent::Ping),
        "@ZWIN|NORMAL" => return Some(DesktopEvent::Normal),
        "@ZWIN|RECOVERY" => return Some(DesktopEvent::Recovery),
        "@ZWIN|FASTBOOT" => return Some(DesktopEvent::Fastboot),
        "@ZWIN|POWER" => return Some(DesktopEvent::Power),
        "@ZWIN|ROTATE|-1" => return Some(DesktopEvent::Rotate(-1)),
        "@ZWIN|ROTATE|1" => return Some(DesktopEvent::Rotate(1)),
        "@ZWIN|SWIPE|UP" => return Some(DesktopEvent::Swipe(-1)),
        "@ZWIN|SWIPE|DOWN" => return Some(DesktopEvent::Swipe(1)),
        "@ZWIN|SWIPE|LEFT" => return Some(DesktopEvent::Swipe(-2)),
        "@ZWIN|SWIPE|RIGHT" => return Some(DesktopEvent::Swipe(2)),
        "@ZADB|GETPROP" => return Some(DesktopEvent::AdbGetProp),
        "@ZADB|SERVICES" => return Some(DesktopEvent::AdbServices),
        "@ZADB|PACKAGES" => return Some(DesktopEvent::AdbPackages),
        "@ZADB|DUMPSYS" => return Some(DesktopEvent::AdbDumpsys),
        "@ZADB|WIFI_SCAN" => return Some(DesktopEvent::WifiScan),
        "@ZADB|WIFI_STATUS" => return Some(DesktopEvent::WifiStatus),
        "@ZADB|WIFI_DISCONNECT" => return Some(DesktopEvent::WifiDisconnect),
        "@ZADB|BT_SCAN" => return Some(DesktopEvent::BtScan),
        "@ZADB|BT_STATUS" => return Some(DesktopEvent::BtStatus),
        "@ZADB|BT_DISCONNECT" => return Some(DesktopEvent::BtDisconnect),
        _ => {}
    }
    // Explicit Wear companion debug hook. The desktop viewer never sends
    // this automatically; production time comes from a paired companion.
    if let Some(rest) = line.strip_prefix("@ZADB|COMPANION_TIME|") {
        let minutes = rest.parse::<u16>().ok()?;
        if minutes < 1440 { return Some(DesktopEvent::CompanionTime(minutes)); }
        return None;
    }
    if let Some(rest) = line.strip_prefix("@ZADB|WIFI_CONNECT|") {
        let (ssid, password) = rest.split_once('|')?;
        return WifiCredentials::from_hex(ssid, password).map(DesktopEvent::WifiConnect);
    }
    if let Some(rest) = line.strip_prefix("@ZADB|BT_CONNECT|") {
        return decode_bt_address(rest).map(DesktopEvent::BtConnect);
    }
    None
}

fn decode_bt_address(src: &str) -> Option<[u8; 6]> {
    let mut canonical = [0u8; 12];
    let mut n = 0usize;
    for b in src.bytes() {
        if b == b':' || b == b'-' { continue; }
        if n >= canonical.len() { return None; }
        canonical[n] = b;
        n += 1;
    }
    if n != 12 { return None; }
    let text = core::str::from_utf8(&canonical).ok()?;
    let mut forward = [0u8; 6];
    decode_hex(text, &mut forward)?;
    // HCI LE addresses are serialized least-significant octet first.
    forward.reverse();
    Some(forward)
}

/// Mirrors every boot screen to the PC while keeping the real display active.
///
/// The USB side intentionally uses non-blocking byte writes. The watch must
/// boot even if no PC is connected or the viewer is closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(not(feature = "pc-block-boot"))]
pub enum DesktopSurface {
    #[cfg(feature = "display-st7789")]
    St7789,
    #[cfg(feature = "display-ssd1306")]
    Ssd1306,
}

#[cfg(not(feature = "pc-block-boot"))]
pub struct DesktopMirror<'d, D> {
    inner: D,
    tx: UsbSerialJtagTx<'d, Blocking>,
    surface: DesktopSurface,
}

#[cfg(not(feature = "pc-block-boot"))]
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
                "@ZWUI", "HELLO", "5", "240", "280", config::PRODUCT, config::MODEL,
            ]),
            #[cfg(feature = "display-ssd1306")]
            DesktopSurface::Ssd1306 => self.send_parts(&[
                "@ZWUI", "HELLO", "5", "128", "64", config::PRODUCT, config::MODEL,
            ]),
        }
    }

    pub fn pong(&mut self) {
        self.send_parts(&["@ZWUI", "PONG", "5"]);
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


#[cfg(not(feature = "pc-block-boot"))]
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

    fn runtime(&mut self, frame: &RuntimeFrame) -> Result<(), DisplayError> {
        let result = self.inner.runtime(frame);
        match frame {
            RuntimeFrame::Legacy(f) => self.send_parts(&["@ZWUI", "STATUS", &f.title, &f.line1, &f.line2, &f.line3]),
            RuntimeFrame::Material(f) => {
                let mut line = heapless::String::<256>::new();
                let _ = core::fmt::write(&mut line, format_args!(
                    "@ZWUI|M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
                    f.screen, f.cursor, f.brightness,
                    if f.dnd { 1 } else { 0 }, if f.airplane { 1 } else { 0 }, f.theme,
                    if f.locked { 1 } else { 0 }, if f.wifi { 1 } else { 0 },
                    if f.bt { 1 } else { 0 }, if f.adb { 1 } else { 0 },
                    f.notes, f.time_minutes, f.swap_pages, f.timer_secs, f.stopwatch_secs,
                    if f.alarm_enabled { 1 } else { 0 }, if f.auto_time { 1 } else { 0 },
                    if f.time_valid { 1 } else { 0 }, f.timezone_hours,
                    f.wifi_ap_count, f.wifi_ap_index, f.wifi_ap_rssi, f.wifi_password_len,
                    f.wifi_editor_char, f.wifi_link_state,
                ));
                for &byte in &f.wifi_ap_ssid[..f.wifi_ap_ssid_len as usize] { let _ = core::fmt::write(&mut line, format_args!("{:02x}", byte)); }
                self.send_parts(&[line.as_str()]);
            }
        }
        result
    }

    fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError> {
        let result = self.inner.error(code, detail);
        self.send_parts(&["@ZWUI", "ERROR", code, detail]);
        result
    }
}
