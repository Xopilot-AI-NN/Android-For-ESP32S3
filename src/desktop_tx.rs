// Модуль очереди вывода в настольный просмотрщик.
//! Bounded viewer output: retry a full USB FIFO without losing packet bytes.
pub struct PacketQueue {
    /// Текущее количество байтов пользовательского ввода.
    bytes: heapless::Deque<u8, 1024>,
}
impl PacketQueue {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub const fn new() -> Self {
        Self {
            bytes: heapless::Deque::new(),
        }
    }
    /// Admit a complete line or drop it entirely; never enqueue half a message.
    pub fn enqueue(&mut self, parts: &[&str]) -> bool {
        let size = parts.iter().map(|p| p.len()).sum::<usize>() + parts.len().saturating_sub(1) + 2;
        if size > self.bytes.capacity() - self.bytes.len() {
            return false;
        }
        for (i, part) in parts.iter().enumerate() {
            if i != 0 {
                let _ = self.bytes.push_back(b'|');
            }
            for &b in part.as_bytes() {
                let _ = self.bytes.push_back(b);
            }
        }
        let _ = self.bytes.push_back(b'\r');
        let _ = self.bytes.push_back(b'\n');
        true
    }
    /// One FIFO-sized batch per call; a disconnected host cannot stall input.
    pub fn pump(&mut self, mut write: impl FnMut(u8) -> bool) {
        for _ in 0..64 {
            let Some(&byte) = self.bytes.front() else {
                break;
            };
            if !write(byte) {
                break;
            }
            self.bytes.pop_front();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет повтор того же байта при полном FIFO и сохранение границ строк.
    fn full_fifo_retries_the_same_byte_and_preserves_line_boundaries() {
        let mut q = PacketQueue::new();
        assert!(q.enqueue(&["@ZWUI", "M3", &"x".repeat(90)]));
        let mut received = std::vec::Vec::new();
        q.pump(|b| {
            if received.len() == 17 {
                return false;
            }
            received.push(b);
            true
        });
        q.pump(|_| false);
        assert!(q.enqueue(&["@ZADB", "OUT", "OK"]));
        for _ in 0..4 {
            q.pump(|b| {
                received.push(b);
                true
            });
        }
        assert_eq!(
            String::from_utf8(received).unwrap(),
            format!("@ZWUI|M3|{}\r\n@ZADB|OUT|OK\r\n", "x".repeat(90))
        );
    }
    #[test]
    /// Проверяет отбрасывание целого сообщения при переполнении без порчи ожидающих данных.
    fn overflow_drops_whole_message_without_corrupting_pending_data() {
        let mut q = PacketQueue::new();
        assert!(q.enqueue(&[&"a".repeat(1000)]));
        assert!(!q.enqueue(&[&"b".repeat(40)]));
        let mut got = std::vec::Vec::new();
        for _ in 0..20 {
            q.pump(|b| {
                got.push(b);
                true
            });
        }
        assert_eq!(got, format!("{}\r\n", "a".repeat(1000)).as_bytes());
    }
}
