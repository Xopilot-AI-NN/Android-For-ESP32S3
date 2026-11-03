// Модуль экрана запуска.
//! Short startup gate; local resource jobs finish before normal input is accepted.
use core::cell::RefCell;
use critical_section::Mutex;
/// Параметр START компонента экрана запуска.
static START: Mutex<RefCell<Option<u64>>> = Mutex::new(RefCell::new(None));
/// Возвращает текущий отсчёт монотонных часов.
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis() as u64
}
/// Запоминает начало ограниченного ожидания готовности интерфейса.
pub fn begin() {
    critical_section::with(|cs| *START.borrow_ref_mut(cs) = Some(now()));
}
/// Сообщает, активен ли соответствующий режим.
pub fn active() -> bool {
    critical_section::with(|cs| START.borrow_ref(cs).is_some())
}
/// Определяет текущий этап запуска и завершает ограниченное ожидание.
pub fn phase(resources_pending: bool, bluetooth_starting: bool) -> u8 {
    critical_section::with(|cs| {
        let mut start = START.borrow_ref_mut(cs);
        let Some(began) = *start else {
            return 0;
        };
        let elapsed = now().saturating_sub(began);
        // A failed optional radio must not keep the watch on a splash forever.
        if elapsed >= 8000 || (elapsed >= 2000 && !resources_pending && !bluetooth_starting) {
            *start = None;
            return 0;
        }
        if resources_pending {
            2
        } else if bluetooth_starting {
            3
        } else {
            1
        }
    })
}
