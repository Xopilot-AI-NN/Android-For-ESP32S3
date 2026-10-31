//! pages settings Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn system_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SYSTEM", t);
        let labels = ["DATE & TIME", "ABOUT", "DEVELOPER OPTIONS", "BACK"];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            self.wear_row(55 + row as i32 * 64, labels[i], i == cur, t);
        }
    }

    pub(super) fn date_time(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DATE & TIME", t);
        let labels = ["AUTOMATIC TIME", "SET HOUR", "SET MINUTE", "TIME ZONE", "BACK"];
        let cur = f.cursor.min(4) as usize;
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 56 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.toggle(174, y + 13, f.auto_time, t); }
            if i == 1 || i == 2 {
                let tm = Self::time_text(f.time_minutes);
                let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
                self.text(167, y + 19, ts, 1, if f.auto_time { t.outline } else { t.primary });
            }
            if i == 3 {
                if f.timezone_hours == 0 { self.text(178, y + 19, "UTC", 1, t.on_surface_variant); }
                else if f.timezone_hours > 0 { self.text(166, y + 19, "UTC+", 1, t.on_surface_variant); }
                else { self.text(166, y + 19, "UTC-", 1, t.on_surface_variant); }
            }
        }
        if f.auto_time && !f.time_valid { self.text_center(251, "SYNCING OVER WI-FI / PHONE", 1, t.primary); }
    }

    pub(super) fn unavailable_page(&mut self, title: &str, icon: &str, body: &str, t: Theme) {
        self.title(title, t);
        self.circle(120, 103, 37, t.surface);
        self.text_center(88, icon, 3, t.primary);
        self.text_center(157, body, 1, t.on_surface);
        self.text_center(180, "NOT PRESENT ON THIS BOARD", 1, t.on_surface_variant);
        self.round_rect(45, 217, 150, 38, 19, t.surface_high);
        self.text_center(229, "BACK", 1, t.on_surface);
    }

    pub(super) fn sound_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.unavailable_page("SOUND & VIBRATION", "S", "AUDIO HARDWARE", t);
    }

    pub(super) fn gestures_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.title("GESTURES", t);
        self.info_card(65, "CROWN", "ROTATE TO SCROLL", true, t);
        self.info_card(132, "CROWN PRESS", "SELECT / APPS", false, t);
        self.info_card(199, "HOME", "RETURN TO WATCH FACE", false, t);
    }

    pub(super) fn accessibility_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.title("ACCESSIBILITY", t);
        self.info_card(68, "INPUT", "CROWN-ONLY MODE", true, t);
        self.info_card(135, "CONTRAST", "DARK WATCH UI", false, t);
        self.round_rect(45, 213, 150, 40, 20, t.surface_high);
        self.text_center(226, "BACK", 1, t.on_surface);
    }

    pub(super) fn security_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SECURITY", t);
        self.round_rect(18, 65, 204, 64, 32, if f.cursor == 0 { t.primary_container } else { t.surface });
        self.text(42, 78, "SCREEN LOCK", 1, t.on_surface);
        self.text(42, 99, if f.locked { "ENABLED" } else { "NONE" }, 1, t.on_surface_variant);
        self.toggle(169, 85, f.locked, t);
        self.round_rect(45, 194, 150, 42, 21, if f.cursor == 1 { t.primary_container } else { t.surface });
        self.text_center(207, "BACK", 1, t.on_surface);
    }

    pub(super) fn apps_notifications_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("APPS & NOTIFS", t);
        let labels = ["NOTIFICATIONS", "APP INFO", "BACK"];
        let cur = f.cursor.min(2) as usize;
        for i in 0..3 {
            let y = 57 + i as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.text(176, y + 19, if f.notes > 0 { "NEW" } else { "0" }, 1, t.on_surface_variant); }
            if i == 1 { self.text(176, y + 19, "6", 1, t.on_surface_variant); }
        }
    }

    pub(super) fn developer_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DEVELOPER OPTIONS", t);
        let labels = ["WIRELESS DEBUG", "BUILD NUMBER", "SYSTEM", "BACK"];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.toggle(174, y + 13, f.adb, t); }
            if i == 1 { self.text(157, y + 19, "17.2.1", 1, t.primary); }
        }
        if f.adb { self.text_center(253, "WIRELESS ADB IS A DEVELOPER FEATURE", 1, t.error); }
    }
}
