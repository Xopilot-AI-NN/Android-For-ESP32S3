// Модуль локальной блокировки PIN.
//! Local PIN gate. This is not secure boot or protection from reflashing/removing storage.
use super::RuntimeEvent;
use sha2::{Digest, Sha256};

/// PIN-гейт, временный ввод и защитные задержки компонента локальной блокировки PIN.
pub struct Security {
    /// Признак локальной PIN-блокировки интерфейса.
    pub locked: bool,
    /// Признак настроенного PIN.
    pub has_pin: bool,
    /// Настроенный срок ожидания/блокировки.
    pub timeout: u16,
    /// Текущий шаг ввода/подтверждения PIN или настройки защитного срока.
    pub view: u8, // 0 settings, 1 new PIN, 2 confirm, 3 unlock, 4 change, 5 timeout, 6 disable
    /// Текущая выбранная цифра PIN.
    pub digit: u8,
    /// Количество элементов или выполненных действий в текущем контексте.
    pub count: u8,
    /// Сообщение или его код для вывода пользователю.
    pub message: u8,
    pub storage: bool,
    /// Входные данные или временно введённые символы.
    input: [u8; 4],
    /// Первая копия ввода для проверки повторного подтверждения.
    first: [u8; 4],
    /// Случайная соль для хеша локального PIN.
    salt: [u8; 16],
    /// Сохранённый хеш PIN; открытый PIN здесь не хранится.
    hash: [u8; 32],
    entropy: Option<[u8; 16]>,
    /// Последовательный порт действующего подключения.
    serial: u32,
    failures: u8,
    blocked_until: u64,
    last_activity: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Вводит заданные PIN-цифры через события в тестовом сценарии.
    fn digits(s: &mut Security, digits: [u8; 4], now: u64) {
        for digit in digits {
            s.digit = digit;
            s.event(RuntimeEvent::Press, now);
        }
    }
    /// Создаёт тестовую конфигурацию локального PIN.
    fn configured() -> Security {
        let mut s = Security::new(0);
        s.storage = true;
        s.entropy([17; 16]);
        s.begin(1);
        digits(&mut s, [1, 2, 3, 4], 0);
        assert_eq!(s.view, 2);
        digits(&mut s, [1, 2, 3, 4], 0);
        assert!(s.has_pin);
        s
    }
    #[test]
    /// Проверяет подтверждение PIN, восстановление и невозможность обхода защитной задержки.
    fn confirmation_restore_and_lockout_cannot_be_bypassed() {
        let mut s = configured();
        let raw = s.encode();
        assert!(raw[24..56].iter().any(|&b| b != 0));
        let mut restored = Security::new(100);
        assert!(restored.restore(&raw, 100));
        assert!(restored.locked);
        restored.event(RuntimeEvent::Home, 100);
        assert!(restored.locked);
        for _ in 0..5 {
            digits(&mut restored, [0, 0, 0, 0], 100);
        }
        assert_eq!(restored.remaining(100), 30);
        digits(&mut restored, [1, 2, 3, 4], 101);
        assert!(restored.locked);
        let raw = restored.encode();
        s.restore(&raw, 200);
        assert_eq!(s.remaining(200), 30);
        s.tick(231);
        digits(&mut s, [1, 2, 3, 4], 231);
        assert!(!s.locked);
    }
    #[test]
    /// Проверяет сроки автоблокировки и отказ при несовпадении подтверждения PIN.
    fn all_idle_timeouts_and_mismatched_confirmation() {
        let mut s = Security::new(0);
        s.storage = true;
        s.entropy([5; 16]);
        s.begin(1);
        digits(&mut s, [1, 2, 3, 4], 0);
        digits(&mut s, [4, 3, 2, 1], 0);
        assert!(!s.has_pin);
        for timeout in [30, 60, 120, 180, 240, 300, 360, 420, 480, 540, 600] {
            let mut s = configured();
            s.timeout = timeout;
            s.activity(10);
            s.tick(10 + u64::from(timeout) - 1);
            assert!(!s.locked);
            s.tick(10 + u64::from(timeout));
            assert!(s.locked);
        }
    }
}
impl Security {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(now: u64) -> Self {
        Self {
            locked: false,
            has_pin: false,
            timeout: 30,
            view: 0,
            digit: 0,
            count: 0,
            message: 0,
            storage: false,
            input: [0; 4],
            first: [0; 4],
            salt: [0; 16],
            hash: [0; 32],
            entropy: None,
            serial: 0,
            failures: 0,
            blocked_until: 0,
            last_activity: now,
        }
    }
    /// Обновляет исходную энтропию для соли PIN.
    pub fn entropy(&mut self, bytes: [u8; 16]) {
        self.entropy = Some(bytes);
    }
    /// Отмечает пользовательскую активность для срока блокировки.
    pub fn activity(&mut self, now: u64) {
        self.last_activity = now;
    }
    /// Продвигает состояние сервиса по монотонному времени.
    pub fn tick(&mut self, now: u64) {
        if self.blocked_until != 0 && now >= self.blocked_until {
            self.blocked_until = 0;
            self.message = 0;
        }
        if self.has_pin
            && !self.locked
            && now.saturating_sub(self.last_activity) >= u64::from(self.timeout)
        {
            self.lock();
        }
    }
    /// Возвращает оставшееся время защитного ограничения.
    pub fn remaining(&self, now: u64) -> u16 {
        self.blocked_until.saturating_sub(now).min(30) as u16
    }
    /// Стирает временно введённые цифры PIN.
    fn clear(&mut self) {
        self.input.fill(0);
        self.count = 0;
        self.digit = 0;
    }
    /// Блокирует интерфейс и очищает временный ввод PIN.
    pub fn lock(&mut self) {
        if self.has_pin {
            self.first.fill(0);
            self.locked = true;
            self.view = 3;
            self.message = 0;
            self.clear();
        } else {
            self.begin(1);
        }
    }
    /// Выбирает операцию PIN и очищает предыдущий незавершённый ввод.
    pub fn begin(&mut self, view: u8) {
        self.clear();
        self.message = 0;
        if view == 1 && (!self.storage || self.entropy.is_none()) {
            self.message = 4;
            return;
        }
        self.view = view;
        if view == 5 {
            self.digit = if self.timeout == 30 {
                0
            } else {
                (self.timeout / 60) as u8
            };
        }
    }
    /// Вычисляет хеш PIN с сохранённой солью.
    fn derive(pin: &[u8; 4], salt: &[u8; 16]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"watch-pin-v1");
        h.update(salt);
        h.update(pin);
        let mut hash: [u8; 32] = h.finalize().into();
        for _ in 0..4096 {
            let mut h = Sha256::new();
            h.update(hash);
            h.update(salt);
            hash = h.finalize().into();
        }
        hash
    }
    /// Сравнивает введённый PIN с хешем, вычисленным с сохранённой солью.
    fn matches(&self) -> bool {
        let candidate = Self::derive(&self.input, &self.salt);
        candidate
            .iter()
            .zip(self.hash.iter())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    }
    /// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
    pub fn event(&mut self, event: RuntimeEvent, now: u64) {
        if self.view == 5 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.digit = (i32::from(self.digit) + d).rem_euclid(11) as u8
                }
                RuntimeEvent::Press => {
                    self.timeout = if self.digit == 0 {
                        30
                    } else {
                        u16::from(self.digit) * 60
                    };
                    self.view = 0;
                }
                RuntimeEvent::Back | RuntimeEvent::Home => self.view = 0,
                _ => {}
            }
            return;
        }
        match event {
            RuntimeEvent::Rotate(d) => {
                self.digit = (i32::from(self.digit) + d).rem_euclid(10) as u8
            }
            RuntimeEvent::Back | RuntimeEvent::Home => {
                if self.count > 0 && event == RuntimeEvent::Back {
                    self.count -= 1;
                    self.input[self.count as usize] = 0;
                } else {
                    self.clear();
                    self.first.fill(0);
                    if !self.locked {
                        self.view = 0;
                    }
                }
            }
            RuntimeEvent::Press if self.remaining(now) == 0 => {
                self.input[self.count as usize] = self.digit;
                self.count += 1;
                self.digit = 0;
                if self.count == 4 {
                    match self.view {
                        1 => {
                            self.first = self.input;
                            self.view = 2;
                            self.clear();
                        }
                        2 => {
                            if self.first == self.input {
                                self.serial = self.serial.wrapping_add(1);
                                let mut salt = Sha256::new();
                                salt.update(self.entropy.unwrap_or([0; 16]));
                                salt.update(now.to_le_bytes());
                                salt.update(self.serial.to_le_bytes());
                                self.salt.copy_from_slice(&salt.finalize()[..16]);
                                self.hash = Self::derive(&self.input, &self.salt);
                                self.has_pin = true;
                                self.view = 0;
                                self.message = 5;
                                self.last_activity = now;
                            } else {
                                self.view = 1;
                                self.message = 2;
                            }
                            self.first.fill(0);
                            self.clear();
                        }
                        3 | 4 | 6 => {
                            if self.matches() {
                                self.failures = 0;
                                self.blocked_until = 0;
                                self.message = 0;
                                self.last_activity = now;
                                if self.view == 3 {
                                    self.locked = false;
                                    self.view = 0;
                                } else if self.view == 4 {
                                    self.begin(1);
                                } else {
                                    self.has_pin = false;
                                    self.hash.fill(0);
                                    self.salt.fill(0);
                                    self.view = 0;
                                    self.message = 6;
                                }
                            } else {
                                self.failures += 1;
                                self.message = 1;
                                if self.failures >= 5 {
                                    self.blocked_until = now + 30;
                                    self.failures = 0;
                                    self.message = 3;
                                }
                            }
                            self.clear();
                        }
                        _ => self.clear(),
                    }
                }
            }
            _ => {}
        }
    }
    /// Преобразует состояние в формат хранения или передачи.
    pub fn encode(&self) -> [u8; 58] {
        let mut out = [0; 58];
        out[..4].copy_from_slice(b"WSEC");
        out[4] = 1;
        out[5] = u8::from(self.has_pin);
        out[6..8].copy_from_slice(&self.timeout.to_le_bytes());
        out[8..24].copy_from_slice(&self.salt);
        out[24..56].copy_from_slice(&self.hash);
        out[56] = self.failures;
        out[57] = u8::from(self.blocked_until != 0);
        out
    }
    /// Проверяет и восстанавливает версионированное состояние.
    pub fn restore(&mut self, bytes: &[u8], now: u64) -> bool {
        if bytes.len() != 58
            || &bytes[..4] != b"WSEC"
            || bytes[4] != 1
            || bytes[5] > 1
            || bytes[56] > 4
            || bytes[57] > 1
        {
            return false;
        }
        let timeout = u16::from_le_bytes([bytes[6], bytes[7]]);
        if timeout != 30 && !(timeout >= 60 && timeout <= 600 && timeout % 60 == 0) {
            return false;
        }
        self.timeout = timeout;
        self.has_pin = bytes[5] != 0;
        self.salt.copy_from_slice(&bytes[8..24]);
        self.hash.copy_from_slice(&bytes[24..56]);
        self.failures = bytes[56];
        self.blocked_until = if bytes[57] != 0 { now + 30 } else { 0 };
        self.last_activity = now;
        if self.has_pin {
            self.lock();
        }
        true
    }
}
