// Экран Settings; состояние поступает из контроллера приложения.
use super::*;
#[allow(unused_imports)]
use core::fmt::Write;
impl Surface {
    /// Отрисовывает текущее состояние приложения без операций ввода-вывода.
    pub(super) fn memory_page(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        self.photo_header(f, LABELS[usize::from(a.id)], t);

                let memory = a.memory;
                let mut active = heapless::String::<64>::new();
                let _ = write!(
                    active,
                    "Активно {} / выбрано {} МиБ",
                    memory.active, memory.selected
                );
                self.text_center_raw(55, &active, 1, t.on_surface_variant);
                let start = usize::from(a.cursor).saturating_sub(1).min(3);
                for row in 0..2 {
                    let index = start + row;
                    let size = crate::swap_config::CHOICES[index];
                    let mut label = heapless::String::<24>::new();
                    let _ = write!(label, "{} МиБ", size);
                    let value = if size > memory.maximum {
                        "Недостаточно места"
                    } else if size == memory.selected {
                        "Выбрано"
                    } else {
                        "Нажмите для выбора"
                    };
                    self.info_card(
                        82 + row as i32 * 65,
                        &label,
                        value,
                        index == usize::from(a.cursor),
                        t,
                    );
                }
                self.scroll_indicator(a.cursor, 5, t);
                let status = match memory.message {
                    1 => "Сохранение…",
                    2 => "Применится после перезагрузки",
                    3 => "Недостаточно места на носителе",
                    4 => "Нет подходящей области userdata",
                    5 => "Не удалось сохранить настройку",
                    _ => "ZPager: хранилище страниц",
                };
                self.text_center(225, status, 1, t.primary);
                self.text_center(253, "Это не дополнительная RAM", 1, t.on_surface_variant);

    }
}
