//! Аварийный сброс независимо от UI, USB-диска, SD и интерпретатора.
//! TIMG1 зарезервирован для этого обработчика; RTOS продолжает пользоваться TIMG0.
use core::cell::RefCell;
use critical_section::Mutex;
use esp_hal::{Blocking, time::{Duration, Instant}, timer::{PeriodicTimer, timg::TimerGroup}};
#[path = "emergency_policy.rs"]
mod policy;
/// Единственный владелец таймера; ISR только подтверждает прерывание и читает GPIO13.
static TIMER: Mutex<RefCell<Option<PeriodicTimer<'static, Blocking>>>> = Mutex::new(RefCell::new(None));
/// Модель непрерывного удержания принадлежит исключительно ISR.
static HOLD: Mutex<RefCell<policy::HoldReset>> = Mutex::new(RefCell::new(policy::HoldReset::new()));
/// Подключает независимое прерывание после настройки входа GPIO13 и до запуска служб.
pub fn install(peripheral: esp_hal::peripherals::TIMG1<'static>) {
    let group = TimerGroup::new(peripheral);
    let mut timer = PeriodicTimer::new(group.timer0);
    timer.set_interrupt_handler(tick);
    critical_section::with(|cs| {
        timer.start(Duration::from_millis(10)).expect("emergency timer period");
        timer.listen();
        TIMER.borrow_ref_mut(cs).replace(timer);
    });
}
/// Не выделяет память, не печатает в UART и не вызывает обработчики приложений.
#[esp_hal::handler]
fn tick() {
    let reset = critical_section::with(|cs| {
        if let Some(timer) = TIMER.borrow_ref_mut(cs).as_mut() { timer.clear_interrupt(); }
        // Read-only чтение регистра входов: GPIO13 по-прежнему принадлежит BootKeys.
        // Не создаём второй GPIO-драйвер и не меняем mux/подтяжку из прерывания.
        let pressed = esp_hal::peripherals::GPIO::regs().in_().read().bits() & (1 << 13) == 0;
        let now = Instant::now().duration_since_epoch().as_millis();
        HOLD.borrow_ref_mut(cs).poll(pressed, now)
    });
    if reset { esp_hal::system::software_reset(); }
}
