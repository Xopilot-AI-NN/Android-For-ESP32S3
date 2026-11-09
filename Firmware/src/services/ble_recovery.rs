// Модуль ограниченного восстановления BLE-host после таймаута или ошибки.
//! Ограничение повторов запуска BLE. Новый запрос пользователя снимает блокировку.
#[derive(Default)]
pub struct Recovery {
    /// Число последовательных неудачных запусков BLE в текущей серии.
    attempts: u8,
    /// Поколение состояния, позволяющее отличить новую версию от старой.
    generation: u32,
}
impl Recovery {
    /// После нового пользовательского запроса разрешает новую серию из трёх попыток.
    pub fn allowed(&mut self, generation: u32) -> bool {
        if self.generation != generation {
            self.generation = generation;
            self.attempts = 0;
        }
        self.attempts < 3
    }
    /// Успешный запуск рекламы начинает отсчёт последовательных сбоев заново.
    pub fn stopped(&mut self, was_ready: bool) -> u8 {
        if was_ready { self.attempts = 0; }
        self.attempts = self.attempts.saturating_add(1);
        self.attempts
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет предел повторов BLE и разрешение новой серии по запросу пользователя.
    fn failed_start_is_bounded_and_pair_request_reenables_recovery() {
        let mut policy = Recovery::default();
        for attempt in 1..=3 { assert!(policy.allowed(0)); assert_eq!(policy.stopped(false), attempt); }
        assert!(!policy.allowed(0));
        assert!(policy.allowed(1));
        assert_eq!(policy.stopped(false), 1);
        assert_eq!(policy.stopped(true), 1);
        assert!(policy.allowed(1));
    }
}
