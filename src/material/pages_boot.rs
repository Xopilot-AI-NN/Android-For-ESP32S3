//! Boot surfaces are distinct from application screens and do not claim USB fastboot support.
use super::*;
impl Surface {
    pub fn boot_frame(&mut self, phase: u8) {
        self.fill_rect(0, 0, 240, 280, 0);
        let progress = i32::from(phase);
        self.line(
            66,
            80,
            66 + 54 * progress / 255,
            80 + 54 * progress / 255,
            15,
            rgb565(0, 175, 255),
        );
        self.line(
            92,
            71,
            92 + 54 * progress / 255,
            71 + 54 * progress / 255,
            15,
            rgb565(255, 218, 34),
        );
        self.circle(160, 76, 8, rgb565(0, 210, 111));
        self.circle(160, 51, 8, rgb565(244, 54, 70));
        self.text_center(173, "AOSP Wear", 2, 0xffff);
        self.text_center(203, "by AOSP", 1, 0xffff);
    }
    pub fn boot_progress_frame(&mut self, label: &str, phase: u8) {
        self.boot_frame(255);
        self.round_rect(30, 236, 180, 4, 2, rgb565(40, 45, 50));
        self.round_rect(
            30,
            236,
            i32::from(phase.min(8)) * 180 / 8,
            4,
            2,
            rgb565(120, 208, 255),
        );
        self.text_center(253, label, 1, 0xffff);
    }
    pub fn boot_menu_frame(&mut self, kind: u8, cursor: u8) {
        let green = rgb565(134, 238, 167);
        let orange = rgb565(255, 179, 93);
        self.fill_rect(0, 0, 240, 280, 0);
        self.text(
            12,
            20,
            if kind == 1 {
                "FASTBOOT MODE"
            } else {
                "Android Recovery"
            },
            2,
            if kind == 1 { green } else { orange },
        );
        self.text(12, 48, "AOSP Wear / ESP32-S3", 1, 0xffff);
        let labels = if kind == 1 {
            ["Start", "Restart bootloader", "Recovery mode", "Power off"]
        } else {
            ["Перезагрузка", "Fastboot", "Выключить", "Назад"]
        };
        for (i, label) in labels.iter().enumerate() {
            let y = 88 + i as i32 * 34;
            if cursor as usize == i {
                self.fill_rect(8, y - 6, 224, 29, if kind == 1 { green } else { orange });
            }
            self.text(
                16,
                y,
                label,
                1,
                if cursor as usize == i { 0 } else { 0xffff },
            );
        }
        self.text_center(245, "Колёсико: выбор", 1, 0xffff);
        self.text_center(265, "Нажатие: подтвердить", 1, 0xffff);
    }
    pub(super) fn power_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("Питание", t);
        let locked = f.screen == 0;
        let labels = if locked {
            &["Выключить", "Перезагрузка", "Отмена"][..]
        } else {
            &[
                "Выключить",
                "Перезагрузка",
                "Циферблат",
                "Recovery",
                "Fastboot",
                "Отмена",
            ][..]
        };
        let cur = (f.cursor as usize).min(labels.len() - 1);
        let start = cur.saturating_sub(1).min(labels.len().saturating_sub(3));
        for (row, label) in labels[start..].iter().take(3).enumerate() {
            self.wear_row(67 + row as i32 * 63, label, start + row == cur, t);
        }
        self.scroll_indicator(cur as u8, labels.len() as u8, t);
    }
}
