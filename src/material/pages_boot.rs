// Модуль экранов запуска и питания.
//! Boot surfaces are distinct from application screens and do not claim USB fastboot support.
use super::*;
impl Surface {
    /// Рисует стартовый логотип без зависимости от системного пакета ресурсов.
    pub fn boot_frame(&mut self, phase: u8) {
        self.fill_rect(0, 0, 240, 280, 0);

        // Android head only: vector geometry, no disk I/O or bitmap allocation.
        let green = rgb565(61, 220, 132);
        self.line(95, 105, 80, 80, 4, green);
        self.line(145, 105, 160, 80, 4, green);
        self.circle(120, 150, 52, green);
        self.fill_rect(66, 151, 108, 53, 0); // flat lower edge of the head
        self.circle(100, 131, 4, 0);
        self.circle(140, 131, 4, 0);
        let tick = if phase == 255 {
            crate::startup::animation_phase()
        } else {
            phase.min(31)
        };
        // Integer smoothstep, held at full opacity once the fade finishes.
        let p = u32::from(tick);
        let alpha = (p * p * (93 - 2 * p) * 255 / (31 * 31 * 31)) as u8;
        self.text_center_raw(211, "WearOS by AOSP", 2, rgb565(alpha, alpha, alpha));
    }
    /// One clean splash; technical boot states remain in the log.
    pub fn boot_progress_frame(&mut self, _label: &str, _phase: u8) {
        self.boot_frame(255);
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
                self.fill_rect(8, y - 6, 224, 29, if kind == 1 { green } else { orange });
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn android_is_static_and_only_caption_fades_monotonically() {
        let mut surface = Surface::new();
        surface.boot_frame(0);
        let logo = surface.pixels()[..200 * WIDTH].to_vec();
        let mut caption = surface.pixels()[200 * WIDTH..].to_vec();
        assert!(caption.iter().all(|p| *p == 0));
        assert!(logo.iter().any(|p| *p == rgb565(61, 220, 132)));
        for phase in 1..=31 {
            surface.boot_frame(phase);
            assert_eq!(&surface.pixels()[..200 * WIDTH], logo.as_slice());
            let next = &surface.pixels()[200 * WIDTH..];
            assert!(next.iter().zip(&caption).all(|(a, b)| a >= b));
            caption.copy_from_slice(next);
        }
        assert!(caption.contains(&0xffff));
        let finished = surface.pixels().to_vec();
        surface.boot_frame(100);
        assert_eq!(surface.pixels(), finished);
    }
}
