//! components Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn status_strip(&mut self, f: MaterialFrame, t: Theme) {
        // System-facing status only. Development transports (USB/ADB) are not
        // exposed on normal Wear surfaces.
        let mut x = 18;
        if f.airplane { self.icon_airplane(x, 15, t.on_surface_variant); x += 29; }
        if f.wifi { self.icon_wifi(x, 15, t.primary); x += 31; }
        if f.bt { self.icon_bt(x, 15, t.primary); x += 25; }
        if f.dnd { self.icon_dnd(x, 15, t.on_surface_variant); }
        if f.notes > 0 { self.circle(218, 15, 4, t.primary); }
    }

    pub(super) fn title(&mut self, title: &str, t: Theme) { self.text_center(15, title, 2, t.on_surface); }

    pub(super) fn keyboard_key(&mut self, x: i32, y: i32, w: i32, h: i32, label: &str, selected: bool, t: Theme) {
        let bg = if selected { t.primary } else { t.surface_high };
        let fg = if selected { t.on_primary } else { t.on_surface };
        self.round_rect(x, y, w, h, 7, bg);
        self.text_center_x(x + w / 2, y + (h - 7) / 2, label, 1, fg);
    }

    pub(super) fn wear_row(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        let (x, w, h, r, bg, fg, icon_r) = if selected {
            (5, 230, 50, 25, t.primary_container, t.on_primary_container, 12)
        } else {
            (17, 206, 42, 21, t.surface, t.on_surface, 9)
        };
        self.round_rect(x, y, w, h, r, bg);
        let icon_bg = if selected { t.primary } else { t.surface_high };
        let icon_fg = if selected { t.on_primary } else { t.on_surface_variant };
        self.circle(x + 25, y + h / 2, icon_r, icon_bg);
        self.row_icon(label, x + 25, y + h / 2, icon_fg);
        let scale = if label.len() > 11 { 1 } else { 2 };
        self.text(x + 46, y + (h - 7 * scale) / 2, label, scale, fg);
    }

    pub(super) fn app_row(&mut self, y: i32, label: &str, selected: bool, icon: u8, t: Theme) {
        let (x, w, h, r, cx, icon_r, bg, fg) = if selected {
            (8, 224, 48, 24, 35, 15, t.primary_container, t.on_primary_container)
        } else {
            (17, 206, 42, 21, 42, 12, t.surface, t.on_surface)
        };
        self.round_rect(x, y, w, h, r, bg);
        self.circle(cx, y + h / 2, icon_r, Self::app_accent(icon));
        let icon_fg = rgb565(10, 34, 24);
        self.app_icon(icon, cx, y + h / 2, icon_fg);
        let scale = if label.len() > 12 { 1 } else { 2 };
        self.text(if selected { 64 } else { 70 }, y + (h - 7 * scale) / 2, label, scale, fg);
    }

    pub(super) fn toggle(&mut self, x: i32, y: i32, on: bool, t: Theme) {
        let bg = if on { t.primary } else { t.surface_high };
        self.round_rect(x, y, 42, 24, 12, bg);
        self.circle(if on { x + 30 } else { x + 12 }, y + 12, 8, if on { t.on_primary } else { t.on_surface_variant });
    }

    pub(super) fn slider(&mut self, x: i32, y: i32, w: i32, v: u8, t: Theme) {
        self.round_rect(x, y, w, 10, 5, t.surface_high);
        let active = ((w - 10) * v as i32 / 100).max(0);
        self.round_rect(x, y, active + 10, 10, 5, t.primary);
        self.circle(x + 5 + active, y + 5, 8, t.primary);
    }

    pub(super) fn quick_button(&mut self, cx: i32, cy: i32, label: &str, on: bool, selected: bool, t: Theme) {
        // Wear OS 6 / Material 3 Expressive: grouped pill buttons expand and
        // shape-morph around the focused item.
        let w = if selected { 92 } else { 82 };
        let h = if selected { 54 } else { 48 };
        let bg = if on { t.primary } else if selected { t.primary_container } else { t.surface_high };
        let fg = if on { t.on_primary } else if selected { t.on_primary_container } else { t.on_surface };
        self.round_rect(cx - w / 2, cy - h / 2, w, h, h / 2, bg);
        let ix = cx - 22;
        match label {
            "WIFI" => self.icon_wifi(ix, cy, fg),
            "BT" => self.icon_bt(ix, cy, fg),
            "DND" => self.icon_dnd(ix, cy, fg),
            "AIR" => self.icon_airplane(ix, cy, fg),
            "LIGHT" => self.icon_sun(ix, cy, fg),
            "SET" => self.icon_settings(ix, cy, fg),
            _ => self.circle(ix, cy, 7, fg),
        }
        self.text(cx - 3, cy - 4, label, 1, fg);
    }

    pub(super) fn info_card(&mut self, y: i32, title: &str, value: &str, active: bool, t: Theme) {
        self.round_rect(16, y, 208, 52, 26, if active { t.primary_container } else { t.surface });
        self.circle(42, y + 26, 10, if active { t.primary } else { t.surface_high });
        self.text(64, y + 10, title, 1, if active { t.on_primary_container } else { t.on_surface_variant });
        self.text(64, y + 28, value, 1, if active { t.primary } else { t.on_surface });
    }
}
