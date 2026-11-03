// Модуль USB-протокола настольного просмотрщика.
use esp_hal::{
    Blocking,
    usb_serial_jtag::{UsbSerialJtagRx, UsbSerialJtagTx},
};

use crate::{
    config,
    display::{BootDisplay, DisplayError},
    runtime::RuntimeFrame,
};

/// Параметр EVENT_QUEUE_CAP компонента USB-протокола настольного просмотрщика.
const EVENT_QUEUE_CAP: usize = 8;
/// Параметр CONTROL_LINE_CAP компонента USB-протокола настольного просмотрщика.
const CONTROL_LINE_CAP: usize = 256;

#[path = "desktop_tx.rs"]
mod tx_queue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Структура данных компонента USB-протокола настольного просмотрщика.
pub struct WifiCredentials {
    /// Имя Wi-Fi-сети.
    ssid: [u8; 32],
    /// Число значащих байтов буфера ssid.
    ssid_len: u8,
    /// Ограниченный буфер пароля сети; не является системным именем.
    password: [u8; 64],
    /// Число значащих байтов буфера password.
    password_len: u8,
}

impl WifiCredentials {
    /// Создаёт пустую запись без пользовательских данных.
    pub const fn empty() -> Self {
        Self {
            ssid: [0; 32],
            ssid_len: 0,
            password: [0; 64],
            password_len: 0,
        }
    }

    /// Возвращает сохранённый SSID как проверенную UTF-8-строку.
    pub fn ssid(&self) -> Option<&str> {
        core::str::from_utf8(&self.ssid[..self.ssid_len as usize]).ok()
    }

    /// Возвращает пароль из ограниченного буфера профиля.
    pub fn password(&self) -> Option<&str> {
        core::str::from_utf8(&self.password[..self.password_len as usize]).ok()
    }

    /// Собирает профиль из SSID и пароля с проверкой размеров.
    pub fn from_parts(ssid: &str, password: &str) -> Option<Self> {
        let sb = ssid.as_bytes();
        let pb = password.as_bytes();
        if sb.is_empty() || sb.len() > 32 || pb.len() > 64 {
            return None;
        }
        let mut out = Self::empty();
        out.ssid[..sb.len()].copy_from_slice(sb);
        out.ssid_len = sb.len() as u8;
        out.password[..pb.len()].copy_from_slice(pb);
        out.password_len = pb.len() as u8;
        Some(out)
    }

    /// Рисует экран encode с актуальным состоянием и фокусом.
    pub fn encode_page(&self, out: &mut [u8; 98]) -> usize {
        out[0] = self.ssid_len;
        out[1] = self.password_len;
        let sl = self.ssid_len as usize;
        let pl = self.password_len as usize;
        out[2..2 + sl].copy_from_slice(&self.ssid[..sl]);
        out[2 + sl..2 + sl + pl].copy_from_slice(&self.password[..pl]);
        2 + sl + pl
    }

    /// Рисует экран decode с актуальным состоянием и фокусом.
    pub fn decode_page(raw: &[u8]) -> Option<Self> {
        if raw.len() < 2 {
            return None;
        }
        let sl = raw[0] as usize;
        let pl = raw[1] as usize;
        if sl > 32 || pl > 64 || raw.len() != 2 + sl + pl {
            return None;
        }
        let mut out = Self::empty();
        out.ssid[..sl].copy_from_slice(&raw[2..2 + sl]);
        out.password[..pl].copy_from_slice(&raw[2 + sl..]);
        out.ssid_len = sl as u8;
        out.password_len = pl as u8;
        out.ssid()?;
        out.password()?;
        Some(out)
    }

    /// Разбирает ограниченную hex-строку протокола.
    fn from_hex(ssid: &str, password: &str) -> Option<Self> {
        let mut out = Self::empty();
        out.ssid_len = decode_hex(ssid, &mut out.ssid)? as u8;
        out.password_len = decode_hex(password, &mut out.password)? as u8;
        out.ssid()?;
        out.password()?;
        Some(out)
    }
}

/// Декодирует hex-байты с проверкой символов и длины.
fn decode_hex(src: &str, out: &mut [u8]) -> Option<usize> {
    if src.len() % 2 != 0 || src.len() / 2 > out.len() {
        return None;
    }
    let bytes = src.as_bytes();
    for i in 0..bytes.len() / 2 {
        out[i] = (hex_nibble(bytes[i * 2])? << 4) | hex_nibble(bytes[i * 2 + 1])?;
    }
    Some(bytes.len() / 2)
}

/// Преобразует один шестнадцатеричный знак в значение 0–15.
fn hex_nibble(v: u8) -> Option<u8> {
    match v {
        b'0'..=b'9' => Some(v - b'0'),
        b'a'..=b'f' => Some(v - b'a' + 10),
        b'A'..=b'F' => Some(v - b'A' + 10),
        _ => None,
    }
}

/// Small copyable command buffer; malformed UTF-8 never enters the runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugRequest {
    /// Текущее количество байтов пользовательского ввода.
    bytes: [u8; 80],
    /// Число значащих байтов в соответствующем текстовом буфере.
    len: u8,
}
impl DebugRequest {
    /// Возвращает строковое представление значения.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
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
    AdbCommand(DebugRequest),
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
pub struct DesktopInput<'d> {
    /// Приёмная сторона канала обмена.
    rx: UsbSerialJtagRx<'d, Blocking>,
    /// Выбранная строка документа.
    line: [u8; CONTROL_LINE_CAP],
    /// Число значащих байтов в соответствующем текстовом буфере.
    len: usize,
    /// Необработанный запрос или ожидающий результат.
    pending: [Option<DesktopEvent>; EVENT_QUEUE_CAP],
    /// Позиция начала кольцевой очереди.
    head: usize,
    /// Позиция конца кольцевой очереди.
    tail: usize,
}

impl<'d> DesktopInput<'d> {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
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

    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
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
                self.len = 0;
            }
        }
        self.pop_event()
    }

    /// Возвращает обёрнутый транспорт вызывающему коду.
    pub fn into_inner(self) -> UsbSerialJtagRx<'d, Blocking> {
        self.rx
    }

    /// Ставит событие ввода в ограниченную очередь.
    fn push_event(&mut self, event: DesktopEvent) {
        let next = (self.tail + 1) % EVENT_QUEUE_CAP;
        if next == self.head {
            self.pending[self.head] = None;
            self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        }
        self.pending[self.tail] = Some(event);
        self.tail = next;
    }

    /// Забирает следующее ожидающее событие.
    fn pop_event(&mut self) -> Option<DesktopEvent> {
        if self.head == self.tail {
            return None;
        }
        let event = self.pending[self.head].take();
        self.head = (self.head + 1) % EVENT_QUEUE_CAP;
        event
    }
}

/// Разбирает строку настольного протокола в типизированное событие.
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
    if let Some(hex) = line.strip_prefix("@ZADB|CMD|") {
        let mut bytes = [0; 80];
        let len = decode_hex(hex, &mut bytes)?;
        core::str::from_utf8(&bytes[..len]).ok()?;
        return Some(DesktopEvent::AdbCommand(DebugRequest {
            bytes,
            len: len as u8,
        }));
    }
    // Explicit Wear companion debug hook. The desktop viewer never sends
    // this automatically; production time comes from a paired companion.
    if let Some(rest) = line.strip_prefix("@ZADB|COMPANION_TIME|") {
        let minutes = rest.parse::<u16>().ok()?;
        if minutes < 1440 {
            return Some(DesktopEvent::CompanionTime(minutes));
        }
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

/// Разбирает шестибайтный Bluetooth-адрес.
fn decode_bt_address(src: &str) -> Option<[u8; 6]> {
    let mut canonical = [0u8; 12];
    let mut n = 0usize;
    for b in src.bytes() {
        if b == b':' || b == b'-' {
            continue;
        }
        if n >= canonical.len() {
            return None;
        }
        canonical[n] = b;
        n += 1;
    }
    if n != 12 {
        return None;
    }
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
pub enum DesktopSurface {
    #[cfg(feature = "display-st7789")]
    St7789,
    #[cfg(feature = "display-ssd1306")]
    Ssd1306,
}

/// Структура данных компонента USB-протокола настольного просмотрщика.
pub struct DesktopMirror<'d, D> {
    /// Обёрнутый объект транспорта или состояния.
    inner: D,
    /// Передающая сторона канала обмена.
    tx: UsbSerialJtagTx<'d, Blocking>,
    /// Основная подложка карточек.
    surface: DesktopSurface,
    /// Необработанный запрос или ожидающий результат.
    pending: tx_queue::PacketQueue,
    /// Состояние текущего потока обмена.
    stream: crate::viewer_stream::ViewerStream,
}

impl<'d, D> DesktopMirror<'d, D> {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(inner: D, tx: UsbSerialJtagTx<'d, Blocking>, surface: DesktopSurface) -> Self {
        let mut this = Self {
            inner,
            tx,
            surface,
            pending: tx_queue::PacketQueue::new(),
            stream: crate::viewer_stream::ViewerStream::default(),
        };
        this.hello();
        this
    }

    /// Передаёт приветствие и сведения о подключении просмотрщика.
    pub fn hello(&mut self) {
        match self.surface {
            #[cfg(feature = "display-st7789")]
            DesktopSurface::St7789 => self.send_parts(&[
                "@ZWUI",
                "HELLO",
                "5",
                "240",
                "280",
                config::PRODUCT,
                config::MODEL,
            ]),
            #[cfg(feature = "display-ssd1306")]
            DesktopSurface::Ssd1306 => self.send_parts(&[
                "@ZWUI",
                "HELLO",
                "5",
                "128",
                "64",
                config::PRODUCT,
                config::MODEL,
            ]),
        }
    }

    /// Отвечает на проверку активности просмотрщика.
    pub fn pong(&mut self) {
        self.stream.touch(
            esp_hal::time::Instant::now()
                .duration_since_epoch()
                .as_millis(),
            false,
        );
        self.send_parts(&["@ZWUI", "PONG", "5"]);
    }
    /// Публикует актуальное состояние подписанному просмотрщику.
    #[cfg(not(feature = "pc-block-boot"))]
    pub fn viewer_sync(&mut self) {
        self.stream.touch(
            esp_hal::time::Instant::now()
                .duration_since_epoch()
                .as_millis(),
            true,
        );
    }

    /// Разделяет обёртку на транспорт и принадлежащие ей компоненты.
    pub fn into_parts(self) -> (D, UsbSerialJtagTx<'d, Blocking>) {
        (self.inner, self.tx)
    }

    /// Передаёт результат ADB/ZADB-команды в просмотрщик.
    #[cfg(not(feature = "pc-block-boot"))]
    pub fn adb_output(&mut self, topic: &str, text: &str) {
        self.send_parts(&["@ZADB", "OUT", topic, text]);
    }
    /// Отправляет составной пакет без сборки лишней временной строки.
    fn send_parts(&mut self, parts: &[&str]) {
        self.pump();
        self.pending.enqueue(parts);
        self.pump();
    }

    /// Continue a partially sent line between frames, without waiting for USB.
    pub fn pump(&mut self) {
        self.pending
            .pump(|byte| self.tx.write_byte_nb(byte).is_ok());
        let _ = self.tx.flush_tx_nb();
    }
}

impl<D: BootDisplay> BootDisplay for DesktopMirror<'_, D> {
    /// Возвращает статистику последней отрисовки и передачи кадра.
    fn render_stats(&self) -> crate::display::RenderStats {
        self.inner.render_stats()
    }
    /// Принимает подготовленный RGB565-буфер пользовательских обоев.
    fn set_wallpaper(&mut self, pixels: alloc::vec::Vec<u16>) {
        self.inner.set_wallpaper(pixels);
    }
    /// Возвращает доступный снимок экрана в формате RGB565.
    fn snapshot_rgb565(&self) -> Option<&[u16]> {
        self.inner.snapshot_rgb565()
    }
    /// Рисует меню выбора режима загрузки.
    fn boot_menu(&mut self, kind: u8, cursor: u8) -> Result<(), DisplayError> {
        self.inner.boot_menu(kind, cursor)
    }
    /// Показывает стартовый логотип.
    fn boot_logo(&mut self) -> Result<(), DisplayError> {
        let result = self.inner.boot_logo();
        self.send_parts(&["@ZWUI", "BOOT"]);
        result
    }

    /// Показывает этап и прогресс загрузки.
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

    /// Выводит строку состояния или диагностический статус.
    fn status(&mut self, title: &str, l1: &str, l2: &str, l3: &str) -> Result<(), DisplayError> {
        let result = self.inner.status(title, l1, l2, l3);
        self.send_parts(&["@ZWUI", "STATUS", title, l1, l2, l3]);
        result
    }

    /// Выводит кадр runtime на выбранный backend.
    fn runtime(&mut self, frame: &RuntimeFrame) -> Result<(), DisplayError> {
        let result = self.inner.runtime(frame);
        if !self.stream.active(
            esp_hal::time::Instant::now()
                .duration_since_epoch()
                .as_millis(),
        ) {
            return result;
        }
        match frame {
            RuntimeFrame::Legacy(f) => {
                self.send_parts(&["@ZWUI", "STATUS", &f.title, &f.line1, &f.line2, &f.line3])
            }
            RuntimeFrame::Material(f) => {
                let mut line = heapless::String::<256>::new();
                let _ = core::fmt::write(
                    &mut line,
                    format_args!(
                        "@ZWUI|M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
                        f.screen,
                        f.cursor,
                        f.brightness,
                        if f.dnd { 1 } else { 0 },
                        if f.airplane { 1 } else { 0 },
                        f.theme,
                        if f.locked { 1 } else { 0 },
                        if f.wifi { 1 } else { 0 },
                        if f.bt { 1 } else { 0 },
                        if f.adb { 1 } else { 0 },
                        f.notes,
                        f.time_minutes,
                        f.swap_pages,
                        f.timer_secs,
                        f.stopwatch_secs,
                        if f.alarm_enabled { 1 } else { 0 },
                        if f.auto_time { 1 } else { 0 },
                        if f.time_valid { 1 } else { 0 },
                        f.timezone_hours,
                        f.wifi_ap_count,
                        f.wifi_ap_index,
                        f.wifi_ap_rssi,
                        f.wifi_password_len,
                        f.wifi_editor_char,
                        f.wifi_link_state,
                    ),
                );
                for &byte in &f.wifi_ap_ssid[..f.wifi_ap_ssid_len as usize] {
                    let _ = core::fmt::write(&mut line, format_args!("{:02x}", byte));
                }
                if self.stream.changed(line.as_bytes()) {
                    self.pump();
                    self.pending.enqueue(&[line.as_str()]);
                }
                self.pump();
            }
        }
        result
    }

    /// Показывает диагностический экран ошибки.
    fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError> {
        let result = self.inner.error(code, detail);
        self.send_parts(&["@ZWUI", "ERROR", code, detail]);
        result
    }
}
