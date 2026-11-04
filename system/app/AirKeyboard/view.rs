//! The preview glyph is composed at the caret; it is never part of View.text.
use super::*;
use crate::apps::air_keyboard::{
    Output, View as AirView,
    keys::{self, Key},
};

// Two bounded passes let the viewport follow the caret without allocating a
// second text buffer or temporarily inserting the hovered character.
fn flow(
    text: &str,
    caret: usize,
    preview: Option<char>,
    mut paint: impl FnMut(i32, i32, char, bool),
) -> (i32, i32) {
    let (mut row, mut x) = (0, 0);
    let mut cursor = (0, 0);
    let mut glyph = |c: char, temporary: bool, row: &mut i32, x: &mut i32| {
        let width = body_character_width(c);
        if *x + width > 208 {
            *x = 0;
            *row += 1;
        }
        paint(*row, *x, c, temporary);
        *x += width;
    };
    for (index, c) in text
        .char_indices()
        .chain(core::iter::once((text.len(), '\0')))
    {
        if index == caret {
            if preview.is_some_and(|c| x + body_character_width(c) > 208) {
                x = 0;
                row += 1;
            }
            cursor = (row, x);
            if let Some(c) = preview {
                glyph(c, true, &mut row, &mut x);
            }
        }
        match c {
            '\0' => break,
            '\n' => {
                row += 1;
                x = 0;
            }
            '\t' => {
                for _ in 0..4 {
                    glyph(' ', false, &mut row, &mut x);
                }
            }
            _ => glyph(c, false, &mut row, &mut x),
        }
    }
    cursor
}
impl Surface {
    fn air_editor(&mut self, v: AirView, height: i32, t: Theme) {
        let text = core::str::from_utf8(&v.text[..v.len as usize]).unwrap_or("");
        let (row, x) = flow(text, v.caret as usize, v.preview, |_, _, _, _| {});
        let visible = (height - 12) / 20;
        let start = (row - visible + 1).max(0);
        self.round_rect(8, 78, 224, height, 12, t.surface_high);
        flow(
            text,
            v.caret as usize,
            v.preview,
            |line, px, c, temporary| {
                if line < start || line >= start + visible {
                    return;
                }
                let (x, y) = (16 + px, 84 + (line - start) * 20);
                if temporary {
                    self.round_rect(
                        x - 1,
                        y - 2,
                        body_character_width(c) + 2,
                        20,
                        3,
                        t.primary_container,
                    );
                }
                let mut bytes = [0; 4];
                self.text_raw(
                    x,
                    y,
                    c.encode_utf8(&mut bytes),
                    1,
                    if temporary { t.primary } else { t.on_surface },
                );
            },
        );
        self.round_rect(14 + x, 84 + (row - start) * 20, 1, 16, 0, t.primary);
    }
    pub(super) fn air_keyboard_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.air_keyboard;
        self.photo_header(f, "Air Keyboard", t);
        if v.menu {
            let labels = [
                if v.inline {
                    "Вид: Inline"
                } else {
                    "Вид: полная раскладка"
                },
                if v.russian {
                    "Язык: RU (ЙЦУКЕН)"
                } else {
                    "Язык: EN (QWERTY)"
                },
                match v.output {
                    Output::Local => "Вывод: только часы",
                    Output::Hid => "Вывод: BLE HID",
                    Output::Mirror => "Вывод: часы + BLE",
                },
                if v.armed {
                    "Отправка: включена"
                } else {
                    "Отправка: выключена"
                },
                "Сопряжение Bluetooth",
                "Экспорт /AirKeyboard.txt",
                if v.clear_confirm {
                    "Подтвердить очистку"
                } else {
                    "Очистить черновик"
                },
                match v.modifiers {
                    1 => "Модификатор: Ctrl",
                    4 => "Модификатор: Alt",
                    5 => "Модификатор: Ctrl+Alt",
                    _ => "Модификаторы: выкл.",
                },
                "Калибровка: MPU6050 нет",
                if v.discard_confirm {
                    "Подтвердить выход"
                } else {
                    "Закрыть приложение"
                },
            ];
            let start = usize::from(v.menu_index).saturating_sub(2).min(5);
            for (row, index) in (start..10).take(5).enumerate() {
                self.action_chip(
                    66 + row as i32 * 37,
                    33,
                    labels[index],
                    v.menu_index as usize == index,
                    t,
                );
            }
        } else {
            self.text_fitted_raw(
                12,
                57,
                match (v.output, v.inline) {
                    (Output::Local, false) => "Full · тест энкодером",
                    (Output::Local, true) => "Inline · тест энкодером",
                    (_, false) => {
                        if v.armed {
                            "Full · BLE вкл. · тест"
                        } else {
                            "Full · BLE выкл. · тест"
                        }
                    }
                    (_, true) => {
                        if v.armed {
                            "Inline · BLE вкл. · тест"
                        } else {
                            "Inline · BLE выкл. · тест"
                        }
                    }
                },
                1,
                216,
                t.on_surface_variant,
            );
            self.air_editor(v, if v.inline { 170 } else { 54 }, t);
            if !v.inline {
                let mut index = 0;
                for row in 0..5 {
                    let n = keys::row_len(v.russian, v.symbols, row);
                    let width = (224 / n as i32).min(34);
                    let left = (240 - width * n as i32) / 2;
                    for col in 0..n {
                        let key = keys::key(v.russian, v.symbols, index, v.shift);
                        let selected = index == v.selected as usize;
                        let x = left + col as i32 * width;
                        let y = 139 + row as i32 * 24;
                        self.round_rect(
                            x,
                            y,
                            width - 2,
                            22,
                            5,
                            if selected { t.primary } else { t.surface_high },
                        );
                        let mut bytes = [0; 4];
                        let label = match key {
                            Key::Character(c) => c.encode_utf8(&mut bytes),
                            Key::Shift => {
                                if v.shift {
                                    "SH"
                                } else {
                                    "Sh"
                                }
                            }
                            Key::Language => {
                                if v.russian {
                                    "RU"
                                } else {
                                    "EN"
                                }
                            }
                            Key::Symbols => {
                                if v.symbols {
                                    "Ab"
                                } else {
                                    "123"
                                }
                            }
                            Key::Space => "_",
                            Key::Backspace => "Del",
                            Key::Enter => "Ent",
                            Key::Left => "<",
                            Key::Right => ">",
                            Key::Up => "Up",
                            Key::Down => "Dn",
                            Key::Tab => "Tb",
                            Key::Escape => "Esc",
                        };
                        if let Key::Character(c) = key {
                            // Wide Cyrillic glyphs fit the cell, but not the
                            // padded chip text box. Center them without ellipsis.
                            self.text_raw(
                                x + (width - body_character_width(c)) / 2,
                                y + 4,
                                label,
                                1,
                                Self::focus_ink(t, selected),
                            );
                        } else {
                            self.text_fitted_raw(
                                x + 3,
                                y + 4,
                                label,
                                1,
                                width - 6,
                                Self::focus_ink(t, selected),
                            );
                        }
                        index += 1;
                    }
                }
            }
        }
        self.text_fitted_raw(10, 266, v.message, 1, 220, t.on_surface_variant);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_preview_is_at_the_unicode_caret_and_moves_the_visual_caret_only() {
        let mut chars = alloc::vec::Vec::new();
        let caret = flow("АБ\nВ", "А".len(), Some('я'), |r, x, c, p| {
            chars.push((r, x, c, p))
        });
        assert_eq!(chars.iter().filter(|(_, _, _, p)| *p).count(), 1);
        assert_eq!(chars[1].2, 'я');
        assert_eq!(caret.0, 0);
        assert_eq!(caret.1, body_character_width('А'));
        let committed: alloc::string::String = chars
            .iter()
            .filter(|(_, _, _, p)| !*p)
            .map(|(_, _, c, _)| *c)
            .collect();
        assert_eq!(committed, "АБВ");
    }
}
