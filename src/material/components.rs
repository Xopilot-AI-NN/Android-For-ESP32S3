// Модуль общих компонентов интерфейса.
//! components Material surface implementation.
use super::*;

impl Surface {
    /// Осветляет выбранный элемент, не меняя его геометрию.
    pub(super) fn focus_fill(color: u16, selected: bool) -> u16 {
        if !selected {
            return color;
        }
        let lift = |value: u16, max: u16| value + (max - value) * 3 / 5;
        (lift(color >> 11, 31) << 11) | (lift((color >> 5) & 63, 63) << 5) | lift(color & 31, 31)
    }
    /// Выбирает контрастный текст для обычной или светлой подложки.
    pub(super) fn focus_ink(t: Theme, selected: bool) -> u16 {
        if selected {
            rgb565(8, 20, 17)
        } else {
            t.on_surface
        }
    }
    /// Накладывает полупрозрачную строку времени, Wi-Fi и Bluetooth поверх кадра.
    pub(super) fn status_strip(&mut self, f: MaterialFrame, t: Theme) {
        // One scrim over the already painted background: 8/15 opacity.
        for y in 0..24 {
            for x in 0..WIDTH as i32 {
                self.coverage_pixel(x, y, t.bg, 8);
            }
        }
        let time = Self::time_text(f.time_minutes);
        self.text_raw(
            10,
            5,
            if f.time_valid {
                core::str::from_utf8(&time).unwrap_or("--:--")
            } else {
                "--:--"
            },
            1,
            t.on_surface,
        );
        self.icon_wifi(
            191,
            12,
            if f.wifi_link_state == 4 {
                t.primary
            } else {
                t.outline
            },
        );
        self.icon_bt(
            221,
            12,
            if matches!(f.bt_status, 2 | 3 | 4) {
                t.primary
            } else {
                t.outline
            },
        );
        if f.airplane {
            self.icon_airplane(159, 12, t.on_surface_variant);
        }
    }

    /// Рисует заголовок в общей области под строкой состояния.
    pub(super) fn title(&mut self, title: &str, t: Theme) {
        // No decorative underline: the source watch used a clean black header.
        self.text_center(
            29,
            title,
            if title.chars().count() > 19 { 1 } else { 2 },
            t.on_surface,
        );
    }

    /// Рисует единообразный заголовок приложения.
    pub(super) fn photo_header(&mut self, _frame: MaterialFrame, title: &str, t: Theme) {
        self.text_center_raw(
            29,
            title,
            if title.chars().count() > 19 { 1 } else { 2 },
            t.on_surface,
        );
    }

    /// Рисует клавишу с подсветкой фокуса и центрированной подписью.
    pub(super) fn keyboard_key(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        label: &str,
        selected: bool,
        t: Theme,
    ) {
        let bg = Self::focus_fill(t.surface_high, selected);
        let fg = Self::focus_ink(t, selected);
        self.round_rect(x, y, w, h, 7, bg);
        self.text_center_x(x + w / 2, y + (h - 14) / 2, label, 1, fg);
    }

    /// Рисует строку меню с иконкой и подходящим размером текста.
    pub(super) fn wear_row(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        // Wide, muted slate pills from the lost firmware photos.
        let (x, w, h) = (19, 202, 55);
        let bg = Self::focus_fill(t.surface_high, selected);
        self.round_rect(x, y, w, h, 27, bg);
        let cy = y + h / 2;
        self.circle(x + 28, cy, 18, t.surface);
        self.row_icon(label, x + 28, cy, t.primary);
        let scale = if Self::text_width(label, 2) <= w - 72 {
            2
        } else {
            1
        };
        let color = Self::focus_ink(t, selected);
        if scale == 1 && Self::text_width(label, 1) > w - 72 {
            self.text_wrapped(x + 57, y + h / 2 - 18, label, w - 72, color);
        } else {
            self.text(
                x + 57,
                y + (h - if scale <= 1 {
                    14
                } else {
                    18 * (scale - 1).max(1)
                }) / 2,
                label,
                scale,
                color,
            );
        }
    }

    /// Показывает положение выбранного элемента в прокручиваемом списке.
    pub(super) fn scroll_indicator(&mut self, cursor: u8, count: u8, t: Theme) {
        if count <= 3 {
            return;
        }
        /// Параметр TOP компонента общих компонентов интерфейса.
        const TOP: i32 = 76;
        /// Параметр TRACK компонента общих компонентов интерфейса.
        const TRACK: i32 = 120;
        /// Параметр THUMB компонента общих компонентов интерфейса.
        const THUMB: i32 = 24;
        self.round_rect(235, TOP, 3, TRACK, 1, t.surface_high);
        let y = TOP + i32::from(cursor.min(count - 1)) * (TRACK - THUMB) / i32::from(count - 1);
        self.round_rect(235, y, 3, THUMB, 1, t.primary);
    }

    /// Рисует строку приложения с его иконкой и названием.
    pub(super) fn app_row(&mut self, y: i32, label: &str, selected: bool, icon: u8, t: Theme) {
        let (x, w, h) = (19, 202, 54);
        self.round_rect(x, y, w, h, 26, Self::focus_fill(t.surface_high, selected));
        self.circle(
            x + 28,
            y + h / 2,
            18,
            Self::focus_fill(Self::app_accent(icon), selected),
        );
        self.app_icon(icon, x + 28, y + h / 2, rgb565(8, 20, 17));
        let scale = if Self::text_width(label, 2) > w - 72 {
            1
        } else {
            2
        };
        self.text_fitted(
            x + 58,
            y + (h - if scale <= 1 {
                14
            } else {
                18 * (scale - 1).max(1)
            }) / 2,
            label,
            scale,
            w - 72,
            Self::focus_ink(t, selected),
        );
    }

    /// Рисует переключатель с актуальным положением.
    pub(super) fn toggle(&mut self, x: i32, y: i32, on: bool, t: Theme) {
        let bg = if on { t.primary } else { t.surface_high };
        self.round_rect(x, y, 42, 24, 12, bg);
        self.circle(
            if on { x + 30 } else { x + 12 },
            y + 12,
            8,
            if on {
                t.on_primary
            } else {
                t.on_surface_variant
            },
        );
    }

    /// Рисует полосу значения и положение ползунка.
    pub(super) fn slider(&mut self, x: i32, y: i32, w: i32, v: u8, t: Theme) {
        if w < 20 {
            return;
        }
        self.round_rect(x, y, w, 8, 4, t.surface_high);
        let active = ((w - 14) * v.min(100) as i32 / 100).clamp(0, w - 14);
        self.round_rect(x, y, active + 7, 8, 4, t.primary);
        self.circle(x + 7 + active, y + 4, 11, t.primary);
    }

    /// Рисует кнопку быстрых настроек с состоянием и фокусом.
    pub(super) fn quick_button(
        &mut self,
        cx: i32,
        cy: i32,
        label: &str,
        on: bool,
        selected: bool,
        t: Theme,
    ) {
        let (w, h) = (104, 54);
        let bg = Self::focus_fill(if on { t.primary } else { t.surface_high }, selected);
        let fg = if selected {
            Self::focus_ink(t, true)
        } else if on {
            t.on_primary
        } else {
            t.on_surface
        };
        self.round_rect(cx - w / 2, cy - h / 2, w, h, 27, bg);
        match label {
            "Wi-Fi" => self.icon_wifi(cx, cy - 7, fg),
            "Bluetooth" => self.icon_bt(cx, cy - 7, fg),
            "DND" => self.icon_dnd(cx, cy - 7, fg),
            "Airplane" => self.icon_airplane(cx, cy - 7, fg),
            "Flashlight" => self.symbol(Symbol::Flashlight, cx, cy - 7, fg),
            "Settings" => self.icon_settings(cx, cy - 7, fg),
            _ => {}
        }
        self.text_center_x(cx, cy + 13, label, 1, fg);
    }

    /// Рисует карточку с раздельными строками заголовка и значения.
    pub(super) fn info_card(&mut self, y: i32, title: &str, value: &str, active: bool, t: Theme) {
        let x = 18;
        let w = 204;
        let h = 57;
        self.round_rect(x, y, w, h, h / 2, Self::focus_fill(t.surface_high, active));
        self.circle(
            x + 27,
            y + h / 2,
            17,
            if active { t.primary } else { t.surface },
        );
        self.row_icon(
            title,
            x + 27,
            y + h / 2,
            if active { t.on_primary } else { t.primary },
        );
        self.text_fitted(x + 53, y + 10, title, 1, 138, Self::focus_ink(t, active));
        self.text_fitted(
            x + 53,
            y + 31,
            value,
            1,
            138,
            if active {
                Self::focus_ink(t, true)
            } else {
                t.on_surface_variant
            },
        );
    }
}
