//! pages main Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.status_strip(f, t);
        self.big_time(64, f.time_minutes, f.time_valid, t.on_surface);
        self.icon_lock(120, 195, t.primary);
        self.text_center(238, "PRESS CROWN TO UNLOCK", 1, t.on_surface_variant);
    }

    pub(super) fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        // Faithful high-contrast dark dial from the user's lost firmware photos.
        self.status_strip(f, t);
        self.big_time(58, f.time_minutes, f.time_valid, t.on_surface);
        if !f.time_valid {
            self.text_center(
                150,
                if f.auto_time {
                    "SYNCING TIME"
                } else {
                    "SET TIME"
                },
                1,
                t.on_surface_variant,
            );
        }
        let cy = 206;
        for (cx, active) in [(49, f.wifi), (120, f.alarm_enabled), (191, f.dnd)] {
            self.circle(
                cx,
                cy,
                27,
                if active {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
        }
        self.icon_wifi(
            49,
            cy,
            if f.wifi {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        self.icon_clock(
            120,
            cy,
            if f.alarm_enabled {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        self.icon_dnd(
            191,
            cy,
            if f.dnd {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        if f.notes > 0 {
            self.circle(120, 259, 4, t.primary);
        }
    }

    pub(super) fn launcher(&mut self, f: MaterialFrame, t: Theme) {
        let labels = [
            "Clock",
            "Media",
            "Settings",
            "Connectivity",
            "System info",
            "Assistant",
            "List view",
        ];
        let positions = [
            (52, 67),
            (120, 59),
            (188, 67),
            (52, 140),
            (120, 133),
            (188, 140),
            (120, 207),
        ];
        let cur = f.cursor.min(6) as usize;
        for i in 0..7 {
            let (x, y) = positions[i];
            let selected = i == cur;
            let radius = if selected { 32 } else { 26 };
            self.circle(
                x,
                y,
                radius,
                if selected {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            if selected {
                self.ring(x, y, radius + 3, 2, t.primary);
            }
            self.app_icon(
                i as u8,
                x,
                y,
                if selected { t.primary } else { t.on_surface },
            );
        }
        self.round_rect(33, 247, 174, 28, 14, t.primary_container);
        self.text_center(257, labels[cur], 1, t.on_primary_container);
    }

    pub(super) fn list_launcher(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Apps", t);
        let labels = [
            "Clock",
            "Media",
            "Settings",
            "Connectivity",
            "System info",
            "Assistant",
            "Grid view",
        ];
        let cur = f.cursor.min(6) as usize;
        let start = cur.saturating_sub(1).min(4);
        for row in 0..3 {
            let index = (start + row).min(6);
            self.app_row(
                62 + row as i32 * 66,
                labels[index],
                index == cur,
                index as u8,
                t,
            );
        }
    }

    pub(super) fn assistant_page(&mut self, _f: MaterialFrame, t: Theme) {
        // Assistant shell is ready for the future phone companion.  The UI is
        // intentionally service-neutral: it mirrors the Wear assistant surface
        // without claiming a Google/Gemini backend that does not exist here.
        self.ring(120, 116, 70, 2, t.surface_high);
        self.icon_assistant(120, 91, t.primary);
        self.text_center(126, "ASK ASSISTANT", 2, t.on_surface);
        self.text_center(158, "COMPANION APP REQUIRED", 1, t.on_surface_variant);
        self.line(79, 183, 101, 183, 3, rgb565(80, 190, 255));
        self.line(101, 183, 123, 183, 3, rgb565(126, 225, 165));
        self.line(123, 183, 145, 183, 3, rgb565(224, 187, 255));
        self.line(145, 183, 167, 183, 3, rgb565(255, 184, 161));
        self.round_rect(51, 224, 138, 38, 19, t.surface_high);
        self.text_center(236, "PRESS TO RETURN", 1, t.on_surface_variant);
    }

    pub(super) fn notifications(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(10, ts, 1, t.on_surface_variant);
        if f.notes == 0 {
            self.circle(120, 113, 32, t.surface);
            self.icon_bell(120, 113, t.on_surface_variant);
            self.text_center(161, "NO NOTIFICATIONS", 1, t.on_surface_variant);
        } else {
            let selected = f.cursor == 0;
            self.round_rect(
                12,
                50,
                216,
                112,
                35,
                if selected {
                    t.primary_container
                } else {
                    t.surface
                },
            );
            self.circle(44, 82, 14, t.primary);
            self.icon_bell(44, 82, t.on_primary);
            self.text(67, 67, "SYSTEM", 1, t.on_surface_variant);
            self.text(
                67,
                88,
                "NOTIFICATION",
                2,
                if selected {
                    t.on_primary_container
                } else {
                    t.on_surface
                },
            );
            self.text(67, 119, "PRESS TO DISMISS", 1, t.on_surface_variant);
            if f.notes > 1 {
                self.round_rect(27, 175, 186, 48, 24, t.surface);
                self.text_center(191, "MORE NOTIFICATIONS", 1, t.on_surface_variant);
            }
        }
    }

    pub(super) fn quick_settings(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        self.text_center(
            8,
            core::str::from_utf8(&tm).unwrap_or("00:00"),
            2,
            t.on_surface,
        );
        self.quick_button(63, 76, "Wi-Fi", f.wifi, f.cursor == 0, t);
        self.quick_button(177, 76, "Bluetooth", f.bt, f.cursor == 1, t);
        self.quick_button(63, 140, "DND", f.dnd, f.cursor == 2, t);
        self.quick_button(177, 140, "Airplane", f.airplane, f.cursor == 3, t);
        self.quick_button(63, 204, "Flashlight", false, f.cursor == 4, t);
        self.quick_button(177, 204, "Settings", false, f.cursor == 5, t);
        self.icon_sun(27, 256, t.primary);
        self.slider(48, 251, 173, f.brightness, t);
        if f.cursor == 6 {
            self.ring(
                48 + 7 + (159 * f.brightness as i32 / 100),
                255,
                14,
                2,
                t.on_surface,
            );
        }
    }

    pub(super) fn settings(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Settings", t);
        let labels = [
            "Connectivity",
            "Apps & notifications",
            "Display",
            "Sound & vibration",
            "Gestures",
            "Accessibility",
            "Security",
            "System",
            "Back",
        ];
        let cur = f.cursor.min(8) as usize;
        let start = cur.saturating_sub(1).min(6);
        for row in 0..3 {
            let index = (start + row).min(8);
            self.wear_row(63 + row as i32 * 67, labels[index], index == cur, t);
        }
    }

    pub(super) fn clock(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(34, f.time_minutes, f.time_valid, t.on_surface);
        self.text_center(98, "CLOCK", 1, t.on_surface_variant);
        let labels = ["TIMER", "STOPWATCH", "ALARM"];
        for i in 0..3 {
            let selected = f.cursor as usize == i;
            let cx = 48 + i as i32 * 72;
            self.circle(
                cx,
                165,
                if selected { 28 } else { 23 },
                if selected { t.primary } else { t.surface_high },
            );
            self.icon_clock(cx, 165, if selected { t.on_primary } else { t.on_surface });
            self.text_center_x(cx, 202, labels[i], 1, t.on_surface_variant);
        }
    }

    pub(super) fn connectivity(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Connectivity", t);
        let labels = ["Wi-Fi", "Bluetooth", "Airplane mode", "Back"];
        let values = [f.wifi, f.bt, f.airplane, false];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let index = (start + row).min(3);
            let y = 65 + row as i32 * 66;
            self.wear_row(y, labels[index], index == cur, t);
            if index < 3 {
                self.toggle(174, y + 16, values[index], t);
            }
        }
    }

    pub(super) fn about(&mut self, f: MaterialFrame, t: Theme) {
        self.title("ABOUT", t);
        self.circle(120, 72, 30, t.primary_container);
        self.text_center(57, "A", 4, t.primary);
        self.text_center(116, "AOSP WEAR", 2, t.on_surface);
        self.text_center(144, "ANDROID 17", 1, t.primary);
        self.round_rect(22, 168, 196, 36, 18, t.surface);
        self.text_center(180, "BUILD 17.2.1", 1, t.on_surface);
        self.text_center(216, "ESP32-S3-ZERO-N4R2", 1, t.on_surface_variant);
        self.text_center(
            238,
            if f.swap_pages > 0 {
                "SYSTEM STORAGE ACTIVE"
            } else {
                "SYSTEM STORAGE READY"
            },
            1,
            t.outline,
        );
    }

    pub(super) fn widgets(&mut self, f: MaterialFrame, t: Theme) {
        let cur = f.cursor.min(2);
        // Edge-to-edge Wear tile card.  Each tile gets its own accent family
        // instead of looking like another Settings page.
        let (bg, fg, sub) = if cur == 0 {
            (
                rgb565(58, 113, 94),
                rgb565(228, 255, 241),
                if f.wifi {
                    "WI-FI CONNECTED"
                } else {
                    "WI-FI OFF"
                },
            )
        } else if cur == 1 {
            (
                rgb565(47, 78, 126),
                rgb565(226, 236, 255),
                if f.timer_secs > 0 {
                    "TIMER RUNNING"
                } else {
                    "5 MINUTE TIMER"
                },
            )
        } else {
            (
                rgb565(92, 67, 119),
                rgb565(247, 231, 255),
                if f.alarm_enabled {
                    "ALARM ON"
                } else {
                    "ALARM OFF"
                },
            )
        };
        self.round_rect(10, 24, 220, 208, 42, bg);
        if cur == 0 {
            self.icon_wifi(52, 66, fg);
            self.text(82, 55, "CONNECTIVITY", 1, fg);
            self.text(27, 112, sub, 2, fg);
            self.text(27, 153, "PRESS TO OPEN", 1, fg);
        } else if cur == 1 {
            self.icon_clock(52, 66, fg);
            self.text(82, 55, "CLOCK", 1, fg);
            self.text(27, 112, sub, 2, fg);
            let timer = Self::mmss_text(if f.timer_secs == 0 { 300 } else { f.timer_secs });
            let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
            self.text(27, 153, timer_s, 2, fg);
        } else {
            self.icon_clock(52, 66, fg);
            self.text(82, 55, "ALARM", 1, fg);
            self.text(27, 112, sub, 2, fg);
            self.text(27, 153, "PRESS TO OPEN", 1, fg);
        }
        // Carousel dots deliberately sit outside the card like Wear tiles.
        for i in 0..3 {
            self.circle(
                108 + i * 12,
                254,
                if i as u8 == cur { 4 } else { 2 },
                if i as u8 == cur { t.primary } else { t.outline },
            );
        }
    }

    pub(super) fn media(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        self.text_center(
            9,
            core::str::from_utf8(&tm).unwrap_or("00:00"),
            1,
            t.on_surface_variant,
        );
        self.circle(120, 73, 35, t.primary_container);
        self.icon_media(120, 73, t.on_primary_container);
        self.text_center(123, "Connect your phone", 2, t.on_surface);
        self.text_center(153, "Pair in Bluetooth settings", 1, t.on_surface_variant);
        let labels = ["<", ">", ">|"];
        for i in 0..3 {
            let cx = 56 + i as i32 * 64;
            let selected = f.cursor as usize == i;
            self.circle(
                cx,
                206,
                if selected { 26 } else { 23 },
                if selected { t.primary } else { t.surface_high },
            );
            self.text_center_x(
                cx,
                197,
                labels[i],
                2,
                if selected { t.on_primary } else { t.on_surface },
            );
        }
        // Volumes are explicitly muted until a real BLE HID report channel exists.
        self.round_rect(15, 249, 64, 25, 12, t.surface_high);
        self.round_rect(88, 249, 64, 25, 12, t.surface_high);
        self.round_rect(161, 249, 64, 25, 12, t.surface_high);
        self.text_center_x(47, 258, "Vol -", 1, t.outline);
        self.text_center_x(120, 258, "Vol +", 1, t.outline);
        self.text_center_x(193, 258, "Back", 1, t.outline);
    }

    pub(super) fn clock_tools(&mut self, f: MaterialFrame, t: Theme) {
        self.title("CLOCK", t);
        let cur = f.cursor.min(3) as usize;
        let timer = Self::mmss_text(if f.timer_secs == 0 { 300 } else { f.timer_secs });
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
        let stopwatch_s = core::str::from_utf8(&stopwatch).unwrap_or("00:00");
        let labels = ["TIMER", "STOPWATCH", "ALARM", "BACK"];
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 {
                self.text(
                    154,
                    y + 19,
                    timer_s,
                    1,
                    if f.timer_secs > 0 {
                        t.primary
                    } else {
                        t.on_surface_variant
                    },
                );
            }
            if i == 1 {
                self.text(154, y + 19, stopwatch_s, 1, t.on_surface_variant);
            }
            if i == 2 {
                self.text(
                    174,
                    y + 19,
                    if f.alarm_enabled { "ON" } else { "OFF" },
                    1,
                    if f.alarm_enabled {
                        t.primary
                    } else {
                        t.on_surface_variant
                    },
                );
            }
        }
    }
}
