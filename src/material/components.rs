//! components Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn status_strip(&mut self, f: MaterialFrame, t: Theme) {
        // System-facing status only. Development transports (USB/ADB) are not
        // exposed on normal Wear surfaces.
        let mut x = 18;
        if f.airplane {
            self.icon_airplane(x, 15, t.on_surface_variant);
            x += 29;
        }
        if f.wifi {
            self.icon_wifi(x, 15, t.primary);
            x += 31;
        }
        if f.bt {
            self.icon_bt(x, 15, t.primary);
            x += 25;
        }
        if f.dnd {
            self.icon_dnd(x, 15, t.on_surface_variant);
        }
        if f.notes > 0 {
            self.circle(218, 15, 4, t.primary);
        }
    }

    pub(super) fn title(&mut self, title: &str, t: Theme) {
        // No decorative underline: the source watch used a clean black header.
        self.text_center(
            22,
            title,
            if title.len() > 19 { 1 } else { 2 },
            t.on_surface,
        );
    }

    pub(super) fn photo_header(&mut self, frame: MaterialFrame, title: &str, t: Theme) {
        let time = Self::time_text(frame.time_minutes);
        let clock = core::str::from_utf8(&time).unwrap_or("00:00");
        self.text_center(5, clock, 1, t.primary);
        self.text_center(
            29,
            title,
            if title.len() > 19 { 1 } else { 2 },
            t.on_surface,
        );
    }

    pub(super) fn keyboard_key(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        label: &str,
        selected: bool,
        t: Theme,
    ) {
        let bg = if selected { t.primary } else { t.surface_high };
        let fg = if selected { t.on_primary } else { t.on_surface };
        self.round_rect(x, y, w, h, 7, bg);
        self.text_center_x(x + w / 2, y + (h - 7) / 2, label, 1, fg);
    }

    pub(super) fn wear_row(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        // Wide, muted slate pills from the lost firmware photos.
        let (x, w, h) = if selected {
            (10, 220, 59)
        } else {
            (19, 202, 55)
        };
        let bg = if selected {
            t.primary_container
        } else {
            t.surface_high
        };
        self.round_rect(x, y, w, h, 27, bg);
        let cy = y + h / 2;
        self.circle(x + 28, cy, 18, if selected { t.primary } else { t.surface });
        self.row_icon(
            label,
            x + 28,
            cy,
            if selected { t.on_primary } else { t.primary },
        );
        let scale = if label.len() <= 12 && label != "Bluetooth" {
            2
        } else {
            1
        };
        self.text(
            x + 57,
            y + (h - 7 * scale) / 2,
            label,
            scale,
            if selected {
                t.on_primary_container
            } else {
                t.on_surface
            },
        );
        if selected {
            self.round_rect(x + w - 6, cy - 12, 3, 24, 2, t.primary);
        }
    }

    pub(super) fn app_row(&mut self, y: i32, label: &str, selected: bool, icon: u8, t: Theme) {
        let (x, w, h) = if selected {
            (10, 220, 58)
        } else {
            (19, 202, 54)
        };
        self.round_rect(
            x,
            y,
            w,
            h,
            26,
            if selected {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.circle(
            x + 28,
            y + h / 2,
            18,
            if selected { t.primary } else { t.surface },
        );
        self.app_icon(
            icon,
            x + 28,
            y + h / 2,
            if selected { t.on_primary } else { t.primary },
        );
        let scale = if label.len() > 12 { 1 } else { 2 };
        self.text(x + 58, y + (h - 7 * scale) / 2, label, scale, t.on_surface);
    }

    pub(super) fn toggle(&mut self, x: i32, y: i32, on: bool, t: Theme) {
        let bg = if on { t.primary } else { t.surface_high };
        self.round_rect(x, y, 42, 24, 12, bg);
        self.circle(
            if on { x + 30 } else { x + 12 },
            y + 12,
            8,
            if on {
                t.on_primary
            } else {
                t.on_surface_variant
            },
        );
    }

    pub(super) fn slider(&mut self, x: i32, y: i32, w: i32, v: u8, t: Theme) {
        if w < 20 {
            return;
        }
        self.round_rect(x, y, w, 8, 4, t.surface_high);
        let active = ((w - 14) * v.min(100) as i32 / 100).clamp(0, w - 14);
        self.round_rect(x, y, active + 7, 8, 4, t.primary);
        self.circle(x + 7 + active, y + 4, 11, t.primary);
    }

    pub(super) fn quick_button(
        &mut self,
        cx: i32,
        cy: i32,
        label: &str,
        on: bool,
        selected: bool,
        t: Theme,
    ) {
        // Photo layout: two columns of three horizontal pills; icon left, label right.
        let (w, h) = if selected { (110, 58) } else { (106, 54) };
        if selected {
            self.round_rect(
                cx - w / 2 - 2,
                cy - h / 2 - 2,
                w + 4,
                h + 4,
                h / 2 + 2,
                t.primary_container,
            );
        }
        let bg = if on {
            t.primary_container
        } else {
            t.surface_high
        };
        let fg = if on {
            t.on_primary_container
        } else {
            t.on_surface
        };
        self.round_rect(cx - w / 2, cy - h / 2, w, h, h / 2, bg);
        let ix = cx - 30;
        match label {
            "Wi-Fi" => self.icon_wifi(ix, cy, if on { t.primary } else { fg }),
            "Bluetooth" => self.icon_bt(ix, cy, if on { t.primary } else { fg }),
            "DND" => self.icon_dnd(ix, cy, fg),
            "Airplane" => self.icon_airplane(ix, cy, fg),
            "Flashlight" => self.icon_sun(ix, cy, fg),
            "Settings" => self.icon_settings(ix, cy, fg),
            _ => self.circle(ix, cy, 5, fg),
        }
        let tx = cx - 10;
        if label == "DND" {
            self.text(tx, cy - 9, "Do not", 1, fg);
            self.text(tx, cy + 2, "disturb", 1, fg);
        } else if label == "Bluetooth" {
            self.text(tx, cy - 3, "BT", 1, fg);
        } else if label == "Flashlight" {
            self.text(tx, cy - 3, "Light", 1, fg);
        } else {
            self.text(tx, cy - 3, label, 1, fg);
        }
        if selected {
            self.round_rect(cx - 10, cy + 16, 22, 2, 1, t.primary);
        }
    }

    pub(super) fn info_card(&mut self, y: i32, title: &str, value: &str, active: bool, t: Theme) {
        let x = if active { 10 } else { 18 };
        let w = if active { 220 } else { 204 };
        let h = 57;
        self.round_rect(
            x,
            y,
            w,
            h,
            h / 2,
            if active {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.circle(
            x + 27,
            y + h / 2,
            17,
            if active { t.primary } else { t.surface },
        );
        self.circle(
            x + 27,
            y + h / 2,
            5,
            if active { t.on_primary } else { t.primary },
        );
        self.text(x + 53, y + 10, title, 1, t.on_surface);
        self.text(
            x + 53,
            y + 31,
            value,
            1,
            if active {
                t.on_primary_container
            } else {
                t.on_surface_variant
            },
        );
    }
}
