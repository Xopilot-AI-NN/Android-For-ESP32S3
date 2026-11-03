// Модуль погоды, мирового времени и дыхания.
//! Current conditions from the on-watch HTTP client; errors never invent readings.
use super::*;
use core::fmt::Write;
impl Surface {

    /// Рисует экран погоды с актуальным состоянием и фокусом.
    pub(super) fn weather_page(&mut self, f: MaterialFrame, t: Theme) {
        let editor = f.app.weather_editor;
        if editor.active {
            let value = core::str::from_utf8(&editor.text[..usize::from(editor.len)]).unwrap_or("");
            self.text_keyboard(f, t, "Поиск города", value, "Поиск");
            return;
        }
        self.photo_header(f, "Погода", t);
        let data = f.app.weather;
        if data.valid {
            let city = core::str::from_utf8(&data.city[..usize::from(data.city_len)]).unwrap_or("");
            self.text_center_raw(58, city, 2, t.on_surface);
            self.round_rect(14, 85, 212, 99, 30, t.primary_container);
            let mut temp = heapless::String::<24>::new();
            let _ = write!(
                temp,
                "{}°",
                (data.temp + if data.temp >= 0 { 5 } else { -5 }) / 10
            );
            self.text_center_raw(100, &temp, 3, t.on_primary_container);
            self.text_center(
                153,
                crate::weather::description(data.code),
                1,
                t.on_primary_container,
            );
            let mut details = heapless::String::<64>::new();
            let _ = write!(
                details,
                "Ощущается {}° · Влажность {}%",
                (data.feels + if data.feels >= 0 { 5 } else { -5 }) / 10,
                data.humidity
            );
            self.text_center_raw(187, &details, 1, t.on_surface_variant);
            details.clear();
            let _ = write!(details, "Ветер {}.{} м/с", data.wind / 10, data.wind % 10);
            self.text_center_raw(207, &details, 1, t.on_surface_variant);
        } else {
            self.circle(120, 109, 35, t.primary_container);
            self.symbol(Symbol::Weather, 120, 109, t.primary);
            self.text_center(159, "Выберите город", 2, t.on_surface);
        }
        let status = match data.status {
            1 => "Получение данных…",
            3 => "Подключитесь к Wi-Fi",
            4 => "Ошибка DNS или соединения",
            5 => "Город не найден",
            6 => "Сервис не ответил вовремя",
            7 => "Ошибка ответа сервиса",
            _ if data.valid
                && esp_hal::time::Instant::now()
                    .duration_since_epoch()
                    .as_secs()
                    .saturating_sub(data.updated)
                    > 3600 =>
            {
                "Данные старше часа"
            }
            _ if data.approximate => "Место по IP · приблизительно",
            _ => "Прогноз Open-Meteo",
        };
        self.text_center(227, status, 1, t.on_surface_variant);
        for (index, label) in ["Город", "Рядом", "Обнов."].iter().enumerate() {
            self.keyboard_key(
                8 + index as i32 * 77,
                247,
                70,
                29,
                label,
                usize::from(editor.cursor) == index,
                t,
            );
        }
    }
}
