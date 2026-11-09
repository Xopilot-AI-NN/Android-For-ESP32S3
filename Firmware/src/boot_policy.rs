//! Выбор сервисного режима не зависит от наличия SD/PC-диска или пользовательских настроек.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootMode {
    /// Продолжить обычную загрузку.
    Normal,
    /// Открыть сервисный USB-интерфейс без монтирования системного диска.
    Fastboot,
    /// Запустить встроенное восстановление из внутренней flash.
    Recovery,
}
/// Положительный шаг — Fastboot, отрицательный — Recovery; ноль не меняет выбор.
pub fn direction(step: i32) -> BootMode {
    match step.cmp(&0) {
        core::cmp::Ordering::Greater => BootMode::Fastboot,
        core::cmp::Ordering::Less => BootMode::Recovery,
        core::cmp::Ordering::Equal => BootMode::Normal,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opposite_directions_select_opposite_modes_even_for_many_steps() {
        for value in [1, 2, 100, i32::MAX] {
            assert_eq!(direction(value), BootMode::Fastboot);
            assert_eq!(direction(-value), BootMode::Recovery);
        }
        assert_eq!(direction(i32::MIN), BootMode::Recovery);
        assert_eq!(direction(0), BootMode::Normal);
    }
}
