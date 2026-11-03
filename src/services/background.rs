// Модуль ожидания между опросами ввода.
//! Yield the main task between input polls instead of spinning at 240 MHz.
pub fn wait(delay: &esp_hal::delay::Delay) {
    #[cfg(feature = "radio-services")]
    {
        let _ = delay;
        // boot::run starts the scheduler before entering either runtime loop.
        esp_rtos::CurrentThreadHandle::get().delay(esp_hal::time::Duration::from_millis(20));
    }
    #[cfg(not(feature = "radio-services"))]
    delay.delay_millis(20);
}
