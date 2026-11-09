// Модуль host-стенда реального runtime.
//! Compile the real Rhai runtime and software clocks on the host, substituting
//! only the ESP monotonic counter. Run with one test thread (shared clock state).
#![allow(dead_code)]
extern crate alloc;
#[path = "../../../src/hal/temperature.rs"]
mod temperature;
#[path = "../../../src/block_lines.rs"]
mod block_lines;
#[path = "../../../src/services/activity_pages.rs"]
pub mod activity_pages;

#[path = "../../../../Firmware/src/services/adb.rs"]
pub mod adb;
#[path = "../../../../Firmware/src/services/ble_recovery.rs"]
mod ble_recovery;
#[path = "../../../../Firmware/src/services/companion.rs"]
pub mod companion;
#[path = "../../../../Firmware/src/desktop_tx.rs"]
mod desktop_tx;
#[path = "../../../../Firmware/src/device_info.rs"]
mod device_info;
#[path = "../../../../Firmware/src/services/device_name.rs"]
mod device_name;
#[path = "../../../../Firmware/src/services/file_store.rs"]
pub mod file_store;
#[path = "../../../../Firmware/src/logging.rs"]
mod logging;
#[path = "../../../../Firmware/src/text_input.rs"]
mod text_input;
#[path = "../../../../Firmware/src/viewer_stream.rs"]
mod viewer_stream;
#[path = "../../../../Firmware/src/services/wallpaper.rs"]
mod wallpaper;
extern crate self as esp_hal;
extern crate self as esp_println;
pub use std::println;
pub mod time {
    use std::sync::atomic::{AtomicU64, Ordering};
    /// Параметр NOW компонента host-стенда реального runtime.
    static NOW: AtomicU64 = AtomicU64::new(0);
    /// Структура данных компонента host-стенда реального runtime.
    pub struct Instant;
    impl Instant {
        /// Возвращает текущий отсчёт монотонных часов.
        pub fn now() -> Self {
            Self
        }
        /// Возвращает прошедшее монотонное время в единицах host-стенда.
        pub fn duration_since_epoch(&self) -> Duration {
            Duration(NOW.load(Ordering::Relaxed))
        }
    }
    /// Структура данных компонента host-стенда реального runtime.
    pub struct Duration(u64);
    impl Duration {
        /// Возвращает длительность в миллисекундах.
        pub fn as_millis(&self) -> u64 {
            self.0
        }
        /// Возвращает длительность в секундах.
        pub fn as_secs(&self) -> u64 {
            self.0 / 1000
        }
        /// Возвращает длительность в микросекундах.
        pub fn as_micros(&self) -> u64 {
            self.0 * 1000
        }
    }
    /// Устанавливает искусственное монотонное время host-теста в секундах.
    pub fn set(seconds: u64) {
        set_ms(seconds * 1000);
    }
    /// Устанавливает искусственное монотонное время host-теста в миллисекундах.
    pub fn set_ms(milliseconds: u64) {
        NOW.store(milliseconds, Ordering::Relaxed);
    }
}
#[allow(unused_imports)]
#[path = "../../../../Firmware/src/config.rs"]
mod board_config;
#[path = "../../../../Firmware/src/hal/input/button.rs"]
mod button;
mod config {
    #[allow(unused_imports)]
    pub use super::board_config::*;
    pub const MAX_SCRIPT_BYTES: usize = 24 * 1024;
    pub const MAX_RUNTIME_BUNDLE_BYTES: usize = 32 * 1024;
}
#[path = "../../../../Firmware/src/hal/display.rs"]
mod display;
#[path = "../../../../Firmware/src/hal/input/encoder.rs"]
mod encoder;
#[path = "../../../../Firmware/src/font.rs"]
mod font;
pub use display::St7789Display;
#[path = "../../../../Bootloader/src/disk_probe.rs"]
mod disk_probe;
#[path = "../../../../Firmware/src/material.rs"]
pub mod material;
#[path = "../../../../App/system/app/WearMedia/controller.rs"]
mod media;
#[path = "../../../../Firmware/src/services/rtc.rs"]
mod rtc;
#[path = "../../../../Firmware/src/runtime.rs"]
mod runtime;
#[path = "../../../../Bootloader/src/sd_bitbang.rs"]
mod sd_bitbang;
#[path = "../../../../Bootloader/src/sd_init.rs"]
mod sd_init;
#[path = "../../../../Bootloader/src/sd_spi.rs"]
mod sd_spi;

#[path = "../../../../Bootloader/src/android_boot.rs"]
mod android_boot;
#[path = "../../../../Bootloader/src/android_storage.rs"]
mod android_storage;
#[path = "../../../../Bootloader/src/avb.rs"]
mod avb;
#[path = "../../../../Bootloader/src/boot_control.rs"]
pub mod boot_control;
#[path = "../../../../Bootloader/src/cpio.rs"]
mod cpio;
#[path = "../../../../Bootloader/src/gpt.rs"]
mod gpt;
#[cfg(test)]
mod image_boot;
#[path = "../../../../Bootloader/src/lp.rs"]
mod lp;
#[path = "../../../../Firmware/src/services/page_store.rs"]
mod page_store;

#[path = "../../../../App/src/lib.rs"]
pub mod apps;
#[path = "../../../../Firmware/src/startup.rs"]
mod startup;
#[path = "../../../../Firmware/src/services/swap_config.rs"]
mod swap_config;
#[path = "../../../../Firmware/src/services/ui_resources.rs"]
pub mod ui_resources;
#[path = "../../../../App/system/app/Weather/main.rs"]
pub mod weather;

// Host rendering substitutes only the hardware allocation policy.
extern crate self as esp_alloc;
/// Структура данных компонента host-стенда реального runtime.
pub enum MemoryCapability {
    Internal,
    External,
}
impl From<MemoryCapability> for u8 {
    /// Преобразует значение в совместимое представление host-стенда.
    fn from(_: MemoryCapability) -> u8 {
        0
    }
}
/// Структура данных компонента host-стенда реального runtime.
pub struct HostHeap;
/// Параметр HEAP компонента host-стенда реального runtime.
pub static HEAP: HostHeap = HostHeap;
impl HostHeap {
    /// Возвращает моделируемый объём занятой памяти.
    pub fn used(&self) -> usize {
        0
    }
    /// Возвращает моделируемый объём свободной памяти.
    pub fn free(&self) -> usize {
        1024 * 1024
    }
    /// Возвращает свободную память для запрошенного класса в host-стенде.
    pub fn free_caps(&self, _: u8) -> usize {
        1024 * 1024
    }
    /// Выделяет host-память вместо аппаратного capability-аллокатора ESP32.
    pub unsafe fn alloc_caps(&self, _: u8, layout: core::alloc::Layout) -> *mut u8 {
        unsafe { std::alloc::alloc(layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime::{BluetoothUiAction, Runtime, RuntimeEvent, RuntimeFrame};
    /// Запускает настоящий runtime на исходниках Framework с host-заменой аппаратных часов.
    fn start() -> Runtime {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../Firmware/system/framework/src");
        let mut paths: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|p| p.unwrap().path())
            .collect();
        paths.sort();
        let mut source = paths
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let app = root.join("../../../../App");
        source.push_str(&std::fs::read_to_string(app.join("src/packages.rhai")).unwrap());
        let mut system_ui: Vec<_> = std::fs::read_dir(app.join("system/priv-app/SystemUI"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rhai"))
            .collect();
        system_ui.sort();
        for path in system_ui {
            source.push_str(&std::fs::read_to_string(path).unwrap());
        }
        source.push_str(&std::fs::read_to_string(root.join("../../../ramdisk/init.rhai")).unwrap());
        source.push_str(
            &std::fs::read_to_string(
                root.join("../../../vendor/etc/aosp_wear/vendor_runtime.rhai"),
            )
            .unwrap(),
        );
        Runtime::start(source).unwrap().0
    }
    /// Получает следующий Material-кадр настоящего runtime для проверки.
    fn frame(os: &mut Runtime) -> material::MaterialFrame {
        let RuntimeFrame::Material(f) = os.render().unwrap() else {
            panic!("not Material");
        };
        *f
    }
    #[test]
    /// Проверяет пропуск лишней работы Rhai при живых таймерах и RTC.
    fn cached_scene_skips_vm_work_but_live_timer_and_rtc_keep_advancing() {
        time::set(100);
        let mut os = start();
        os.replace_state(11).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        os.replace_state(5 | (70 << 9)).unwrap();
        let calls = os.script_stats().calls;
        let before = frame(&mut os);
        for _ in 0..50 {
            os.render().unwrap();
        }
        assert_eq!(os.script_stats().calls, calls);
        assert!(os.script_stats().cache_hits >= 50);
        let remaining = frame(&mut os).timer_secs;
        time::set(102);
        let after = frame(&mut os);
        assert_eq!(after.timer_secs, remaining - 2);
        assert_ne!(after.time_seconds, before.time_seconds);
        assert_eq!(os.script_stats().calls, calls);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(os.script_stats().calls, calls + 1);
        time::set(500);
        assert_eq!(frame(&mut os).clock_alert, 1);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).clock_alert, 0);
    }
    #[test]
    /// Проверяет немедленную обработку колёсика и смены экрана без декоративных задержек.
    fn reduced_motion_keeps_crown_steps_and_page_changes_immediate() {
        time::set(100);
        let mut os = start();
        os.replace_state(5 | (70 << 9)).unwrap();
        time::set_ms(100_100);
        assert_eq!(frame(&mut os).transition_alpha, 255);
        assert_eq!(frame(&mut os).animation_phase, 0);
        for ms in [100_120, 100_160, 100_200, 100_240] {
            time::set_ms(ms);
            let before = os.state();
            let RuntimeFrame::Material(f) = os.dispatch(RuntimeEvent::Rotate(1)).unwrap() else {
                panic!();
            };
            assert_ne!(os.state(), before);
            assert_eq!(
                f.transition_alpha, 255,
                "each crown step must reach the panel immediately"
            );
        }
        os.dispatch(RuntimeEvent::Home).unwrap();
        assert_eq!(frame(&mut os).transition_alpha, 255);
        time::set_ms(100_440);
        assert_eq!(frame(&mut os).transition_alpha, 255);
    }
    #[test]
    /// Проверяет сквозные переходы между приложениями, медиакоманды, сопряжение и часы.
    fn navigation_media_pairing_and_clocks() {
        time::set(0);
        let mut os = start();
        os.replace_state(10).unwrap();
        for usage in [0xB6, 0xCD, 0xB5, 0xEA, 0xE9] {
            os.dispatch(RuntimeEvent::Press).unwrap();
            assert_eq!(
                os.take_bluetooth_action(),
                Some(BluetoothUiAction::Media(usage))
            );
            os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        }
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 2); // sixth media control is Back
        os.replace_state(13 | (2 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 10); // Bluetooth -> Media controls
        os.replace_state(13 | (1 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 30);
        assert_eq!(os.take_bluetooth_action(), Some(BluetoothUiAction::Pair));
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.take_bluetooth_action(), Some(BluetoothUiAction::Pair));
        os.set_bluetooth_status(4, Some(123456));
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(
            os.take_bluetooth_action(),
            Some(BluetoothUiAction::Confirm(true))
        );
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(
            os.take_bluetooth_action(),
            Some(BluetoothUiAction::Confirm(false))
        );
        os.set_bluetooth_status(3, None);
        os.replace_state(4 | (1 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.take_bluetooth_action(), Some(BluetoothUiAction::Pair));
        os.replace_state(11).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap(); // Open preset grid
        os.dispatch(RuntimeEvent::Press).unwrap(); // 5 minute timer
        assert_eq!(frame(&mut os).timer_secs, 300);
        os.replace_state(5).unwrap(); // Expiry also reaches Settings in the foreground.
        time::set(301);
        assert_eq!(frame(&mut os).timer_secs, 0);
        assert_eq!(frame(&mut os).clock_alert, 1);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).clock_alert, 0);
        os.replace_state(11 | (1 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        time::set(311);
        assert_eq!(frame(&mut os).stopwatch_secs, 10);
        os.dispatch(RuntimeEvent::Press).unwrap();
        time::set(330);
        assert_eq!(frame(&mut os).stopwatch_secs, 10);
        os.replace_state(11 | (2 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 31);
        os.dispatch(RuntimeEvent::Press).unwrap(); // alarm enabled
        let saved = os.clock_config();
        os.restore_clock_config(0);
        assert!(!frame(&mut os).alarm_enabled);
        os.restore_clock_config(saved);
        assert!(frame(&mut os).alarm_enabled);
        os.restore_clock_config(-1);
        assert_eq!(os.clock_config(), saved);
        rtc::set_local_minutes(420);
        assert_eq!(frame(&mut os).clock_alert, 2);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).clock_alert, 0); // no immediate re-trigger
        rtc::set_local_minutes(421);
        frame(&mut os);
        rtc::set_local_minutes(420);
        assert_eq!(frame(&mut os).clock_alert, 2);
        os.dispatch(RuntimeEvent::Press).unwrap();
    }
    #[test]
    /// Проверяет восстановление предпочтений и сохранность PIN-блокировки.
    fn preferences_and_pin_gate_survive_restore() {
        time::set(2000);
        let mut os = start();
        os.set_security_storage(true, [9; 16]);
        os.replace_state(18).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.crown_sensitivity(), 2);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        let prefs = os.preference_config();
        let mut other = start();
        other.restore_preferences(&prefs);
        assert!(frame(&mut other).crown_reverse);
        assert_eq!(other.crown_sensitivity(), 2);
        // Positive editor deltas remain monotonic before explicitly reversing direction.
        other.restore_preferences(&[1, 1, 0, 0]);
        other.replace_state(28 | (50 << 9)).unwrap();
        for expected in [55, 60, 65, 70] {
            other.dispatch(RuntimeEvent::Rotate(1)).unwrap();
            assert_eq!(frame(&mut other).brightness, expected);
        }

        /// Вводит последовательность PIN-цифр через обычные события runtime.
        fn enter(os: &mut Runtime, pin: [u8; 4]) {
            if frame(os).display_sleep {
                os.dispatch(RuntimeEvent::Press).unwrap();
            }
            for digit in pin {
                let current = frame(os).pin_digit;
                os.dispatch(RuntimeEvent::Rotate(i32::from(digit) - i32::from(current)))
                    .unwrap();
                os.dispatch(RuntimeEvent::Press).unwrap();
            }
        }
        other.set_security_storage(true, [11; 16]);
        other.replace_state(20).unwrap();
        other.dispatch(RuntimeEvent::Press).unwrap();
        enter(&mut other, [1, 2, 3, 4]);
        enter(&mut other, [1, 2, 3, 4]);
        assert!(frame(&mut other).has_pin);
        assert!(frame(&mut other).security_pending);
        let raw = other.security_config();
        other.security_saved();
        other.replace_state(11).unwrap();
        other.dispatch(RuntimeEvent::Press).unwrap();
        other.dispatch(RuntimeEvent::Rotate(4)).unwrap();
        other.dispatch(RuntimeEvent::Press).unwrap();
        other.dispatch(RuntimeEvent::Rotate(-1000)).unwrap();
        other.dispatch(RuntimeEvent::Press).unwrap();
        time::set(2031);
        assert_eq!(frame(&mut other).screen, 0);
        other.dispatch(RuntimeEvent::Home).unwrap();
        assert_eq!(frame(&mut other).screen, 0);
        other.dispatch(RuntimeEvent::Swipe(2)).unwrap();
        assert_eq!(frame(&mut other).screen, 0);
        other.replace_state(5).unwrap();
        assert_eq!(frame(&mut other).screen, 0);
        time::set(2061);
        assert_eq!(frame(&mut other).clock_alert, 1);
        other.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut other).screen, 0);
        assert_eq!(frame(&mut other).clock_alert, 0);
        enter(&mut other, [1, 2, 3, 4]);
        assert_eq!(frame(&mut other).screen, 1);
        let mut restored = start();
        restored.set_security_storage(true, [12; 16]);
        restored.restore_security(&raw);
        assert_eq!(frame(&mut restored).screen, 0);
        enter(&mut restored, [1, 2, 3, 4]);
        assert_eq!(frame(&mut restored).screen, 1);
        restored.replace_state(20 | (1 << 5)).unwrap();
        restored.dispatch(RuntimeEvent::Press).unwrap();
        restored.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        restored.dispatch(RuntimeEvent::Press).unwrap();
        assert!(frame(&mut restored).security_pending); // Simulate storage not acknowledging the new timeout.
        time::set(2122);
        assert_eq!(frame(&mut restored).screen, 0);
        enter(&mut restored, [1, 2, 3, 4]);
        assert_eq!(frame(&mut restored).screen, 1);
        restored.replace_state(20 | (3 << 5)).unwrap();
        restored.dispatch(RuntimeEvent::Press).unwrap();
        enter(&mut restored, [0, 0, 0, 0]);
        assert!(frame(&mut restored).has_pin);
        enter(&mut restored, [1, 2, 3, 4]);
        assert!(!frame(&mut restored).has_pin);
        let mut damaged = raw;
        damaged[4] = 99;
        restored.restore_security(&damaged);
        assert_eq!(frame(&mut restored).security_view, 7);
        restored.dispatch(RuntimeEvent::Home).unwrap();
        assert_eq!(frame(&mut restored).screen, 0);
    }

    #[test]
    /// Проверяет срок сна и поглощение первого нажатия при пробуждении.
    fn screen_sleep_wakes_without_activation_and_timeout_is_configurable() {
        time::set(3000);
        let mut os = start();
        time::set(3030);
        assert!(frame(&mut os).display_sleep);
        os.set_bluetooth_status(4, Some(123456));
        assert!(!frame(&mut os).display_sleep);
        os.set_bluetooth_status(1, None);
        assert!(frame(&mut os).display_sleep);
        os.dispatch(RuntimeEvent::Sync).unwrap();
        assert!(frame(&mut os).display_sleep);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert!(!frame(&mut os).display_sleep);
        assert_eq!(os.state() & 31, 1);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(os.state() & 31, 4);
        os.replace_state(14 | (2 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).security_view, 5);
        os.dispatch(RuntimeEvent::Rotate(10)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 14);
        assert_eq!(frame(&mut os).lock_timeout_secs, 600);
        time::set(3629);
        assert!(!frame(&mut os).display_sleep);
        time::set(3630);
        assert!(frame(&mut os).display_sleep);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 14);
        assert_eq!(frame(&mut os).security_view, 0);
    }

    #[test]
    /// Проверяет прямые переходы раздела часов и восстановление выбранного циферблата.
    fn clock_hub_routes_directly_and_watch_face_persists() {
        time::set(4000);
        let mut os = start();
        os.replace_state(6).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 31);
        let alarm_config=os.clock_config();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(os.clock_config(),alarm_config ^ 0x800);
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(os.clock_config(),alarm_config);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(os.state() & 31, 6);
        // Selecting a tab updates its page without a confirmation press.
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(frame(&mut os).cursor,1);
        assert_eq!(frame(&mut os).app.id,apps::NONE);
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).app.id,apps::WORLD);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).cursor,1);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(frame(&mut os).cursor,2);
        assert_eq!(frame(&mut os).timer_editor,0);
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).timer_editor,1);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).cursor,2);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(frame(&mut os).cursor,3);
        os.dispatch(RuntimeEvent::Click(2)).unwrap(); // reset before testing start
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert!(frame(&mut os).stopwatch_running);
        assert_eq!(frame(&mut os).screen,6);
        os.dispatch(RuntimeEvent::Click(2)).unwrap();
        assert!(!frame(&mut os).stopwatch_running);
        assert_eq!(frame(&mut os).stopwatch_secs,0);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert_eq!(frame(&mut os).cursor,4);
        assert!(!frame(&mut os).bedtime_active);
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert!(frame(&mut os).bedtime_active);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).screen,1);
        os.replace_state(14 | (3 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).watch_face, 1);
        let prefs = os.preference_config();
        let mut restored = start();
        restored.restore_preferences(&prefs);
        assert_eq!(frame(&mut restored).watch_face, 1);
    }
    #[test]
    /// Проверяет постановку команд питания в очередь без аппаратных действий внутри runtime.
    fn power_menu_queues_actions_without_executing_hardware_in_runtime() {
        time::set(5000);
        let mut os = start();
        os.dispatch(RuntimeEvent::Power).unwrap();
        assert!(frame(&mut os).power_menu);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.take_system_action(), Some(runtime::SystemAction::Reboot));
        assert_eq!(os.take_system_action(), None);
    }
    #[test]
    fn clock_tabs_do_not_change_settings_or_start_tools_on_hover() {
        time::set(7000);
        let mut os=start();
        os.replace_state(6 | (70<<9)).unwrap();
        let before=frame(&mut os);
        let config=os.clock_config();
        for tab in [1,2,3,4,0] {
            os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
            let f=frame(&mut os);
            assert_eq!((f.screen,f.cursor),(6,tab));
            assert_eq!(f.app.id,apps::NONE);
            assert_eq!(f.timer_secs,before.timer_secs);
            assert_eq!(f.stopwatch_running,before.stopwatch_running);
            assert!(!f.bedtime_active);
            assert_eq!(os.clock_config(),config);
        }
    }
    #[test]
    fn bedtime_dims_without_overwriting_preferences_and_alarm_still_wakes() {
        time::set(8000);
        let mut os=start();
        let mut preferences=os.preference_config(); preferences[7]=1;
        os.restore_preferences(&preferences);
        os.replace_state(6 | (4<<5) | (70<<9)).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        let f=frame(&mut os);
        assert!(f.bedtime_active);
        assert_eq!(f.display_brightness,Some(35));
        assert!(f.aod_enabled);
        assert_eq!(os.preference_config(),preferences);
        os.dispatch(RuntimeEvent::Home).unwrap();
        time::set(8030);
        assert!(frame(&mut os).aod);
        assert_eq!(frame(&mut os).display_brightness,Some(35));
        // A real alarm receives the selected brightness, even in bedtime mode.
        rtc::set_local_minutes(7*60);
        os.restore_clock_config((7*60)|0x800);
        assert_eq!(frame(&mut os).clock_alert,2);
        assert_eq!(frame(&mut os).display_brightness,Some(70));
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).clock_alert,0);
        assert!(frame(&mut os).bedtime_active);
        os.replace_state(6 | (4<<5) | (70<<9)).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert!(!frame(&mut os).bedtime_active);
        assert_eq!(frame(&mut os).display_brightness,Some(70));
        assert!(frame(&mut os).aod_enabled);
        assert_eq!(os.preference_config(),preferences);
        os.restore_clock_config(7*60);
    }

    #[test]
    fn hardware_double_click_contexts_exclude_typing_and_alarm_dismissal() {
        time::set(8800); let mut os=start();
        os.replace_state(6|(3<<5)|(70<<9)).unwrap();
        assert!(os.file_gestures_active());
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        assert!(os.file_gestures_active());
        os.dispatch(RuntimeEvent::Click(2)).unwrap();
        assert!(!os.file_gestures_active());
        os.dispatch(RuntimeEvent::Home).unwrap();
        os.replace_state(6|(70<<9)).unwrap();
        assert!(!os.file_gestures_active());
    }
    #[test]
    fn runtime_frame_keeps_large_material_snapshot_off_callers_stack() {
        assert!(core::mem::size_of::<RuntimeFrame>() < 128);
        assert!(core::mem::size_of::<material::MaterialFrame>() > 4096);
    }
    #[test]
    fn bedtime_schedule_roundtrip_midnight_boundaries_and_editor() {
        time::set(9000);
        let mut os=start();
        let mut prefs=os.preference_config();
        prefs[12]=1; prefs[13]=40;
        prefs[14..16].copy_from_slice(&(23u16*60).to_le_bytes());
        prefs[16..18].copy_from_slice(&(7u16*60).to_le_bytes());
        os.restore_preferences(&prefs);
        for (minute, active) in [(1380,true),(1439,true),(0,true),(419,true),(420,false),(1379,false)] {
            rtc::set_local_minutes(minute);
            let f=frame(&mut os);
            assert_eq!(f.bedtime_active,active,"minute {}",minute);
            assert_eq!(f.bedtime_brightness,40);
        }
        assert_eq!(os.preference_config(),prefs);
        rtc::set_local_minutes(23*60);
        assert!(frame(&mut os).bedtime_active);
        os.replace_state(6|(4<<5)|(70<<9)).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert!(!frame(&mut os).bedtime_active);
        rtc::set_local_minutes(23*60+30);
        assert!(!frame(&mut os).bedtime_active);
        rtc::set_local_minutes(7*60);
        assert!(!frame(&mut os).bedtime_active);
        rtc::set_local_minutes(23*60);
        assert!(frame(&mut os).bedtime_active);
        os.replace_state(6|(4<<5)|(70<<9)).unwrap();
        os.dispatch(RuntimeEvent::Click(2)).unwrap();
        assert_eq!(frame(&mut os).bedtime_editor,1);
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).bedtime_editor,2);
        os.dispatch(RuntimeEvent::Rotate(4)).unwrap();
        assert_eq!(frame(&mut os).bedtime_start,0);
        os.dispatch(RuntimeEvent::Back).unwrap();
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).bedtime_editor,0);
        let mut other=start(); other.restore_preferences(&os.preference_config());
        assert_eq!(other.preference_config(),os.preference_config());
        // Corrupt schedule fields are rejected as a unit, ordinary prefs survive.
        let mut bad=prefs; bad[13]=255; other.restore_preferences(&bad);
        assert_eq!(frame(&mut other).bedtime_brightness,40);
    }

    #[test]
    /// Проверяет предустановки, ручную длительность и запуск таймера.
    fn timer_presets_and_custom_duration() {
        time::set(1000);
        let mut os = start();
        os.replace_state(11).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 1);
        for (cursor, duration) in [(0, 300), (1, 600), (2, 900), (3, 1800)] {
            os.replace_state(11 | (cursor << 5)).unwrap();
            os.dispatch(RuntimeEvent::Click(1)).unwrap();
            assert_eq!(frame(&mut os).timer_secs, duration);
        }
        os.replace_state(11 | (4 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 2);
        os.dispatch(RuntimeEvent::Rotate(1000)).unwrap();
        assert_eq!(frame(&mut os).timer_duration_secs, 3600);
        os.dispatch(RuntimeEvent::Rotate(-1000)).unwrap();
        assert_eq!(frame(&mut os).timer_duration_secs, 60);
        os.dispatch(RuntimeEvent::Click(1)).unwrap();
        assert_eq!(frame(&mut os).timer_secs, 60);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 0);
        os.dispatch(RuntimeEvent::Home).unwrap();
        assert_eq!(os.state() & 31, 1);
    }

    #[test]
    /// Проверяет открытие утилит в обоих видах меню и сохранность значений в сессии.
    fn installed_apps_open_from_both_launcher_layouts() {
        let mut os = start();
        for screen in [2, 26] {
            if screen == 26 {
                os.replace_state(2).unwrap();
                os.dispatch(RuntimeEvent::Rotate(i32::from(apps::COUNT)))
                    .unwrap();
                os.dispatch(RuntimeEvent::Press).unwrap();
            }
            for id in [
                apps::CALENDAR,
                apps::GALLERY,
                apps::BT_KEYBOARD,
                apps::GOOGLE,
                apps::GMAIL,
                apps::TRANSLATOR,
            ] {
                let index = apps::LAUNCHER.iter().position(|v| *v == id).unwrap();
                os.replace_state(screen).unwrap();
                os.dispatch(RuntimeEvent::Rotate(index as i32)).unwrap();
                os.dispatch(RuntimeEvent::Press).unwrap();
                assert_eq!(frame(&mut os).app.id, id);
                os.dispatch(RuntimeEvent::Back).unwrap();
                assert_eq!(frame(&mut os).screen, screen as u8);
            }
        }
    }
    #[test]
    fn unified_calendar_switches_month_events_and_back() {
        let mut state = apps::State::default();
        assert!(!apps::LAUNCHER.contains(&apps::GOOGLE_CALENDAR));
        state.open(apps::GOOGLE_CALENDAR); // old package ID remains compatible
        assert_eq!(state.id(), apps::CALENDAR);
        state.event(RuntimeEvent::Rotate(2));
        let month = state.view().tools.month;
        state.event(RuntimeEvent::Click(2));
        assert_eq!(state.view().cursor, 1);
        assert_eq!(state.view().remote.kind, 1);
        assert!(state.cancel_editor());
        assert_eq!(state.view().cursor, 0);
        assert_eq!(state.view().tools.month, month);
        state.event(RuntimeEvent::Click(2));
        state.event(RuntimeEvent::Click(2));
        assert_eq!(state.view().cursor, 0);
    }

    #[test]
    /// Проверяет выбор приложений за пределами четырёх бит курсора без изменения яркости.
    fn launcher_reaches_apps_beyond_the_legacy_four_bit_cursor() {
        let mut os = start();
        os.replace_state(2).unwrap();
        let brightness = os.state() & (127 << 9);
        os.dispatch(RuntimeEvent::Rotate(i32::from(apps::COUNT) - 1))
            .unwrap();
        assert_eq!(frame(&mut os).cursor, apps::COUNT - 1);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).app.id, apps::AIR_KEYBOARD);
        assert_eq!(os.state() & (127 << 9), brightness);
        os.dispatch(RuntimeEvent::Back).unwrap();
        os.dispatch(RuntimeEvent::Rotate(1)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).screen, 26);
        assert_eq!(frame(&mut os).cursor, 0);
    }

    #[test]
    /// Проверяет отрисовку экранов/тем/курсоров и отказ для повреждённого кадра.
    fn render_all_screens_and_reject_malformed_frames() {
        assert!(material::MaterialFrame::parse("M3|1|0").is_none());
        let mut surface = material::Surface::new();
        let mut os = start();
        for theme in 0..4 {
            for screen in 0..32 {
                for cursor in 0..16 {
                    os.replace_state(screen | (cursor << 5) | (theme << 18))
                        .unwrap();
                    let rect = surface.render(frame(&mut os));
                    assert_eq!(rect, material::Rect::FULL);
                    assert_eq!(surface.pixels().len(), 240 * 280);
                }
            }
        }
    }
}

#[path = "../../../../Firmware/src/services/storage_info.rs"]
pub mod storage_info;

#[path = "../../../../Firmware/src/boot_policy.rs"]
mod boot_policy;
#[path = "../../../../Firmware/src/hal/emergency_policy.rs"]
mod emergency_policy;

#[cfg(test)]
mod recovery_transport_tests {
    use super::*;
    use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
    /// Отказ размера носителя моделирует отсутствующий сервер, сохраняя объект транспорта.
    struct FailedTransport(u32);
    impl BlockDevice for FailedTransport {
        type Error = core::fmt::Error;
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Err(core::fmt::Error)
        }
        fn read(&self, _: &mut [Block], _: BlockIdx) -> Result<(), Self::Error> {
            Err(core::fmt::Error)
        }
        fn write(&self, _: &[Block], _: BlockIdx) -> Result<(), Self::Error> {
            Err(core::fmt::Error)
        }
    }
    #[test]
    fn failed_boot_returns_the_original_transport_for_fastboot() {
        let result = android_storage::load_block_device_recoverable(FailedTransport(42));
        let Err((device, error)) = result else {
            panic!("missing disk must fail");
        };
        assert_eq!(device.0, 42);
        assert!(matches!(error, android_storage::AndroidBootError::Card));
    }
}

#[path = "../../../../Firmware/src/services/persistent_log.rs"]
mod persistent_log;

#[path = "../../../../Firmware/src/hal/input/clicks.rs"]
mod clicks;

#[path = "../../../../Firmware/src/services/browser.rs"]
pub mod browser_service;
#[path = "../../../../Firmware/src/services/google_gateway.rs"]
pub mod google_gateway;
#[path = "../../../../Firmware/src/services/google_http.rs"]
mod google_http;
#[path = "../../../../Firmware/src/services/google_tls.rs"]
mod google_tls;
