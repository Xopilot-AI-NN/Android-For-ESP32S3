//! SMS previews from an explicitly approved KDE Connect bridge; no Google password is involved.
use crate::{
    apps::remote::{View, wrap},
    runtime::RuntimeEvent,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::cell::RefCell;
use critical_section::Mutex;
struct Message {
    sender: String,
    body: String,
    date: String,
}
struct Inbox {
    available: bool,
    messages: Vec<Message>,
    staging: Vec<u8>,
    expected: usize,
    revision: u32,
}
static INBOX: Mutex<RefCell<Inbox>> = Mutex::new(RefCell::new(Inbox {
    available: false,
    messages: Vec::new(),
    staging: Vec::new(),
    expected: 0,
    revision: 0,
}));
pub fn clear() {
    critical_section::with(|cs| {
        let mut inbox = INBOX.borrow_ref_mut(cs);
        inbox.available = false;
        inbox.messages.clear();
        inbox.staging = Vec::new();
        inbox.expected = 0;
        inbox.revision = inbox.revision.wrapping_add(1);
    });
}
/// The companion service checks the bridge session and PIN before entering here.
pub fn receive(op: &str, args: &mut core::str::SplitWhitespace<'_>) -> bool {
    critical_section::with(|cs| {
        let mut inbox = INBOX.borrow_ref_mut(cs);
        match op {
            "sms-begin" => {
                let Some(length) = args
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| *n <= 8192)
                else {
                    return false;
                };
                inbox.staging = Vec::new();
                inbox.expected = 0;
                if inbox.staging.try_reserve_exact(length).is_err() {
                    return false;
                }
                inbox.expected = length;
                true
            }
            "sms-data" => {
                let Some(offset) = args.next().and_then(|s| s.parse::<usize>().ok()) else {
                    return false;
                };
                let Some(hex) = args.next().filter(|s| s.len() <= 32 && s.len() % 2 == 0) else {
                    return false;
                };
                if offset != inbox.staging.len() || offset + hex.len() / 2 > inbox.expected {
                    return false;
                }
                // A transport chunk is at most 16 bytes. Decode on the stack
                // rather than allocating a temporary Vec for every command.
                let mut bytes = [0u8; 16];
                for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                    let (Some(hi), Some(lo)) = (
                        (pair[0] as char).to_digit(16),
                        (pair[1] as char).to_digit(16),
                    ) else {
                        return false;
                    };
                    bytes[index] = (hi * 16 + lo) as u8;
                }
                inbox.staging.extend_from_slice(&bytes[..hex.len() / 2]);
                true
            }
            "sms-end" => {
                if inbox.staging.len() != inbox.expected || inbox.expected == 0 {
                    return false;
                }
                let Ok(value) = serde_json::from_slice::<serde_json::Value>(&inbox.staging) else {
                    inbox.staging = Vec::new();
                    inbox.expected = 0;
                    return false;
                };
                let Some(rows) = value["messages"].as_array().filter(|rows| rows.len() <= 12)
                else {
                    inbox.staging = Vec::new();
                    inbox.expected = 0;
                    return false;
                };
                inbox.available = value["available"].as_bool().unwrap_or(true);
                inbox.messages = rows
                    .iter()
                    .map(|row| Message {
                        sender: row["sender"]
                            .as_str()
                            .unwrap_or("Неизвестный номер")
                            .chars()
                            .take(64)
                            .collect(),
                        body: row["body"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(512)
                            .collect(),
                        date: row["date"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(32)
                            .collect(),
                    })
                    .collect();
                inbox.staging = Vec::new();
                inbox.expected = 0;
                inbox.revision = inbox.revision.wrapping_add(1);
                true
            }
            _ => false,
        }
    })
}
#[derive(Default)]
pub struct Controller {
    cursor: usize,
    scroll: usize,
    detail: bool,
}
impl Controller {
    pub fn open(&mut self) {
        self.cursor = 0;
        self.scroll = 0;
        self.detail = false;
    }
    pub fn back(&mut self) -> bool {
        if self.detail {
            self.detail = false;
            true
        } else {
            false
        }
    }
    pub fn event(&mut self, event: RuntimeEvent) {
        critical_section::with(|cs| {
            let inbox = INBOX.borrow_ref(cs);
            self.cursor = self.cursor.min(inbox.messages.len().saturating_sub(1));
            match event {
                RuntimeEvent::Rotate(d) if self.detail => {
                    if let Some(row) = inbox.messages.get(self.cursor) {
                        let lines = wrap(&alloc::format!(
                            "{}\n{}\n\n{}",
                            row.sender,
                            row.date,
                            row.body
                        ));
                        self.scroll = (self.scroll as i32 + d)
                            .clamp(0, lines.len().saturating_sub(7) as i32)
                            as usize;
                    }
                }
                RuntimeEvent::Rotate(d) => {
                    self.cursor = (self.cursor as i32 + d)
                        .clamp(0, inbox.messages.len().saturating_sub(1) as i32)
                        as usize
                }
                RuntimeEvent::Press if !inbox.messages.is_empty() => {
                    self.detail = true;
                    self.scroll = 0;
                }
                _ => {}
            }
        });
    }
    pub fn view(&self) -> View {
        critical_section::with(|cs| {
            let inbox = INBOX.borrow_ref(cs);
            let bridge = crate::companion::view();
            let mut view = View {
                kind: 6,
                mode: if self.detail { 4 } else { 2 },
                cursor: self.cursor as u16,
                count: inbox.messages.len() as u16,
                revision: inbox.revision,
                message: if bridge.online && !inbox.available {
                    "Разрешите SMS в KDE Connect"
                } else if bridge.online {
                    "SMS · синхронизация с телефоном"
                } else {
                    "Подключите Телефон / KDE"
                },
                ..View::default()
            };
            let (lines, first) = if self.detail {
                let lines = inbox
                    .messages
                    .get(self.cursor)
                    .map(|row| {
                        wrap(&alloc::format!(
                            "{}\n{}\n\n{}",
                            row.sender,
                            row.date,
                            row.body
                        ))
                    })
                    .unwrap_or_default();
                (lines, self.scroll)
            } else {
                let lines = inbox
                    .messages
                    .iter()
                    .flat_map(|row| {
                        [
                            row.sender.clone(),
                            row.body.lines().next().unwrap_or("").to_string(),
                        ]
                    })
                    .collect::<Vec<_>>();
                (lines, self.cursor.saturating_sub(1) * 2)
            };
            view.offset = first as u16;
            view.total_lines = lines.len() as u16;
            for (i, line) in lines.iter().skip(first).take(7).enumerate() {
                view.lengths[i] = crate::apps::files::copy_text(&mut view.lines[i], line);
            }
            view
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deliver(command: &str) -> bool {
        let mut args = command.split_whitespace();
        receive(args.next().unwrap(), &mut args)
    }

    #[test]
    fn invalid_hex_is_atomic_and_completed_batches_release_staging_memory() {
        clear();
        let payload = br#"{"available":true,"messages":[]}"#;
        assert!(deliver(&alloc::format!("sms-begin {}", payload.len())));
        assert!(!deliver("sms-data 0 4178zz"));
        critical_section::with(|cs| assert!(INBOX.borrow_ref(cs).staging.is_empty()));
        for (index, chunk) in payload.chunks(16).enumerate() {
            let hex: String = chunk.iter().map(|b| alloc::format!("{b:02x}")).collect();
            assert!(deliver(&alloc::format!("sms-data {} {hex}", index * 16)));
        }
        assert!(deliver("sms-end"));
        critical_section::with(|cs| {
            let inbox = INBOX.borrow_ref(cs);
            assert!(inbox.available);
            assert_eq!(inbox.staging.capacity(), 0);
            assert_eq!(inbox.expected, 0);
        });
        clear();
    }
}
