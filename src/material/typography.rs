// Модуль растровых шрифтов и размещения текста.
//! typography Material surface implementation.
use super::*;

/// Параметр BODY компонента растровых шрифтов и размещения текста.
const BODY: &[u8; 43824] = include_bytes!("fonts/body.bin");
/// Параметр LABEL компонента растровых шрифтов и размещения текста.
const LABEL: &[u8; 43824] = include_bytes!("fonts/label.bin");
/// Параметр BODY_WIDTHS компонента растровых шрифтов и размещения текста.
const BODY_WIDTHS: &[u8; 166] = include_bytes!("fonts/body-widths.bin");
/// Параметр DISPLAY компонента растровых шрифтов и размещения текста.
const DISPLAY: &[u8; 175296] = include_bytes!("fonts/display.bin");
/// Параметр DISPLAY_WIDTHS компонента растровых шрифтов и размещения текста.
const DISPLAY_WIDTHS: &[u8; 166] = include_bytes!("fonts/display-widths.bin");
/// Параметр LABEL_WIDTHS компонента растровых шрифтов и размещения текста.
const LABEL_WIDTHS: &[u8; 166] = include_bytes!("fonts/label-widths.bin");

impl Surface {
    /// Рисует один символ выбранного шрифта.
    pub(super) fn glyph(&mut self, x: i32, y: i32, ch: char, scale: i32, c: u16) {
        let index = Self::font_index(ch);
        let (atlas, width, height, zoom, baseline): (&[u8], usize, usize, i32, i32) = if scale == 3
        {
            (DISPLAY, 44, 48, 1, 10)
        } else if scale <= 1 {
            (BODY, 22, 24, 1, 4)
        } else {
            (LABEL, 22, 24, (scale - 1).max(1), 5)
        };
        for row in 0..height {
            for col in 0..width {
                let offset = index * width * height + row * width + col;
                let packed = atlas[offset / 2];
                let alpha = if offset % 2 == 0 {
                    packed >> 4
                } else {
                    packed & 15
                };
                if alpha == 0 { continue; }
                for dy in 0..zoom {
                    for dx in 0..zoom {
                        self.coverage_pixel(
                            x + col as i32 * zoom + dx,
                            y + (row as i32 - baseline) * zoom + dy,
                            c,
                            alpha,
                        );
                    }
                }
            }
        }
    }

    /// Выводит крупное время циферблата.
    pub(super) fn big_time(&mut self, y: i32, minutes: u16, valid: bool, color: u16) {
        let raw = Self::time_text(minutes);
        let text = if raw[0] == b'0' { &raw[1..] } else { &raw[..] };
        let _ = valid; // Validity is indicated separately on the watch face.
        self.clock_text(y, text, color);
    }

    /// Рисует цифры отдельным крупным атласом часов.
    pub(super) fn clock_text(&mut self, y: i32, text: &[u8], color: u16) {
        /// Параметр DATA компонента растровых шрифтов и размещения текста.
        const DATA: &[u8; 22176] = include_bytes!("fonts/clock.bin");
        /// Параметр WIDTHS компонента растровых шрифтов и размещения текста.
        const WIDTHS: &[u8; 11] = include_bytes!("fonts/clock-widths.bin");
        let index = |ch: u8| {
            if ch == b':' {
                10
            } else {
                usize::from(ch.saturating_sub(b'0').min(9))
            }
        };
        let width: i32 = text.iter().map(|&ch| i32::from(WIDTHS[index(ch)])).sum();
        let mut x = (WIDTH as i32 - width) / 2;
        for &ch in text {
            let glyph = index(ch);
            for row in 0..84 {
                for col in 0..48 {
                    let offset = glyph * 4032 + row * 48 + col;
                    let packed = DATA[offset / 2];
                    let alpha = if offset % 2 == 0 {
                        packed >> 4
                    } else {
                        packed & 15
                    };
                    if alpha == 0 { continue; }
                    self.coverage_pixel(x + col as i32, y + row as i32 - 14, color, alpha);
                }
            }
            x += i32::from(WIDTHS[glyph]);
        }
    }

    /// Переводит системную подпись и выводит её выбранным шрифтом.
    pub(super) fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, c: u16) {
        self.text_raw(x, y, localization::label(text), scale, c);
    }

    /// Выводит текст без перевода пользовательского содержимого.
    pub(super) fn text_raw(&mut self, x: i32, y: i32, text: &str, scale: i32, c: u16) {
        let mut cx = x;
        for ch in text.chars() {
            self.glyph(cx, y, ch, scale, c);
            cx += Self::character_width(ch, scale);
        }
    }

    /// Two lines for long localized labels; split only at UTF-8 boundaries.
    pub(super) fn text_wrapped(&mut self, x: i32, y: i32, text: &str, width: i32, color: u16) {
        let mut remaining = localization::label(text);
        for line in 0..2 {
            if line == 1 {
                self.text_fitted_raw(x, y + 19, remaining, 1, width, color);
                return;
            }
            let mut used = 0;
            let mut end = 0;
            let mut word_end = 0;
            for (i, ch) in remaining.char_indices() {
                let advance = Self::character_width(ch, 1);
                if used + advance > width {
                    break;
                }
                used += advance;
                end = i + ch.len_utf8();
                if ch == ' ' {
                    word_end = i;
                }
            }
            if end == 0 {
                return;
            }
            if end < remaining.len() && word_end != 0 {
                end = word_end;
            }
            self.text_raw(x, y + line * 19, remaining[..end].trim_end(), 1, color);
            remaining = remaining[end..].trim_start();
            if remaining.is_empty() {
                return;
            }
        }
    }

    /// Ограничивает пользовательский текст шириной, добавляя многоточие по UTF-8.
    pub(super) fn text_fitted_raw(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        scale: i32,
        width: i32,
        color: u16,
    ) {
        let value = text;
        let full: i32 = value.chars().map(|c| Self::character_width(c, scale)).sum();
        if full <= width {
            self.text_raw(x, y, value, scale, color);
            return;
        }
        let suffix = Self::character_width('…', scale);
        let mut used = 0;
        let mut end = 0;
        for (offset, ch) in value.char_indices() {
            let advance = Self::character_width(ch, scale);
            if used + advance + suffix > width {
                break;
            }
            used += advance;
            end = offset + ch.len_utf8();
        }
        self.text_raw(x, y, &value[..end], scale, color);
        self.glyph(x + used, y, '…', scale, color);
    }

    /// Переводит системную подпись и ограничивает её заданной шириной.
    pub(super) fn text_fitted(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        scale: i32,
        width: i32,
        color: u16,
    ) {
        self.text_fitted_raw(x, y, localization::label(text), scale, width, color);
    }

    /// Вычисляет ширину строки в пикселях выбранного шрифта.
    pub(super) fn text_width(text: &str, scale: i32) -> i32 {
        localization::label(text)
            .chars()
            .map(|ch| Self::character_width(ch, scale))
            .sum()
    }

    /// Находит глиф в атласе с резервным знаком для неподдерживаемого символа.
    fn font_index(ch: char) -> usize {
        match ch as u32 {
            32..=126 => ch as usize - 32,
            0x410..=0x44f => 95 + ch as usize - 0x410,
            0x401 => 159,
            0x451 => 160,
            0xb0 => 161,
            0xb7 => 162,
            0x2026 => 163,
            0x2014 => 164,
            0x2212 => 165,
            _ => (b'?' - 32) as usize,
        }
    }

    /// Возвращает реальную ширину символа для выбранного шрифта.
    fn character_width(ch: char, scale: i32) -> i32 {
        let index = Self::font_index(ch);
        if scale == 3 {
            return i32::from(DISPLAY_WIDTHS[index]);
        }
        i32::from(if scale <= 1 {
            BODY_WIDTHS[index]
        } else {
            LABEL_WIDTHS[index]
        }) * (scale - 1).max(1)
    }

    /// Центрирует пользовательскую строку, уменьшая размер при нехватке места.
    pub(super) fn text_center_raw(&mut self, y: i32, text: &str, scale: i32, c: u16) {
        let mut scale = scale;
        let width: i32 = text
            .chars()
            .map(|ch| Self::character_width(ch, scale))
            .sum();
        if width > 224 && scale > 1 {
            scale = 1;
        }
        let width: i32 = text
            .chars()
            .map(|ch| Self::character_width(ch, scale))
            .sum();
        self.text_fitted_raw(((WIDTH as i32 - width) / 2).max(8), y, text, scale, 224, c);
    }

    /// Рисует строку с горизонтальным центрированием.
    pub(super) fn text_center(&mut self, y: i32, text: &str, scale: i32, c: u16) {
        self.text_center_raw(y, localization::label(text), scale, c);
    }

    /// Центрирует системную подпись относительно заданной координаты.
    pub(super) fn text_center_x(&mut self, cx: i32, y: i32, text: &str, scale: i32, c: u16) {
        self.text(cx - Self::text_width(text, scale) / 2, y, text, scale, c);
    }

    /// Форматирует минуты суток как HH:MM.
    pub(super) fn time_text(mins: u16) -> [u8; 5] {
        let h = (mins / 60) % 24;
        let m = mins % 60;
        [
            b'0' + (h / 10) as u8,
            b'0' + (h % 10) as u8,
            b':',
            b'0' + (m / 10) as u8,
            b'0' + (m % 10) as u8,
        ]
    }

    /// Форматирует длительность в секундах как MM:SS.
    pub(super) fn mmss_text(secs: u16) -> [u8; 5] {
        let m = (secs / 60).min(99);
        let s = secs % 60;
        [
            b'0' + (m / 10) as u8,
            b'0' + (m % 10) as u8,
            b':',
            b'0' + (s / 10) as u8,
            b'0' + (s % 10) as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Проверяет кириллические глифы, пропорциональную ширину и перевод только системного текста.
    fn russian_ui_uses_cyrillic_glyphs_and_proportional_widths() {
        assert_eq!(DEFAULT_LOCALE, "ru-RU");
        assert_eq!(localization::label("Settings"), "Настройки");
        for ch in "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя".chars()
        {
            let index = Surface::font_index(ch);
            assert_ne!(index, Surface::font_index('?'));
            assert!(BODY[index * 264..(index + 1) * 264].iter().any(|&p| p != 0));
        }
        assert_eq!(
            Surface::text_width("Settings", 1),
            Surface::text_width("Настройки", 1)
        );
        let mut surface = Surface::new();
        surface.text_center_raw(10, "Settings", 1, 0xffff);
        let raw = surface.pixels().to_vec();
        surface.pixels.fill(0);
        surface.text_center(10, "Settings", 1, 0xffff);
        assert_ne!(surface.pixels(), raw.as_slice()); // User text bypasses translation.
    }
}
