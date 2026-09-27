//! Development block device transported over the ESP32-S3 USB-Serial-JTAG link.
//!
//! This is intentionally *not* USB Mass Storage host mode. The ESP32-S3-Zero
//! Type-C connector is wired as a USB device/power sink, so for solder-free
//! bring-up the PC serves a raw GPT disk image over the existing USB device
//! channel. The rest of the bootloader sees a normal 512-byte BlockDevice.

use core::{cell::RefCell, fmt::{self, Write as _}};

use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
use crate::{desktop::{self, DesktopEvent}, runtime::RuntimeFrame};

use esp_hal::{
    Blocking,
    delay::Delay,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

const SECTOR_SIZE: usize = 512;
const RX_PACKET: usize = 64;
const LINE_CAP: usize = 192;
const WAIT_STEP_US: u32 = 100;
const WAIT_STEPS: u32 = 300_000; // ~30 seconds for a development PC server.
const EVENT_QUEUE_CAP: usize = 8;
const CONTROL_LINE_CAP: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcBlockError {
    Timeout,
    Transport,
    Protocol,
    Crc,
    Range,
}

impl fmt::Display for PcBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => f.write_str("USB-PC block timeout"),
            Self::Transport => f.write_str("USB-PC block transport error"),
            Self::Protocol => f.write_str("USB-PC block protocol error"),
            Self::Crc => f.write_str("USB-PC block CRC mismatch"),
            Self::Range => f.write_str("USB-PC block LBA out of range"),
        }
    }
}

impl core::error::Error for PcBlockError {}

struct Link<'d> {
    rx: UsbSerialJtagRx<'d, Blocking>,
    tx: UsbSerialJtagTx<'d, Blocking>,
    delay: Delay,
    packet: [u8; RX_PACKET],
    packet_pos: usize,
    packet_len: usize,
    control_line: [u8; CONTROL_LINE_CAP],
    control_len: usize,
    pending: [Option<DesktopEvent>; EVENT_QUEUE_CAP],
    pending_head: usize,
    pending_tail: usize,
}

impl<'d> Link<'d> {
    fn send(&mut self, bytes: &[u8]) -> Result<(), PcBlockError> {
        self.tx.write(bytes).map_err(|_| PcBlockError::Transport)?;
        self.tx.flush_tx().map_err(|_| PcBlockError::Transport)
    }

    fn send_parts(&mut self, parts: &[&str]) -> Result<(), PcBlockError> {
        let mut line = SmallLine::new();
        for (idx, part) in parts.iter().enumerate() {
            if idx != 0 {
                write!(&mut line, "|").map_err(|_| PcBlockError::Protocol)?;
            }
            write!(&mut line, "{}", part).map_err(|_| PcBlockError::Protocol)?;
        }
        write!(&mut line, "\r\n").map_err(|_| PcBlockError::Protocol)?;
        self.send(line.as_bytes())
    }

    fn push_event(&mut self, event: DesktopEvent) {
        let next = (self.pending_tail + 1) % EVENT_QUEUE_CAP;
        if next == self.pending_head {
            self.pending[self.pending_head] = None;
            self.pending_head = (self.pending_head + 1) % EVENT_QUEUE_CAP;
        }
        self.pending[self.pending_tail] = Some(event);
        self.pending_tail = next;
    }

    fn pop_event(&mut self) -> Option<DesktopEvent> {
        if self.pending_head == self.pending_tail {
            return None;
        }
        let event = self.pending[self.pending_head].take();
        self.pending_head = (self.pending_head + 1) % EVENT_QUEUE_CAP;
        event
    }

    fn feed_control_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                if self.control_len != 0 {
                    if let Ok(text) = core::str::from_utf8(&self.control_line[..self.control_len]) {
                        if let Some(event) = desktop::parse_event(text) {
                            self.push_event(event);
                        }
                    }
                    self.control_len = 0;
                }
            }
            b'\r' => {}
            _ if self.control_len < self.control_line.len() => {
                self.control_line[self.control_len] = byte;
                self.control_len += 1;
            }
            _ => self.control_len = 0,
        }
    }

    fn poll_event_nonblocking(&mut self) -> Option<DesktopEvent> {
        if let Some(event) = self.pop_event() {
            return Some(event);
        }

        // Consume any bytes already buffered by a previous block response.
        while self.packet_pos < self.packet_len {
            let byte = self.packet[self.packet_pos];
            self.packet_pos += 1;
            self.feed_control_byte(byte);
        }
        self.packet_pos = 0;
        self.packet_len = 0;
        if let Some(event) = self.pop_event() {
            return Some(event);
        }

        let mut buf = [0u8; RX_PACKET];
        let n = self.rx.drain_rx_fifo(&mut buf);
        for &byte in &buf[..n] {
            self.feed_control_byte(byte);
        }
        self.pop_event()
    }

    fn read_byte(&mut self) -> Result<u8, PcBlockError> {
        let mut waits = 0u32;
        loop {
            if self.packet_pos < self.packet_len {
                let byte = self.packet[self.packet_pos];
                self.packet_pos += 1;
                return Ok(byte);
            }

            self.packet_pos = 0;
            self.packet_len = self.rx.drain_rx_fifo(&mut self.packet);
            if self.packet_len != 0 {
                continue;
            }

            if waits >= WAIT_STEPS {
                return Err(PcBlockError::Timeout);
            }
            waits += 1;
            self.delay.delay_micros(WAIT_STEP_US);
        }
    }

    fn read_line(&mut self, out: &mut [u8; LINE_CAP]) -> Result<usize, PcBlockError> {
        let mut len = 0usize;
        loop {
            let byte = self.read_byte()?;
            match byte {
                b'\n' => return Ok(len),
                b'\r' => {}
                _ if len < out.len() => {
                    out[len] = byte;
                    len += 1;
                }
                _ => return Err(PcBlockError::Protocol),
            }
        }
    }

    fn read_exact(&mut self, out: &mut [u8]) -> Result<(), PcBlockError> {
        for byte in out {
            *byte = self.read_byte()?;
        }
        Ok(())
    }

    fn request_size(&mut self) -> Result<u32, PcBlockError> {
        self.send(b"@ZBLK|HELLO|1\r\n")?;
        // READY is informational. Older/newer PC servers may omit it, so ask
        // SIZE and ignore protocol lines until a SIZE response arrives.
        self.send(b"@ZBLK|SIZE\r\n")?;
        let mut line = [0u8; LINE_CAP];
        for _ in 0..8 {
            let len = self.read_line(&mut line)?;
            let text = core::str::from_utf8(&line[..len]).map_err(|_| PcBlockError::Protocol)?;
            if let Some(value) = text.strip_prefix("@ZBLK|SIZE|") {
                return value.parse::<u32>().map_err(|_| PcBlockError::Protocol);
            }
            if text.starts_with("@ZBLK|FAIL|") {
                return Err(PcBlockError::Protocol);
            }
        }
        Err(PcBlockError::Protocol)
    }

    fn read_sector(&mut self, lba: u32, out: &mut [u8; SECTOR_SIZE]) -> Result<(), PcBlockError> {
        let mut cmd = SmallLine::new();
        write!(&mut cmd, "@ZBLK|READ|{}|1\r\n", lba).map_err(|_| PcBlockError::Protocol)?;
        self.send(cmd.as_bytes())?;

        let mut line = [0u8; LINE_CAP];
        let len = self.read_line(&mut line)?;
        let text = core::str::from_utf8(&line[..len]).map_err(|_| PcBlockError::Protocol)?;
        let mut fields = text.split('|');
        if fields.next() != Some("@ZBLK") || fields.next() != Some("DATA") {
            return Err(PcBlockError::Protocol);
        }
        let bytes = fields
            .next()
            .ok_or(PcBlockError::Protocol)?
            .parse::<usize>()
            .map_err(|_| PcBlockError::Protocol)?;
        let expected_crc = parse_hex_u32(fields.next().ok_or(PcBlockError::Protocol)?)?;
        if bytes != SECTOR_SIZE || fields.next().is_some() {
            return Err(PcBlockError::Protocol);
        }

        self.read_exact(out)?;
        if crc32(out) != expected_crc {
            return Err(PcBlockError::Crc);
        }
        Ok(())
    }

    fn write_sector(&mut self, lba: u32, data: &[u8; SECTOR_SIZE]) -> Result<(), PcBlockError> {
        let mut cmd = SmallLine::new();
        write!(
            &mut cmd,
            "@ZBLK|WRITE|{}|1|{}|{:08x}\r\n",
            lba,
            SECTOR_SIZE,
            crc32(data),
        )
        .map_err(|_| PcBlockError::Protocol)?;
        self.send(cmd.as_bytes())?;
        self.send(data)?;

        let mut line = [0u8; LINE_CAP];
        let len = self.read_line(&mut line)?;
        let text = core::str::from_utf8(&line[..len]).map_err(|_| PcBlockError::Protocol)?;
        if text == "@ZBLK|OK" {
            Ok(())
        } else {
            Err(PcBlockError::Protocol)
        }
    }
}

/// Raw development disk served by `usb_block_server.py` on the attached PC.
pub struct UsbPcBlockDevice<'d> {
    link: RefCell<Link<'d>>,
    blocks: RefCell<Option<u32>>,
}

impl<'d> UsbPcBlockDevice<'d> {
    pub fn new(
        rx: UsbSerialJtagRx<'d, Blocking>,
        tx: UsbSerialJtagTx<'d, Blocking>,
        delay: Delay,
    ) -> Self {
        Self {
            link: RefCell::new(Link {
                rx,
                tx,
                delay,
                packet: [0; RX_PACKET],
                packet_pos: 0,
                packet_len: 0,
                control_line: [0; CONTROL_LINE_CAP],
                control_len: 0,
                pending: [None; EVENT_QUEUE_CAP],
                pending_head: 0,
                pending_tail: 0,
            }),
            blocks: RefCell::new(None),
        }
    }

    pub fn viewer_hello(&self) {
        let mut link = self.link.borrow_mut();
        #[cfg(feature = "display-st7789")]
        let _ = link.send_parts(&["@ZWUI", "HELLO", "5", "240", "280", crate::config::PRODUCT, crate::config::MODEL]);
        #[cfg(feature = "display-ssd1306")]
        let _ = link.send_parts(&["@ZWUI", "HELLO", "5", "128", "64", crate::config::PRODUCT, crate::config::MODEL]);
    }

    pub fn viewer_boot(&self) {
        let _ = self.link.borrow_mut().send_parts(&["@ZWUI", "BOOT"]);
    }

    pub fn viewer_progress(&self, label: &str, phase: u8) {
        let phase = match phase % 8 {
            0 => "0", 1 => "1", 2 => "2", 3 => "3",
            4 => "4", 5 => "5", 6 => "6", _ => "7",
        };
        let _ = self.link.borrow_mut().send_parts(&["@ZWUI", "PROGRESS", label, phase]);
    }

    pub fn viewer_status(&self, title: &str, l1: &str, l2: &str, l3: &str) {
        let _ = self.link.borrow_mut().send_parts(&["@ZWUI", "STATUS", title, l1, l2, l3]);
    }

    pub fn viewer_runtime(&self, frame: &RuntimeFrame) {
        match frame {
            RuntimeFrame::Legacy(f) => self.viewer_status(&f.title, &f.line1, &f.line2, &f.line3),
            RuntimeFrame::Material(f) => {
                let mut b = SmallLine::new();
                let _ = write!(b, "@ZWUI|M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
                    f.screen, f.cursor, f.brightness,
                    if f.dnd { 1 } else { 0 }, if f.airplane { 1 } else { 0 }, f.theme,
                    if f.locked { 1 } else { 0 }, if f.wifi { 1 } else { 0 },
                    if f.bt { 1 } else { 0 }, if f.adb { 1 } else { 0 },
                    f.notes, f.time_minutes, f.swap_pages, f.timer_secs, f.stopwatch_secs,
                    if f.alarm_enabled { 1 } else { 0 }, if f.auto_time { 1 } else { 0 },
                    if f.time_valid { 1 } else { 0 }, f.timezone_hours,
                    f.wifi_ap_count, f.wifi_ap_index, f.wifi_ap_rssi, f.wifi_password_len,
                    f.wifi_editor_char, f.wifi_link_state);
                for &byte in &f.wifi_ap_ssid[..f.wifi_ap_ssid_len as usize] { let _ = write!(b, "{:02x}", byte); }
                let _ = write!(b, "\r\n");
                let _ = self.link.borrow_mut().send(b.as_bytes());
            }
        }
    }

    pub fn viewer_error(&self, code: &str, detail: &str) {
        let _ = self.link.borrow_mut().send_parts(&["@ZWUI", "ERROR", code, detail]);
    }

    pub fn viewer_pong(&self) {
        let _ = self.link.borrow_mut().send_parts(&["@ZWUI", "PONG", "5"]);
    }

    /// Developer debug output used by the ZADB bridge. This is intentionally
    /// line-oriented and multiplexed through the same no-reset PC bridge.
    pub fn viewer_adb_output(&self, kind: &str, text: &str) {
        let _ = self.link.borrow_mut().send_parts(&["@ZADB", "OUT", kind, text]);
    }

    /// Tell the PC block server it may now forward interactive viewer commands.
    /// Until this point the server deliberately queues them so block protocol
    /// traffic cannot be interleaved with UI control lines.
    pub fn viewer_interactive(&self) {
        let _ = self.link.borrow_mut().send(b"@ZBLK|INTERACTIVE\r\n");
    }

    pub fn poll_desktop_event(&self) -> Option<DesktopEvent> {
        self.link.borrow_mut().poll_event_nonblocking()
    }

    fn capacity(&self) -> Result<u32, PcBlockError> {
        if let Some(blocks) = *self.blocks.borrow() {
            return Ok(blocks);
        }
        let blocks = self.link.borrow_mut().request_size()?;
        if blocks == 0 {
            return Err(PcBlockError::Protocol);
        }
        *self.blocks.borrow_mut() = Some(blocks);
        Ok(blocks)
    }
}

impl BlockDevice for UsbPcBlockDevice<'_> {
    type Error = PcBlockError;

    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let capacity = self.capacity()?;
        let start = start_block_idx.0;
        let end = start.checked_add(blocks.len() as u32).ok_or(PcBlockError::Range)?;
        if end > capacity {
            return Err(PcBlockError::Range);
        }

        let mut link = self.link.borrow_mut();
        for (offset, block) in blocks.iter_mut().enumerate() {
            link.read_sector(start + offset as u32, &mut block.contents)?;
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let capacity = self.capacity()?;
        let start = start_block_idx.0;
        let end = start.checked_add(blocks.len() as u32).ok_or(PcBlockError::Range)?;
        if end > capacity {
            return Err(PcBlockError::Range);
        }

        let mut link = self.link.borrow_mut();
        for (offset, block) in blocks.iter().enumerate() {
            link.write_sector(start + offset as u32, &block.contents)?;
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        self.capacity().map(BlockCount)
    }
}

struct SmallLine {
    buf: [u8; 640],
    len: usize,
}

impl SmallLine {
    const fn new() -> Self {
        Self { buf: [0; 640], len: 0 }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl fmt::Write for SmallLine {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let bytes = text.as_bytes();
        if self.len + bytes.len() > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

fn parse_hex_u32(text: &str) -> Result<u32, PcBlockError> {
    u32::from_str_radix(text, 16).map_err(|_| PcBlockError::Protocol)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
