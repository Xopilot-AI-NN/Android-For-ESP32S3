//! One compact phone-style keyboard surface for every on-device text editor.
use super::*;
use crate::text_input::{CANCEL, DELETE, LANGUAGE, MODE, SHIFT, SPACE, SUBMIT};
impl Surface {
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
    pub(super) fn text_keyboard(
        &mut self,
        f: MaterialFrame,
        t: Theme,
        title: &str,
        value: &str,
        done: &str,
    ) {
        let keyboard = f.keyboard;
        let count = keyboard.character_count();
        let header = match f.keyboard_error {
            1 => "Лимит текста",
            2 => "Введите имя",
            _ => title,
        };
        self.text_raw(
            10,
            7,
            header,
            1,
            if f.keyboard_error == 0 {
                t.on_surface
            } else {
                t.error
            },
        );
        let bytes = if f.app.id == crate::apps::WEATHER {
            f.app.weather_editor.len
        } else if f.name_editing {
            f.name_draft_len
        } else {
            f.wifi_password_len
        };
        let limit = if f.name_editing { 32 } else { 64 };
        self.text_raw(
            177,
            7,
            &alloc::format!("{}/{}", bytes, limit),
            1,
            t.on_surface_variant,
        );
        // A light pill mirrors the input field in the supplied Gboard reference.
        self.round_rect(8, 31, 224, 37, 18, t.on_surface);
        self.text_raw(17, 42, Self::text_field_tail(value, 202), 1, t.bg);
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
                let label = keyboard
                    .character(index)
                    .unwrap_or(' ')
                    .encode_utf8(&mut bytes);
                self.keyboard_key(
                    left + col as i32 * (width + gap),
                    79 + row as i32 * 36,
                    width,
                    31,
                    label,
                    usize::from(keyboard.focus) == index,
                    t,
                );
            }
            base += length;
        }
        let shift = if keyboard.page == 0 {
            if keyboard.upper { "ABC" } else { "abc" }
        } else if keyboard.page == 1 {
            "#+="
        } else {
            "123"
        };
        self.keyboard_key(6, 194, 45, 30, shift, keyboard.selected_action(SHIFT), t);
        self.keyboard_key(
            55,
            194,
            39,
            30,
            if keyboard.page == 0 { "?123" } else { "ABC" },
            keyboard.selected_action(MODE),
            t,
        );
        self.keyboard_key(
            98,
            194,
            47,
            30,
            if keyboard.english { "EN" } else { "RU" },
            keyboard.selected_action(LANGUAGE),
            t,
        );
        self.keyboard_key(
            149,
            194,
            85,
            30,
            "Удалить",
            keyboard.selected_action(DELETE),
            t,
        );
        self.keyboard_key(6, 232, 45, 35, "Назад", keyboard.selected_action(CANCEL), t);
        self.keyboard_key(
            55,
            232,
            126,
            35,
            "Пробел",
            keyboard.selected_action(SPACE),
            t,
        );
        self.round_rect(
            185,
            232,
            49,
            35,
            17,
            Self::focus_fill(t.primary, keyboard.selected_action(SUBMIT)),
        );
        self.text_center_x(209, 245, done, 1, t.on_primary);
    }
}
