// Сохранение и восстановление пользовательских настроек и локального PIN.
use super::*;
#[cfg(feature = "rhai-runtime")]
impl Runtime {
    /// Возвращает выбранную чувствительность колёсика.
    pub fn crown_sensitivity(&self) -> u8 {
        self.crown_sensitivity
    }
    /// Возвращает направление вращения колёсика.
    pub fn crown_reversed(&self) -> bool {
        self.crown_reverse
    }
    /// Кодирует пользовательские параметры в версионированный формат.
    pub fn preference_config(&self) -> [u8; 20] {
        [
            5,
            self.crown_sensitivity,
            u8::from(self.crown_reverse),
            u8::from(self.high_contrast),
            self.watch_face,
            u8::from(self.wallpaper_enabled),
            u8::from(self.launcher_grid),
            u8::from(self.aod_enabled),
            self.wallpaper_index,
            self.wallpaper_level,
            u8::from(self.show_seconds),
            0,
            u8::from(self.bedtime_auto),
            self.bedtime_brightness,
            self.bedtime_start as u8,
            (self.bedtime_start >> 8) as u8,
            self.bedtime_end as u8,
            (self.bedtime_end >> 8) as u8,
            0,
            0,
        ]
    }
    /// Восстанавливает совместимые версии пользовательских параметров.
    pub fn restore_preferences(&mut self, raw: &[u8]) {
        if ((raw.len() == 4 && raw[0] == 1)
            || (raw.len() == 8 && raw[0] == 2)
            || (raw.len() == 12 && matches!(raw[0], 3 | 4))
            || (raw.len() == 20 && raw[0] == 5))
            && raw[1] <= 2
            && raw[2] <= 1
            && raw[3] <= 1
        {
            self.crown_sensitivity = raw[1];
            self.crown_reverse = raw[2] != 0;
            self.high_contrast = raw[3] != 0;
            if raw.len() >= 8 {
                self.watch_face = raw[4].min(4);
                self.wallpaper_enabled = raw[5] != 0;
            }
            if raw.len() >= 12 {
                self.launcher_grid = raw[6] != 0;
                self.aod_enabled = raw[7] != 0;
                self.wallpaper_index = raw[8].min(crate::wallpaper::builtin_count());
                if raw[0] >= 4 {
                    self.wallpaper_level = raw[9].clamp(40, 100);
                    self.show_seconds = raw[10] != 0;
                }
            }
            if raw.len() == 20 {
                let start = u16::from_le_bytes([raw[14], raw[15]]);
                let end = u16::from_le_bytes([raw[16], raw[17]]);
                if raw[12] <= 1 && (30..=40).contains(&raw[13]) && start < 1440 && end < 1440 {
                    self.bedtime_auto = raw[12] != 0;
                    self.bedtime_brightness = raw[13];
                    self.bedtime_start = start;
                    self.bedtime_end = end;
                }
            }
        }
    }
    /// Отмечает успешную запись параметров PIN.
    pub fn security_saved(&mut self) {
        self.security_saved = self.security.encode();
    }
    /// Возвращает сериализованные параметры локальной блокировки.
    pub fn security_config(&self) -> [u8; 58] {
        self.security.encode()
    }
    /// Проверяет и восстанавливает сохранённую конфигурацию PIN.
    pub fn restore_security(&mut self, raw: &[u8]) {
        if self
            .security
            .restore(raw, Instant::now().duration_since_epoch().as_secs())
        {
            self.security_saved = self.security.encode();
        } else {
            self.security_storage_error();
        }
    }
    /// Сообщает интерфейсу об ошибке хранилища PIN.
    pub fn security_storage_error(&mut self) {
        self.security.has_pin = true;
        self.security.lock();
        self.security.view = 7;
    }
    /// Обновляет доступность постоянного хранения параметров PIN.
    pub fn set_security_storage(&mut self, available: bool, entropy: [u8; 16]) {
        self.security.storage = available;
        self.security.entropy(entropy);
    }
}
