//! Чистая временная модель аварийной кнопки; используется ISR и host-тестами.
/// Непрерывное удержание должно продолжаться десять секунд; отпускание сбрасывает отсчёт.
#[derive(Default)]
pub struct HoldReset {
    /// Начало удержания по монотонным часам; None означает отпущенную кнопку.
    since: Option<u64>,
}
impl HoldReset {
    /// Создаёт пустой счётчик для статического состояния ISR.
    pub const fn new() -> Self { Self { since: None } }
    /// Проверяет срок без зависимости от основного цикла, дребезга меню и жестов.
    pub fn poll(&mut self, pressed: bool, now_ms: u64) -> bool {
        if !pressed { self.since = None; return false; }
        let start = *self.since.get_or_insert(now_ms);
        now_ms.saturating_sub(start) >= 10_000
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_occurs_while_held_and_release_requires_new_ten_seconds() {
        let mut hold = HoldReset::default();
        assert!(!hold.poll(true, 100));
        assert!(!hold.poll(true, 10_099));
        assert!(hold.poll(true, 10_100));
        assert!(!hold.poll(false, 10_101));
        assert!(!hold.poll(true, 30_000));
        assert!(!hold.poll(true, 39_999));
        assert!(hold.poll(true, 40_000));
    }
}
