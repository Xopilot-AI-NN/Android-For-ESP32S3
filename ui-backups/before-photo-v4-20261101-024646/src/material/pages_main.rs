//! pages main Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(55, f.time_minutes, f.time_valid, t.on_surface);
        self.ring(120, 145, 17, 3, t.surface_high);
        self.round_rect(114, 142, 12, 12, 3, t.on_surface_variant);
        if f.notes > 0 {
            self.circle(120, 196, 5, t.primary);
        }
        self.text_center(226, "UNLOCK", 1, t.on_surface_variant);
    }

    pub(super) fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        // Watch face intentionally contains no Android/debug branding. Wear
        // surfaces prioritize glanceable time, status, and complications.
        self.status_strip(f, t);
        self.big_time(43, f.time_minutes, f.time_valid, t.on_surface);
        if !f.time_valid {
            self.text_center(
                112,
                if f.auto_time {
                    "SYNCING TIME"
                } else {
                    "SET TIME IN SETTINGS"
                },
                1,
                t.on_surface_variant,
            );
        }

        // Three small complication slots. They expose only real state that the
        // board actually knows, not development transport state.
        let y = 177;
        for cx in [54, 120, 186] {
            self.circle(cx, y, 24, t.surface);
        }
        if f.wifi {
            self.icon_wifi(54, y, t.primary);
        } else {
            self.icon_wifi(54, y, t.outline);
        }
        if f.alarm_enabled {
            self.icon_clock(120, y, t.primary);
        } else {
            self.icon_clock(120, y, t.outline);
        }
        if f.dnd {
            self.icon_dnd(186, y, t.primary);
        } else {
            self.icon_dnd(186, y, t.outline);
        }

        if f.notes > 0 {
            self.circle(120, 231, 6, t.primary);
            self.circle(120, 231, 2, t.on_primary);
        }
    }

    pub(super) fn launcher(&mut self, f: MaterialFrame, t: Theme) {
        // Honeycomb/grid launcher adapted from modern Wear OS.  The selected
        // bubble grows slightly so the encoder always has an obvious focus.
        let labels = [
            "CLOCK",
            "MEDIA",
            "SETTINGS",
            "CONNECTIVITY",
            "SYSTEM INFO",
            "ASSISTANT",
            "LIST VIEW",
        ];
        let cur = f.cursor.min(6) as usize;
        let pos = [
            (52, 70),
            (120, 58),
            (188, 70),
            (52, 140),
            (120, 128),
            (188, 140),
            (120, 208),
        ];
        for i in 0..7 {
            let (cx, cy) = pos[i];
            let selected = i == cur;
            let r = if selected {
                33
            } else {
                if i == 6 { 26 } else { 28 }
            };
            let bg = if i == 6 {
                t.surface_high
            } else {
                Self::app_accent(i as u8)
            };
            let fg = if i == 6 {
                t.on_surface_variant
            } else {
                rgb565(10, 34, 24)
            };
            if selected {
                self.ring(cx, cy, r + 3, 2, t.on_surface);
            }
            self.circle(cx, cy, r, bg);
            self.app_icon(i as u8, cx, cy, fg);
        }
        // Only the focused app gets a label, keeping the grid glanceable like
        // the round-screen launcher while still being usable with one encoder.
        self.round_rect(31, 244, 178, 27, 13, t.surface);
        self.text_center(254, labels[cur], 1, t.on_surface);
    }

    pub(super) fn list_launcher(&mut self, f: MaterialFrame, t: Theme) {
        let labels = [
            "CLOCK",
            "MEDIA",
            "SETTINGS",
            "CONNECTIVITY",
            "SYSTEM INFO",
            "ASSISTANT",
            "GRID VIEW",
        ];
        let cur = f.cursor.min(6) as usize;
        let start = cur.saturating_sub(1).min(4);
        for row in 0..3 {
            let i = (start + row).min(6);
            let y = 44 + row as i32 * 66;
            let selected = i == cur;
            if i == 6 {
                let (x, w, h) = if selected {
                    (22, 196, 54)
                } else {
                    (34, 172, 46)
                };
                self.round_rect(
                    x,
                    y,
                    w,
                    h,
                    h / 2,
                    if selected { t.primary } else { t.surface_high },
                );
                self.icon_grid(
                    x + 31,
                    y + h / 2,
                    if selected {
                        t.on_primary
                    } else {
                        t.on_surface_variant
                    },
                );
                self.text(
                    x + 58,
                    y + (h - 14) / 2,
                    "GRID VIEW",
                    2,
                    if selected { t.on_primary } else { t.on_surface },
                );
            } else {
                self.app_row(y, labels[i], selected, i as u8, t);
            }
        }
        self.circle(
            228,
            72 + (cur.saturating_sub(start) as i32 * 66),
            3,
            t.primary,
        );
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
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(9, ts, 2, t.on_surface);
        self.quick_button(66, 68, "WIFI", f.wifi, f.cursor == 0, t);
        self.quick_button(174, 68, "BT", f.bt, f.cursor == 1, t);
        self.quick_button(66, 132, "DND", f.dnd, f.cursor == 2, t);
        self.quick_button(174, 132, "AIR", f.airplane, f.cursor == 3, t);
        self.quick_button(66, 196, "LIGHT", true, f.cursor == 4, t);
        self.quick_button(174, 196, "SET", false, f.cursor == 5, t);
        self.icon_sun(25, 250, t.on_surface_variant);
        self.slider(43, 246, 165, f.brightness, t);
    }

    pub(super) fn settings(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SETTINGS", t);
        let labels = [
            "CONNECTIVITY",
            "APPS & NOTIFS",
            "DISPLAY",
            "SOUND & VIBRATION",
            "GESTURES",
            "ACCESSIBILITY",
            "SECURITY",
            "SYSTEM",
            "BACK",
        ];
        let cur = f.cursor.min(8) as usize;
        let start = cur.saturating_sub(1).min(6);
        for row in 0..3 {
            let i = (start + row).min(8);
            self.wear_row(55 + row as i32 * 64, labels[i], i == cur, t);
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
        self.title("CONNECTIVITY", t);
        let labels = ["WI-FI", "BLUETOOTH", "AIRPLANE MODE", "BACK"];
        let vals = [f.wifi, f.bt, f.airplane, false];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i < 3 {
                self.toggle(174, y + 13, vals[i], t);
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
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(9, ts, 1, t.on_surface_variant);
        self.circle(120, 78, 38, t.surface_high);
        self.text_center(61, "M", 4, t.primary);
        self.text_center(128, "NO MEDIA PLAYING", 1, t.on_surface);
        self.text_center(149, "START MEDIA ON PHONE", 1, t.on_surface_variant);
        let labels = ["<", "||", ">"];
        for i in 0..3 {
            let cx = 55 + i as i32 * 65;
            let selected = f.cursor as usize == i;
            self.circle(
                cx,
                204,
                if selected { 26 } else { 21 },
                if selected { t.primary } else { t.surface_high },
            );
            self.text_center_x(
                cx,
                198,
                labels[i],
                2,
                if selected { t.on_primary } else { t.on_surface },
            );
        }
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
