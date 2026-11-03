// Модуль экранов запуска и питания.
//! Boot surfaces are distinct from application screens and do not claim USB fastboot support.
use super::*;
impl Surface {
    /// Рисует стартовый логотип без зависимости от системного пакета ресурсов.
    pub fn boot_frame(&mut self, phase: u8) {
        self.fill_rect(0, 0, 240, 280, 0);

        const POSITIONS: [(i32, i32); 16] = [
            (40, 0), (37, 15), (28, 28), (15, 37),
            (0, 40), (-15, 37), (-28, 28), (-37, 15),
            (-40, 0), (-37, -15), (-28, -28), (-15, -37),
            (0, -40), (15, -37), (28, -28), (37, -15),
        ];

        let colors = [
            rgb565(66, 133, 244),
            rgb565(234, 67, 53),
            rgb565(251, 188, 5),
            rgb565(52, 168, 83),
        ];

        let step = usize::from(phase) % POSITIONS.len();

        for (index, color) in colors.iter().enumerate() {
            let position = (step + index * 4) % POSITIONS.len();
            let (dx, dy) = POSITIONS[position];

            self.circle(
                120 + dx,
                112 + dy,
                7,
                *color,
            );
        }

        self.text_center(185, "Wear OS", 2, 0xffff);
        self.text_center(
            207,
            "ESP32-S3",
            1,
            rgb565(145, 145, 145),
        );
    }
    /// Рисует стартовый экран и текущий этап подготовки.
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
    /// Рисует меню fastboot/recovery с выбранной строкой.
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
            ["Перезагрузка", "Обновление USB", "Выключить", "Назад"]
        };
        for (i, label) in labels.iter().enumerate() {
            let y = 88 + i as i32 * 34;

            if cursor as usize == i {
                self.fill_rect(
                    8, y - 6, 224, 29,
                    if kind == 1 { green } else { orange },
                );
            }

            self.text(
                16,
                y,
                *label,
                1,
                if cursor as usize == i { 0 } else { 0xffff },
            );
        }
        self.text_center(245, "Колёсико: выбор / Нажатие: OK", 1, 0xffff);
        self.text_center(265, "Удерживать 10 с: перезапуск", 1, 0xffff);
    }
    /// Рисует экран питания с актуальным состоянием и фокусом.
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
