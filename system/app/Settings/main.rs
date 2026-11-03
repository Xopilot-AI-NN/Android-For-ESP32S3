// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует главное меню настроек.
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

    /// Рисует состояние Wi-Fi, Bluetooth и авиарежима.
    pub(super) fn connectivity(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Connectivity", t);
        let labels = ["Wi-Fi", "Bluetooth", "Airplane mode", "Back"];
        let values = [f.wifi, f.bt, f.airplane, false];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let index = (start + row).min(3);
            let y = 65 + row as i32 * 66;
            if index < 3 {
                self.info_card(y, labels[index], if values[index] { "ON" } else { "OFF" }, index == cur, t);
            } else {
                self.wear_row(y, labels[index], index == cur, t);
            }
        }
    }

    /// Рисует сведения об устройстве и версии без служебных декоративных заглушек.
    pub(super) fn about(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "About", t);
        let name = core::str::from_utf8(&f.device_name[..usize::from(f.device_name_len).min(32)])
            .unwrap_or(crate::device_info::DEFAULT_DEVICE_NAME);
        let labels = ["Система", "Модель", "Имя устройства", "Версия"];
        let values = [
            "AOSP Wear",
            crate::device_info::BOARD,
            name,
            crate::config::BOOTLOADER_VERSION,
        ];
        let cur = usize::from(f.cursor.min(3));
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let index = start + row;
            let y = 65 + row as i32 * 66;
            self.text(22, y, labels[index], 1, t.primary);
            self.text_fitted_raw(22, y + 23, values[index], 2, 196, t.on_surface);
        }
        self.scroll_indicator(cur as u8, 4, t);
    }
}
