//! pages main Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.status_strip(f, t);
        self.big_time(64, f.time_minutes, f.time_valid, t.on_surface);
        self.icon_lock(120, 195, t.primary);
        self.text_center(238, "PRESS CROWN TO UNLOCK", 1, t.on_surface_variant);
    }

    pub(super) fn complication_face(&mut self, f: MaterialFrame, t: Theme) {
        self.status_strip(f, t);
        let accent = t.primary;
        let muted = t.surface_high;
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
                ""
            },
            1,
            t.on_surface,
        );
        self.symbol(Symbol::Wifi, 184, 54, accent);
        self.text_center_x(
            184,
            77,
            if f.wifi_link_state == 4 {
                ""
            } else if f.wifi {
                ""
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
        let count = crate::apps::COUNT as usize;
        let cur = usize::from(f.cursor).min(count);
        let page = cur.min(count - 1) / 6;
        let positions = [
            (50, 75),
            (120, 75),
            (190, 75),
            (50, 151),
            (120, 151),
            (190, 151),
        ];
        for (cell, (x, y)) in positions.into_iter().enumerate() {
            let i = page * 6 + cell;
            if i >= count {
                break;
            }
            let selected = i == cur;
            self.circle(x, y, 29, Self::focus_fill(t.surface_high, selected));
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
        self.text_center(
            199,
            if cur < count {
                crate::apps::LABELS[cur]
            } else {
                ""
            },
            1,
            t.on_surface,
        );
        for p in 0..count.div_ceil(6) {
            self.circle(
                115 + p as i32 * 10,
                222,
                2,
                if p == page { t.primary } else { t.outline },
            );
        }
        self.round_rect(
            55,
            237,
            130,
            35,
            17,
            Self::focus_fill(t.surface_high, cur == count),
        );
        self.text_center(248, "List view", 1, t.on_surface);
    }

    pub(super) fn list_launcher(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Apps", t);
        let count = crate::apps::COUNT as usize;
        let cur = usize::from(f.cursor).min(count);
        let start = cur.saturating_sub(1).min(count - 2);
        for row in 0..3 {
            let index = start + row;
            let y = 62 + row as i32 * 66;
            if index == count {
                self.round_rect(
                    55,
                    y,
                    130,
                    60,
                    30,
                    Self::focus_fill(t.surface_high, cur == count),
                );
                self.text_center(y + 12, "Grid view", 1, t.on_surface);
                self.symbol(Symbol::Grid, 120, y + 43, t.on_surface);
            } else {
                self.app_row(y, crate::apps::LABELS[index], index == cur, index as u8, t);
            }
        }
        self.scroll_indicator(cur as u8, crate::apps::COUNT + 1, t);
    }

    pub(super) fn assistant_page(&mut self, f: MaterialFrame, t: Theme) {
        // Assistant shell is ready for the future phone companion.  The UI is
        // intentionally service-neutral: it mirrors the Wear assistant surface
        // without claiming a Google/Gemini backend that does not exist here.
        self.ring(120, 116, 70, 2, t.surface_high);
        self.icon_assistant(120, 91, t.primary);
        self.text_center(126, "ASK ASSISTANT", 2, t.on_surface);
        self.text_center(
            158,
            if f.bt_status == 3 {
                "Ассистент устройства"
            } else {
                "Подключите устройство"
            },
            1,
            t.on_surface_variant,
        );
        self.line(79, 183, 101, 183, 3, rgb565(80, 190, 255));
        self.line(101, 183, 123, 183, 3, rgb565(126, 225, 165));
        self.line(123, 183, 145, 183, 3, rgb565(224, 187, 255));
        self.line(145, 183, 167, 183, 3, rgb565(255, 184, 161));
        self.round_rect(51, 224, 138, 38, 19, t.surface_high);
        self.text_center(236, "Нажмите для запуска", 1, t.on_surface_variant);
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
        self.scroll_indicator(cur as u8, 9, t);
    }

    pub(super) fn clock(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(34, f.time_minutes, f.time_valid, t.on_surface);
        self.text_center(98, "CLOCK", 1, t.on_surface_variant);
        let labels = ["TIMER", "STOPWATCH", "ALARM"];
        for i in 0..3 {
            let selected = f.cursor as usize == i;
            let cx = 48 + i as i32 * 72;
            self.circle(cx, 165, 23, Self::focus_fill(t.surface_high, selected));
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
        let name = core::str::from_utf8(&f.device_name[..usize::from(f.device_name_len).min(32)])
            .unwrap_or(crate::device_info::DEFAULT_DEVICE_NAME);
        self.text_center_raw(116, name, 2, t.on_surface);
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
                t.primary_container,
                t.on_primary_container,
                if f.wifi {
                    "WI-FI CONNECTED"
                } else {
                    "WI-FI OFF"
                },
            )
        } else if cur == 1 {
            (
                t.primary_container,
                t.on_primary_container,
                if f.timer_secs > 0 {
                    "TIMER RUNNING"
                } else {
                    "CHOOSE A TIMER"
                },
            )
        } else {
            (
                t.primary_container,
                t.on_primary_container,
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
        let cream = t.primary;
        let ink = t.on_primary;
        for (i, x) in [(0, 39), (2, 201)] {
            self.circle(x, 151, 26, Self::focus_fill(cream, f.cursor == i));
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
        }
        self.scallop(
            120,
            151,
            Self::focus_fill(cream, f.cursor == 1),
            t.surface_high,
        );
        self.symbol(
            if f.media_playing {
                Symbol::Pause
            } else {
                Symbol::Play
            },
            120,
            151,
            ink,
        );
        if f.media_known {
            self.round_rect(36, 198, 168, 4, 2, t.surface_high);
            self.round_rect(
                36,
                198,
                (168 * i32::from(f.media_progress) / 1000).max(2),
                4,
                2,
                t.primary,
            );
            if f.media_playing {
                for i in 0..3 {
                    let h = 3 + ((u16::from(f.animation_phase) / 20 + i * 3) % 7) as i32;
                    self.round_rect(111 + i as i32 * 7, 230 - h, 3, h, 1, t.primary);
                }
            }
        }
        // HID reports do not contain playback state: Play/Pause is one toggle,
        // and the watch must not fabricate a paused/playing status or metadata.
        self.text_center(
            208,
            if f.media_known {
                if f.media_playing {
                    "Воспроизведение"
                } else {
                    "Пауза"
                }
            } else {
                "Play / pause"
            },
            1,
            t.on_surface_variant,
        );
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

    pub(super) fn stopwatch_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Stopwatch", t);
        let time = Self::mmss_text(f.stopwatch_secs);
        self.clock_text(99, &time, t.primary);
        let labels = [
            if f.stopwatch_running {
                "Пауза"
            } else {
                "Пуск"
            },
            "Сброс",
            "BACK",
        ];
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                32,
                178 + i as i32 * 31,
                176,
                28,
                14,
                Self::focus_fill(t.surface_high, f.cursor as usize == i),
            );
            self.text_center(187 + i as i32 * 31, label, 1, t.on_surface);
        }
    }

    pub(super) fn timer_presets(&mut self, f: MaterialFrame, t: Theme) {
        let lavender = t.primary_container;
        let lime = t.primary;
        let ink = t.on_primary_container;
        self.symbol(Symbol::Timer, 120, 23, t.primary);
        self.text_center(45, "Timer", 2, t.on_surface);
        if f.timer_editor == 2 {
            let duration = Self::mmss_text(f.timer_duration_secs);
            self.clock_text(104, &duration, t.primary);
            self.text_center(188, "Crown: adjust minutes", 1, t.on_surface_variant);
            self.round_rect(55, 229, 130, 38, 19, lime);
            self.text_center(242, "Start timer", 1, t.on_primary);
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
                29,
                Self::focus_fill(lavender, f.cursor as usize == i),
            );
            self.text_center_x(
                x + 49,
                y + 12,
                label,
                2,
                if f.cursor as usize == i {
                    t.on_primary
                } else {
                    ink
                },
            );
            self.text_center_x(
                x + 49,
                y + 39,
                "Min",
                1,
                if f.cursor as usize == i {
                    t.on_primary
                } else {
                    ink
                },
            );
        }
        self.round_rect(
            63,
            235,
            114,
            35,
            17,
            if f.cursor == 4 { lavender } else { lime },
        );
        self.text_center(
            244,
            "+",
            2,
            if f.cursor == 4 {
                t.on_primary_container
            } else {
                t.on_primary
            },
        );
    }

    pub(super) fn clock_tools(&mut self, f: MaterialFrame, t: Theme) {
        if f.timer_editor == 3 {
            self.stopwatch_page(f, t);
            return;
        }
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
                Self::focus_fill(t.surface_high, f.cursor as usize == i),
            );
            self.text_center(114 + i as i32 * 39, label, 1, t.on_surface);
        }
    }
}
