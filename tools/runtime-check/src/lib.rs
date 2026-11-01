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
#[path = "../../../src/font.rs"]
mod font;
pub use display::St7789Display;
#[path = "../../../src/material.rs"]
mod material;
#[path = "../../../src/rtc.rs"]
mod rtc;
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
