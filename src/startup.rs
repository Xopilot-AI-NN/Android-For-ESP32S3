// Модуль экрана запуска.
//! Readiness gate: the same logo stays active until resources and radio finish starting.
use core::cell::RefCell;
use critical_section::Mutex;
/// Параметр START компонента экрана запуска.
static START: Mutex<RefCell<Option<u64>>> = Mutex::new(RefCell::new(None));
static ANIMATION: Mutex<RefCell<Option<u64>>> = Mutex::new(RefCell::new(None));

/// One monotonic fade across bootloader and runtime; no restart or looping.
pub fn animation_phase() -> u8 {
    let time = now();
    critical_section::with(|cs| {
        let mut start = ANIMATION.borrow_ref_mut(cs);
        let began = *start.get_or_insert(time);
        (time.saturating_sub(began) / 50).min(31) as u8
    })
}
/// Возвращает текущий отсчёт монотонных часов.
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis() as u64
}
/// Запоминает начало анимации; тайм-аут не подменяет готовность служб.
pub fn begin() {
    critical_section::with(|cs| *START.borrow_ref_mut(cs) = Some(now()));
}
/// Сообщает, активен ли соответствующий режим.
pub fn active() -> bool {
    critical_section::with(|cs| START.borrow_ref(cs).is_some())
}
/// Выбирает кадр анимации и снимает блокировку ввода только после готовности.
pub fn phase(resources_pending: bool, bluetooth_starting: bool) -> u8 {
    let animation = animation_phase();
    critical_section::with(|cs| {
        let mut start = START.borrow_ref_mut(cs);
        let Some(_) = *start else {
            return 0;
        };
        // Завершаем только по готовности; ошибка радио — терминальное состояние, не starting.
        if !resources_pending && !bluetooth_starting {
            *start = None;
            return 0;
        }
        1 + animation
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fade_finishes_once_and_runtime_handoff_does_not_restart_it() {
        critical_section::with(|cs| *ANIMATION.borrow_ref_mut(cs) = None);
        crate::time::set_ms(100);
        assert_eq!(animation_phase(), 0);
        crate::time::set_ms(850);
        assert_eq!(animation_phase(), 15);
        crate::time::set_ms(1700);
        assert_eq!(animation_phase(), 31);
        begin();
        assert_eq!(phase(true, true), 32);
        crate::time::set_ms(90_000);
        assert_eq!(animation_phase(), 31);
        assert_eq!(phase(false, false), 0);
    }
    #[test]
    fn logo_waits_for_both_services_without_an_arbitrary_timeout() {
        crate::time::set_ms(0);
        begin();
        assert_ne!(phase(true, false), 0);
        crate::time::set_ms(120_000);
        assert_ne!(phase(false, true), 0);
        assert!(active());
        assert_eq!(phase(false, false), 0);
        assert!(!active());
    }
}
