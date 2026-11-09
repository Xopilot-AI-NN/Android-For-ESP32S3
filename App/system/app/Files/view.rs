//! Энкодерный вариант референса Files: путь, компактные карточки и меню без второй панели.
use super::*;
impl Surface {
    /// Векторный документ работает даже со старым атласом системных пиктограмм.
    fn file_document(&mut self, cx: i32, cy: i32, color: u16) {
        self.line(cx - 7, cy - 10, cx + 3, cy - 10, 2, color);
        self.line(cx - 7, cy - 10, cx - 7, cy + 10, 2, color);
        self.line(cx - 7, cy + 10, cx + 8, cy + 10, 2, color);
        self.line(cx + 8, cy + 10, cx + 8, cy - 5, 2, color);
        self.line(cx + 3, cy - 10, cx + 8, cy - 5, 2, color);
        self.line(cx + 3, cy - 10, cx + 3, cy - 5, 2, color);
        self.line(cx + 3, cy - 5, cx + 8, cy - 5, 2, color);
        self.line(cx - 3, cy, cx + 4, cy, 1, color);
        self.line(cx - 3, cy + 4, cx + 4, cy + 4, 1, color);
    }
    pub(super) fn files_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.files;
        let title = core::str::from_utf8(&v.title[..v.title_len as usize]).unwrap_or("Файлы");
        if matches!(v.mode, 2 | 4) {
            f.keyboard = v.keyboard;
            self.text_keyboard(
                f,
                t,
                title,
                core::str::from_utf8(&v.input[..v.input_len as usize]).unwrap_or(""),
                "OK",
            );
            return;
        }
        if v.mode == 0 {
            self.file_browser(f, t);
            return;
        }
        self.photo_header(
            f,
            if v.mode == 8 {
                "Создать"
            } else if v.mode == 10 {
                "Свойства"
            } else {
                title
            },
            t,
        );
        if v.mode == 10 {
            self.symbol(Symbol::Info, 120, 88, t.primary);
            self.text_fitted_raw(12, 125, title, 1, 216, t.on_surface);
            self.text_center_raw(
                164,
                &alloc::format!("{} байт", v.selected_size),
                1,
                t.primary,
            );
            self.text_center_raw(200, "Локальный файл / папка", 1, t.on_surface_variant);
            self.action_chip(237, 36, "Назад", true, t);
            return;
        }
        if v.mode == 3 {
            if v.dirty {
                self.circle(16, 65, 3, t.primary);
            }
            self.text_raw(
                25,
                57,
                if v.dirty {
                    "Изменено"
                } else {
                    "Сохранено"
                },
                1,
                t.on_surface_variant,
            );
            self.symbol(Symbol::Done, 220, 42, t.primary);
            self.line(29, 83, 29, 233, 1, t.outline);
            for (i, row) in v.rows.iter().enumerate() {
                if row.len == 0 {
                    continue;
                }
                let index = usize::from(v.first_row) + i;
                let y = 86 + i as i32 * 24;
                let selected = row.selected;
                if selected {
                    self.round_rect(34, y - 3, 196, 23, 5, t.surface_high);
                }
                if index < usize::from(v.count.saturating_sub(3)) {
                    self.text_raw(
                        6,
                        y,
                        &alloc::format!("{}", index + 1),
                        1,
                        t.on_surface_variant,
                    );
                }
                let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
                self.text_fitted_raw(
                    37,
                    y,
                    text,
                    1,
                    190,
                    if selected { t.primary } else { t.on_surface },
                );
            }
            self.line(0, 246, 240, 246, 1, t.outline);
            self.text_raw(
                10,
                257,
                &alloc::format!("Стр: {} · {} Б", v.cursor + 1, v.bytes),
                1,
                t.on_surface_variant,
            );
            self.text_raw(186, 257, "UTF-8", 1, t.on_surface_variant);
            return;
        }
        for (i, row) in v.rows.iter().enumerate() {
            if row.len == 0 {
                continue;
            }
            let y = 70 + i as i32 * 32;
            let selected = row.selected;
            self.round_rect(
                12,
                y,
                216,
                29,
                13,
                if selected { t.primary } else { t.surface_high },
            );
            let ink = Self::focus_ink(t, selected);
            let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
            self.text_fitted_raw(23, y + 7, text, 1, 195, ink);
        }
        if !v.message.is_empty() {
            self.text_fitted_raw(10, 267, v.message, 1, 220, t.on_surface_variant);
        }
    }
    fn file_browser(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.files;
        self.photo_header(f, "Файлы", t);
        for dy in [-5, 0, 5] {
            self.circle(220, 40 + dy, 1, t.on_surface_variant);
        }
        self.round_rect(12, 59, 216, 24, 11, t.surface_high);
        self.symbol(Symbol::Folder, 25, 71, t.on_surface_variant);
        let path = core::str::from_utf8(&v.title[..v.title_len as usize]).unwrap_or("/");
        self.text_fitted_raw(
            42,
            63,
            if v.busy { "Чтение…" } else { path },
            1,
            175,
            t.on_surface_variant,
        );
        if v.items == 0 {
            self.symbol(Symbol::Folder, 120, 139, t.primary);
            self.text_center_raw(174, "Здесь пока пусто", 1, t.on_surface_variant);
        }
        let selected_name =
            core::str::from_utf8(&v.selected_name[..v.selected_len as usize]).unwrap_or("");
        let selected_row = v
            .rows
            .iter()
            .take(4)
            .position(|row| row.selected)
            .or_else(|| {
                v.rows.iter().take(4).position(|row| {
                    row.len > 0
                        && core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("")
                            == selected_name
                })
            })
            .unwrap_or(0);
        let top = selected_row.saturating_sub(2);
        for (i, row) in v.rows.iter().take(4).skip(top).take(3).enumerate() {
            if row.kind == 0 || row.len == 0 {
                continue;
            }
            let y = 91 + i as i32 * 51;
            let selected = row.selected;
            let ink = Self::focus_ink(t, selected);
            self.round_rect(
                12,
                y,
                216,
                46,
                16,
                if selected { t.primary } else { t.surface },
            );
            let icon_ink = if selected { ink } else { t.on_surface_variant };
            if row.kind == 2 {
                self.symbol(Symbol::Folder, 31, y + 23, icon_ink);
            } else {
                self.file_document(31, y + 23, icon_ink);
            }
            let name = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
            self.text_fitted_raw(52, y + 6, name, 1, 162, ink);
            if row.kind == 1 {
                let size = if row.size >= 1024 * 1024 {
                    alloc::format!(
                        "{}.{}M",
                        row.size / 1048576,
                        (row.size % 1048576) * 10 / 1048576
                    )
                } else if row.size >= 1024 {
                    alloc::format!("{}K", row.size / 1024)
                } else {
                    alloc::format!("{}B", row.size)
                };
                self.text_fitted_raw(
                    52,
                    y + 27,
                    &size,
                    1,
                    150,
                    if selected {
                        t.on_primary
                    } else {
                        t.on_surface_variant
                    },
                );
            } else {
                self.text_raw(
                    52,
                    y + 27,
                    "Папка",
                    1,
                    if selected {
                        t.on_primary
                    } else {
                        t.on_surface_variant
                    },
                );
                self.symbol(Symbol::Next, 214, y + 23, ink);
            }
        }
        for index in 0..4 {
            let x = 12 + index as i32 * 55;
            let selected = v.toolbar == index as u8;
            self.round_rect(
                x,
                249,
                51,
                28,
                12,
                if selected { t.primary } else { t.surface_high },
            );
            let ink = Self::focus_ink(t, selected);
            let cx = x + 26;
            match index {
                0 => {
                    self.line(cx - 6, 263, cx + 6, 263, 2, ink);
                    self.line(cx, 257, cx, 269, 2, ink);
                }
                1 => {
                    for dy in [-5, 0, 5] {
                        self.circle(cx, 263 + dy, 1, ink);
                    }
                }
                2 => self.symbol(Symbol::Share, cx, 263, ink),
                _ => self.symbol(Symbol::Back, cx, 263, ink),
            }
        }
    }
}
