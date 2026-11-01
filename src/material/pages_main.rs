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
        self.status_strip(f, t);
        let accent = rgb565(166, 222, 239);
        let muted = rgb565(44, 69, 80);
        for (x, y, progress) in [
            (56, 65, if f.alarm_enabled { 100 } else { 0 }),
            (
                184,
                65,
                if f.wifi_link_state == 4 {
                    100
                } else if f.wifi {
                    25
                } else {
                    0
                },
            ),
            (
                56,
                217,
                (u32::from(f.timer_secs) * 100 / u32::from(f.timer_duration_secs.max(1))) as u8,
            ),
            (184, 217, ((f.stopwatch_secs % 60) * 100 / 60) as u8),
        ] {
            self.circle(x, y, 30, t.surface);
            self.progress_ring(x, y, 33, progress, accent, muted);
        }
        self.symbol(Symbol::Alarm, 56, 54, accent);
        let alarm = Self::time_text(f.alarm_minutes);
        self.text_center_x(
            56,
            77,
            if f.alarm_enabled {
                core::str::from_utf8(&alarm).unwrap_or("07:00")
            } else {
                "Off"
            },
            1,
            t.on_surface,
        );
        self.symbol(Symbol::Wifi, 184, 54, accent);
        self.text_center_x(
            184,
            77,
            if f.wifi_link_state == 4 {
                "Online"
            } else if f.wifi {
                "Wi-Fi"
            } else {
                "Off"
            },
            1,
            t.on_surface,
        );
        self.big_time(117, f.time_minutes, f.time_valid, accent);
        self.text_center(
            174,
            if f.time_valid { "" } else { "Set time to sync" },
            1,
            t.on_surface_variant,
        );
        self.symbol(Symbol::Timer, 56, 205, accent);
        self.symbol(Symbol::Clock, 184, 205, accent);
        let timer = Self::mmss_text(f.timer_secs);
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        self.text_center_x(
            56,
            229,
            core::str::from_utf8(&timer).unwrap_or("00:00"),
            1,
            t.on_surface,
        );
        self.text_center_x(
            184,
            229,
            core::str::from_utf8(&stopwatch).unwrap_or("00:00"),
            1,
            t.on_surface,
        );
        self.text_center_x(56, 259, "Timer", 1, t.on_surface_variant);
        self.text_center_x(184, 259, "Stopwatch", 1, t.on_surface_variant);
        for i in 0..3 {
            self.circle(
                110 + i * 10,
                269,
                if i == 1 { 3 } else { 2 },
                if i == 1 { accent } else { muted },
            );
        }
        if f.notes > 0 {
            self.circle(120, 199, 3, t.primary);
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
                if selected {
                    t.primary
                } else {
                    Self::app_accent(i as u8)
                },
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
            if index == 6 {
                self.round_rect(
                    55,
                    203,
                    130,
                    72,
                    32,
                    if cur == 6 { t.primary } else { t.surface_high },
                );
                let color = if cur == 6 { t.on_primary } else { t.on_surface };
                self.text_center(215, "Grid view", 2, color);
                self.symbol(Symbol::Grid, 120, 253, color);
                continue;
            }
            self.app_row(
                62 + row as i32 * 66,
                labels[index],
                index == cur,
                index as u8,
                t,
            );
        }
        self.round_rect(235, 76, 3, 114, 1, t.surface_high);
        self.round_rect(235, 76 + cur as i32 * 14, 3, 24, 1, t.primary);
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
            self.symbol(
                [Symbol::Timer, Symbol::Clock, Symbol::Alarm][i],
                cx,
                165,
                if selected { t.on_primary } else { t.on_surface },
            );
            self.text_center_x(cx, 202, labels[i], 1, t.on_surface_variant);
        }
        self.round_rect(
            63,
            237,
            114,
            30,
            15,
            if f.cursor == 3 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.text_center(246, "Back", 1, t.on_surface);
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
            self.round_rect(
                12,
                y,
                216,
                58,
                29,
                if index == cur {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            self.row_icon(labels[index], 38, y + 29, t.primary);
            self.text(62, y + 23, labels[index], 1, t.on_surface);
            if index < 3 {
                self.toggle(174, y + 16, values[index], t);
            }
        }
    }

    pub(super) fn about(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "About", t);
        self.circle(120, 82, 25, t.primary_container);
        self.text_center(69, "A", 3, t.primary);
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
                    "CHOOSE A TIMER"
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
            let timer = Self::mmss_text(if f.timer_secs == 0 {
                f.timer_duration_secs
            } else {
                f.timer_secs
            });
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
        let time = Self::time_text(f.time_minutes);
        self.text_center(
            9,
            core::str::from_utf8(&time).unwrap_or("00:00"),
            1,
            t.on_surface_variant,
        );
        self.text_center(42, "Media controls", 2, t.on_surface);
        self.text_center(
            69,
            if f.bt_status == 3 {
                "Phone connected"
            } else {
                "Pair your phone to play"
            },
            1,
            t.on_surface_variant,
        );
        let cream = rgb565(255, 224, 165);
        let ink = rgb565(57, 42, 18);
        for (i, x) in [(0, 39), (2, 201)] {
            self.circle(x, 151, if f.cursor == i { 29 } else { 26 }, cream);
            self.symbol(
                if i == 0 {
                    Symbol::Previous
                } else {
                    Symbol::Next
                },
                x,
                151,
                ink,
            );
            if f.cursor == i {
                self.ring(x, 151, 32, 2, t.on_surface);
            }
        }
        self.scallop(
            120,
            151,
            cream,
            if f.cursor == 1 {
                t.on_surface
            } else {
                t.surface_high
            },
        );
        self.symbol(Symbol::Play, 120, 151, ink);
        // HID reports do not contain playback state: Play/Pause is one toggle,
        // and the watch must not fabricate a paused/playing status or metadata.
        self.text_center(208, "Play / pause", 1, t.on_surface_variant);
        for (i, x, label) in [(3, 43, "Vol -"), (4, 120, "Vol +"), (5, 197, "Back")] {
            self.round_rect(
                x - 33,
                237,
                66,
                31,
                15,
                if f.cursor == i { cream } else { t.surface_high },
            );
            self.text_center_x(
                x,
                247,
                label,
                1,
                if f.cursor == i { ink } else { t.on_surface },
            );
        }
    }

    pub(super) fn timer_presets(&mut self, f: MaterialFrame, t: Theme) {
        let lavender = rgb565(207, 198, 255);
        let lime = rgb565(222, 244, 148);
        let ink = rgb565(36, 30, 57);
        self.symbol(Symbol::Timer, 120, 23, lavender);
        self.text_center(45, "Timer", 2, t.on_surface);
        if f.timer_editor == 2 {
            let duration = Self::mmss_text(f.timer_duration_secs);
            self.clock_text(104, &duration, lavender);
            self.text_center(188, "Crown: adjust minutes", 1, t.on_surface_variant);
            self.round_rect(55, 229, 130, 38, 19, lime);
            self.text_center(242, "Start timer", 1, ink);
            return;
        }
        if f.timer_secs > 0 {
            let time = Self::mmss_text(f.timer_secs);
            self.text_center(71, core::str::from_utf8(&time).unwrap_or("00:00"), 1, lime);
        }
        for (i, label) in ["5", "10", "15", "30"].iter().enumerate() {
            let x = 18 + (i % 2) as i32 * 105;
            let y = 90 + (i / 2) as i32 * 67;
            self.round_rect(
                x,
                y,
                99,
                61,
                if f.cursor as usize == i { 20 } else { 29 },
                if f.cursor as usize == i {
                    lime
                } else {
                    lavender
                },
            );
            self.text_center_x(x + 49, y + 12, label, 2, ink);
            self.text_center_x(x + 49, y + 39, "Min", 1, ink);
        }
        self.round_rect(
            63,
            235,
            114,
            35,
            17,
            if f.cursor == 4 { lavender } else { lime },
        );
        self.text_center(244, "+", 2, ink);
    }

    pub(super) fn clock_tools(&mut self, f: MaterialFrame, t: Theme) {
        if f.timer_editor != 0 {
            self.timer_presets(f, t);
            return;
        }
        self.title("CLOCK", t);
        let cur = f.cursor.min(4) as usize;
        let timer = Self::mmss_text(if f.timer_secs == 0 {
            f.timer_duration_secs
        } else {
            f.timer_secs
        });
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
        let stopwatch_s = core::str::from_utf8(&stopwatch).unwrap_or("00:00");
        let labels = ["TIMER", "STOPWATCH", "ALARM", "RESET STOPWATCH", "BACK"];
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 57 + row as i32 * 64;
            self.round_rect(
                12,
                y,
                216,
                58,
                29,
                if i == cur {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            self.text(30, y + 20, labels[i], 1, t.on_surface);
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

    pub(super) fn alarm_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Alarm", t);
        let time = Self::time_text(f.alarm_minutes);
        self.text_center(
            68,
            core::str::from_utf8(&time).unwrap_or("07:00"),
            2,
            t.primary,
        );
        let labels = [
            if f.alarm_enabled {
                "Alarm on"
            } else {
                "Alarm off"
            },
            "Hour +",
            "Minute +",
            "Back",
        ];
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                22,
                103 + i as i32 * 39,
                196,
                34,
                17,
                if f.cursor as usize == i {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            self.text_center(114 + i as i32 * 39, label, 1, t.on_surface);
        }
    }
}
