// Модуль отрисовки общей клавиатуры.
//! One compact phone-style keyboard surface for every on-device text editor.
use super::*;
use crate::text_input::{CANCEL, DELETE, LANGUAGE, MODE, SHIFT, SPACE, SUBMIT};
impl Surface {
    /// Подбирает видимый конец текста по ширине без разрыва UTF-8.
    fn text_field_tail<'a>(value: &'a str, width: i32) -> &'a str {
        let mut start = 0;
        while start < value.len() && Self::text_width(&value[start..], 1) > width {
            start += value[start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }
        &value[start..]
    }
    /// Рисует общую RU/EN-клавиатуру и состояние редактора.
    pub(super) fn text_keyboard(
        &mut self,
        f: MaterialFrame,
        t: Theme,
        title: &str,
        value: &str,
        _done: &str,
    ) {
        let keyboard = f.keyboard;
        let file_editor = matches!(f.app.id, crate::apps::FILES | crate::apps::EDITOR);
        let organizer = matches!(f.app.id, crate::apps::TASKS | crate::apps::EXPENSES);
        let count = keyboard.character_count();
        let header = match f.keyboard_error {
            1 => "Лимит текста",
            2 => "Проверьте ввод",
            3 if file_editor => f.app.files.message,
            _ => title,
        };
        self.text_fitted_raw(
            10,
            29,
            header,
            1,
            if file_editor { 126 } else { 156 },
            if f.keyboard_error == 0 {
                t.on_surface
            } else {
                t.error
            },
        );
        let bytes = if organizer { f.app.organizer.bytes } else if file_editor {
            f.app.files.bytes
        } else if f.app.id == crate::apps::WEATHER {
            u16::from(f.app.weather_editor.len)
        } else if f.name_editing {
            u16::from(f.name_draft_len)
        } else {
            u16::from(f.wifi_password_len)
        };
        let limit = if organizer { 128 } else if file_editor { f.app.files.limit } else if f.name_editing { 32 } else { 64 };
        self.text_raw(
            if file_editor { 142 } else { 177 },
            29,
            &alloc::format!("{}/{}", bytes, limit),
            1,
            t.on_surface_variant,
        );
        // A light pill mirrors the input field in the supplied Gboard reference.
        self.round_rect(8, 49, 224, 32, 16, t.on_surface);
        self.text_raw(17, 58, Self::text_field_tail(value.rsplit('\n').next().unwrap_or(""), 202), 1, t.bg);
        // Suggestions are explicit crown targets, never silent replacements.
        for (index, word) in keyboard.suggestions.iter().enumerate() {
            if word.is_empty() {
                continue;
            }
            let x = 5 + index as i32 * 78;
            let selected = usize::from(keyboard.focus) == count + 7 + index;
            self.round_rect(
                x,
                87,
                74,
                26,
                10,
                Self::focus_fill(t.surface_high, selected),
            );
            self.text_fitted_raw(x + 5, 93, word, 1, 64, Self::focus_ink(t, selected));
        }
        let mut base = 0;
        for (row, length) in keyboard.rows().into_iter().enumerate() {
            if length == 0 {
                continue;
            }
            let gap = 2;
            let width = ((228 - (length as i32 - 1) * gap) / length as i32).min(24);
            let row_width = length as i32 * width + (length as i32 - 1) * gap;
            let left = (WIDTH as i32 - row_width) / 2;
            for col in 0..length {
                let index = base + col;
                if index >= count {
                    break;
                }
                let mut bytes = [0; 4];
                let ch = keyboard.character(index).unwrap_or(' ');
                let label = if ch == '\n' { "NL" } else { ch.encode_utf8(&mut bytes) };
                self.keyboard_key(
                    left + col as i32 * (width + gap),
                    119 + row as i32 * 36,
                    width,
                    31,
                    label,
                    usize::from(keyboard.focus) == index,
                    t,
                );
            }
            base += length;
        }
        // One service row: case, symbols, language, erase, cancel, space, submit.
        let controls = [
            (SHIFT, 4, 28, Symbol::Shift),
            (MODE, 34, 36, Symbol::Grid),
            (LANGUAGE, 72, 28, Symbol::Language),
            (DELETE, 102, 30, Symbol::Backspace),
            (CANCEL, 134, 28, Symbol::Back),
            (SPACE, 164, 40, Symbol::Space),
            (SUBMIT, 206, 30, Symbol::Done),
        ];
        for (action, x, width, icon) in controls {
            let selected = keyboard.selected_action(action);
            let base = if action == SUBMIT || (action == SHIFT && keyboard.upper) {
                t.primary
            } else {
                t.surface_high
            };
            self.round_rect(x, 234, width, 37, 10, Self::focus_fill(base, selected));
            let ink = if selected {
                Self::focus_ink(t, true)
            } else if action == SUBMIT || (action == SHIFT && keyboard.upper) {
                t.on_primary
            } else {
                t.on_surface
            };
            if action == SHIFT && keyboard.page != 0 {
                self.text_center_x(x + width / 2, 246, if keyboard.page == 1 { "1/2" } else { "2/2" }, 1, ink);
            } else if action == MODE {
                self.text_center_x(
                    x + width / 2,
                    246,
                    if keyboard.page == 0 { "123" } else { "АБВ" },
                    1,
                    ink,
                );
            } else {
                self.symbol(icon, x + width / 2, 252, ink);
            }
        }
    }
}
