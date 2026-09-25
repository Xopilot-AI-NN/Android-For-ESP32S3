//! Rhai-движок для загрузчика.
//!
//! Предоставляемые API-функции в скриптах:
//!
//! ```rhai
//! // Управление ядром
//! core_set_max_freq(160);       // ограничить макс (80 / 160 / 240 МГц)
//! core_set_min_freq(80);
//! core_set_freq(160);           // принудительно
//! let mhz = core_get_freq();    // текущая частота в МГц
//!
//! // RTC
//! let ms = rtc_get_ms();        // 0–999
//! rtc_tick(50);                 // аванс на N ms
//!
//! // Диагностика
//! let thr = core_throttle_events();
//! let crit = core_critical_events();
//!
//! // Вывод в сериальный порт
//! print("hello from rhai");
//! ```
//!
//! Ограничения скриптов: числа только i32, нет массивов/объектов/float —
//! режим компиляции Rhai: no_std + only_i32 + no_float + no_index + no_object.

extern crate alloc;
use alloc::{string::String, format};
use rhai::Engine;

use crate::{
    vendor::rtc,
    xtensa_lx7_core_control::{self, CpuFreq},
};

// ── Сборка движка ──────────────────────────────────────────────────────

pub fn make_engine() -> Engine {
    let mut engine = Engine::new_raw();

    // Максимальное число операций за один вызов run_script.
    // Защищает от зависания процессора.
    engine.set_max_operations(50_000);

    // ─ Вывод ──────────────────────────────────────────────────
    engine.register_fn("print", |s: &str| {
        esp_println::println!("[rhai] {s}");
    });
    engine.register_fn("print_int", |n: i32| {
        esp_println::println!("[rhai] {n}");
    });

    // ─ Core control ─────────────────────────────────────────────

    // Текущая частота CPU в МГц (80 / 160 / 240)
    engine.register_fn("core_get_freq", || -> i32 {
        xtensa_lx7_core_control::current_freq().as_mhz() as i32
    });

    // Принудительно установить частоту. Значения: 80, 160, 240.
    engine.register_fn("core_set_freq", |mhz: i32| {
        xtensa_lx7_core_control::force_cpu_freq(CpuFreq::from_mhz(mhz as u32));
    });

    // Минимальная частота — тротлинг не опустится ниже.
    engine.register_fn("core_set_min_freq", |mhz: i32| {
        xtensa_lx7_core_control::set_min_freq(CpuFreq::from_mhz(mhz as u32));
    });

    // Максимальная частота — удобно для энергосбережения.
    engine.register_fn("core_set_max_freq", |mhz: i32| {
        xtensa_lx7_core_control::set_max_freq(CpuFreq::from_mhz(mhz as u32));
    });

    // Минимальная разрешённая частота (MHz)
    engine.register_fn("core_get_min_freq", || -> i32 {
        xtensa_lx7_core_control::min_freq().as_mhz() as i32
    });

    // Максимальная разрешённая частота (MHz)
    engine.register_fn("core_get_max_freq", || -> i32 {
        xtensa_lx7_core_control::max_freq().as_mhz() as i32
    });

    // Количество тротлинг-событий
    engine.register_fn("core_throttle_events", || -> i32 {
        xtensa_lx7_core_control::throttle_event_count() as i32
    });

    // Количество критических перегревов
    engine.register_fn("core_critical_events", || -> i32 {
        xtensa_lx7_core_control::critical_event_count() as i32
    });

    // ─ RTC ─────────────────────────────────────────────────────

    // Миллисекунды RTC (0–999)
    engine.register_fn("rtc_time_ms", || -> i32 {
        rtc::get_time().3 as i32
    });

    // Аванс RTC на N ms
    engine.register_fn("rtc_tick", |ms: i32| {
        if ms > 0 { rtc::rtc_tick_ms(ms as u32); }
    });

    engine
}

// ── Запуск скрипта ──────────────────────────────────────────────────────

/// Запустить Rhai-скрипт.
/// Возвращает `Ok(())` или строку с описанием ошибки.
pub fn run_script(source: &str) -> Result<(), String> {
    make_engine()
        .eval::<()>(source)
        .map_err(|e| format!("{e}"))
}