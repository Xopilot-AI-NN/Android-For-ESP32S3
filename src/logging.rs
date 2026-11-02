//! Bounded in-device log history shared by serial output and wireless logcat.
use core::{
    cell::RefCell,
    fmt::{self, Write},
};
use critical_section::Mutex;
use heapless::String;
const CAPACITY: usize = 32;

#[derive(Clone)]
pub struct Record {
    pub sequence: u64,
    pub uptime_ms: u64,
    pub month: u8,
    pub day: u8,
    pub minutes: u16,
    pub seconds: u8,
    pub priority: u8,
    pub tag: String<24>,
    pub message: String<160>,
}
impl Record {
    const fn empty() -> Self {
        Self {
            sequence: 0,
            uptime_ms: 0,
            month: 0,
            day: 0,
            minutes: 0,
            seconds: 0,
            priority: b'I',
            tag: String::new(),
            message: String::new(),
        }
    }
}
struct Ring {
    records: [Record; CAPACITY],
    next: u64,
    len: usize,
}
static RING: Mutex<RefCell<Ring>> = Mutex::new(RefCell::new(Ring {
    records: [const { Record::empty() }; CAPACITY],
    next: 1,
    len: 0,
}));

#[macro_export]
macro_rules! watch_println {
    ($($arg:tt)*) => { $crate::logging::emit(core::format_args!($($arg)*)) };
}

pub fn emit(args: fmt::Arguments<'_>) {
    // Formatting occurs outside the critical section. Record size and count
    // are fixed, so opening logcat never allocates an unbounded log buffer.
    let mut line = String::<256>::new();
    let _ = line.write_fmt(args);
    // Keep complete serial diagnostics; only the in-device ring is truncated.
    esp_println::println!("{}", args);
    if line.starts_with('@') || line.is_empty() {
        return;
    }
    let mut record = Record::empty();
    record.uptime_ms = esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis();
    if let Some((day, month)) = crate::rtc::calendar() {
        record.day = day;
        record.month = month;
    }
    record.minutes = crate::rtc::local_minutes() as u16;
    record.seconds = crate::rtc::local_seconds();
    let mut text = line.as_str();
    if text.len() > 3 && b"VDIWEF".contains(&text.as_bytes()[0]) && text.as_bytes()[1] == b' ' {
        record.priority = text.as_bytes()[0];
        if let Some((_, tail)) = text.split_once(") ") {
            text = tail;
        }
    } else if text.contains("error") || text.contains("failed") || text.contains("panic") {
        record.priority = b'E';
    } else if text.contains("warning") || text.contains("timeout") {
        record.priority = b'W';
    }
    let (tag, message) = text
        .split_once(':')
        .filter(|(tag, _)| tag.len() <= 24)
        .unwrap_or(("Watch", text));
    let _ = record.tag.push_str(tag.trim());
    for ch in message.trim().chars() {
        if record.message.push(ch).is_err() {
            break;
        }
    }
    critical_section::with(|cs| {
        let mut ring = RING.borrow(cs).borrow_mut();
        record.sequence = ring.next;
        let index = ((ring.next - 1) % CAPACITY as u64) as usize;
        ring.records[index] = record;
        ring.next = ring.next.saturating_add(1);
        ring.len = (ring.len + 1).min(CAPACITY);
    });
}

pub fn bounds() -> (u64, u64) {
    critical_section::with(|cs| {
        let ring = RING.borrow(cs).borrow();
        (ring.next - ring.len as u64, ring.next)
    })
}
pub fn clear() {
    critical_section::with(|cs| RING.borrow(cs).borrow_mut().len = 0);
}
pub fn next(cursor: &mut u64) -> Option<Record> {
    critical_section::with(|cs| {
        let ring = RING.borrow(cs).borrow();
        *cursor = (*cursor).max(ring.next - ring.len as u64);
        if *cursor >= ring.next {
            return None;
        }
        let record = ring.records[(((*cursor) - 1) % CAPACITY as u64) as usize].clone();
        *cursor += 1;
        Some(record)
    })
}
