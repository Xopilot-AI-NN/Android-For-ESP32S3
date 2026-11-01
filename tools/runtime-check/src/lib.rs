//! Compile the real Rhai runtime and software clocks on the host, substituting
//! only the ESP monotonic counter. Run with one test thread (shared clock state).
#![allow(dead_code)]
extern crate alloc;
extern crate self as esp_hal;
extern crate self as esp_println;
pub use std::println;
pub mod time {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NOW: AtomicU64 = AtomicU64::new(0);
    pub struct Instant;
    impl Instant {
        pub fn now() -> Self {
            Self
        }
        pub fn duration_since_epoch(&self) -> std::time::Duration {
            std::time::Duration::from_secs(NOW.load(Ordering::Relaxed))
        }
    }
    pub fn set(seconds: u64) {
        NOW.store(seconds, Ordering::Relaxed);
    }
}
#[path = "../../../src/input/button.rs"]
mod button;
#[path = "../../../src/config.rs"]
mod config;
#[path = "../../../src/display.rs"]
mod display;
#[path = "../../../src/input/encoder.rs"]
mod encoder;
#[path = "../../../src/font.rs"]
mod font;
pub use display::St7789Display;
#[path = "../../../src/material.rs"]
mod material;
#[path = "../../../src/sd_spi.rs"]
mod sd_spi;
#[path = "../../../src/sd_init.rs"]
mod sd_init;
#[path = "../../../src/sd_bitbang.rs"]
mod sd_bitbang;
#[path = "../../../src/disk_probe.rs"]
mod disk_probe;
#[path = "../../../src/rtc.rs"]
mod rtc;
#[path = "../../../src/media.rs"]
mod media;
#[path = "../../../src/runtime.rs"]
mod runtime;

#[cfg(test)]
mod tests {
    use super::*;
    use runtime::{BluetoothUiAction, Runtime, RuntimeEvent, RuntimeFrame};
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
        source.push_str(&std::fs::read_to_string(root.join("../../../ramdisk/init.rhai")).unwrap());
        source.push_str(
            &std::fs::read_to_string(
                root.join("../../../vendor/etc/aosp_wear/vendor_runtime.rhai"),
            )
            .unwrap(),
        );
        Runtime::start(source).unwrap().0
    }
    fn frame(os: &mut Runtime) -> material::MaterialFrame {
        let RuntimeFrame::Material(f) = os.render().unwrap() else {
            panic!("not Material");
        };
        f
    }
    #[test]
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
    fn clock_hub_routes_directly_and_watch_face_persists() {
        time::set(4000);
        let mut os = start();
        os.replace_state(6 | (2 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(os.state() & 31, 31);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(os.state() & 31, 6);
        os.replace_state(6).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 1);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(os.state() & 31, 6);
        os.replace_state(6 | (1 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 3);
        os.replace_state(14 | (3 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).watch_face, 1);
        let prefs = os.preference_config();
        let mut restored = start();
        restored.restore_preferences(&prefs);
        assert_eq!(frame(&mut restored).watch_face, 1);
    }
    #[test]
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
    fn timer_presets_and_custom_duration() {
        time::set(1000);
        let mut os = start();
        os.replace_state(11).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 1);
        for (cursor, duration) in [(0, 300), (1, 600), (2, 900), (3, 1800)] {
            os.replace_state(11 | (cursor << 5)).unwrap();
            os.dispatch(RuntimeEvent::Press).unwrap();
            assert_eq!(frame(&mut os).timer_secs, duration);
        }
        os.replace_state(11 | (4 << 5)).unwrap();
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 2);
        os.dispatch(RuntimeEvent::Rotate(1000)).unwrap();
        assert_eq!(frame(&mut os).timer_duration_secs, 3600);
        os.dispatch(RuntimeEvent::Rotate(-1000)).unwrap();
        assert_eq!(frame(&mut os).timer_duration_secs, 60);
        os.dispatch(RuntimeEvent::Press).unwrap();
        assert_eq!(frame(&mut os).timer_secs, 60);
        os.dispatch(RuntimeEvent::Back).unwrap();
        assert_eq!(frame(&mut os).timer_editor, 0);
        os.dispatch(RuntimeEvent::Home).unwrap();
        assert_eq!(os.state() & 31, 1);
    }

    #[test]
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
