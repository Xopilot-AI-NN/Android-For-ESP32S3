// Модуль экранов проводника и редактора.
use super::*;
impl Surface {
    /// Рисует экран проводника/редактора с актуальным состоянием и фокусом.
    pub(super) fn files_page(&mut self, f: MaterialFrame, t: Theme) {
        let view = f.app.files;
        let title =
            core::str::from_utf8(&view.title[..usize::from(view.title_len)]).unwrap_or("Файлы");
        if matches!(view.mode, 2 | 4) {
            let input =
                core::str::from_utf8(&view.input[..usize::from(view.input_len)]).unwrap_or("");
            self.text_keyboard(f, t, title, input, "OK");
            return;
        }
        if view.mode == 0 {
            self.file_browser(f, t);
            return;
        }
        self.photo_header(
            f,
            if view.mode == 5 {
                "Есть изменения"
            } else if view.mode == 6 {
                "Удалить?"
            } else if view.mode == 0 {
                "Файлы"
            } else {
                title
            },
            t,
        );
        if view.mode == 0 {
            self.text_center_raw(55, title, 1, t.on_surface_variant);
        }
        if view.mode == 3 {
            self.text_center(
                53,
                if view.dirty {
                    "Изменения не сохранены"
                } else {
                    "Нажмите строку: правка"
                },
                1,
                t.on_surface_variant,
            );
        }
        for (i, row) in view.rows.iter().enumerate() {
            if row.len == 0 {
                continue;
            }
            let label = core::str::from_utf8(&row.text[..usize::from(row.len)]).unwrap_or("");
            let y = 76 + i as i32 * 58;
            self.round_rect(
                12,
                y,
                216,
                51,
                20,
                Self::focus_fill(t.surface_high, row.selected),
            );
            let ink = Self::focus_ink(t, row.selected);
            if row.kind != 0 {
                self.symbol(
                    if row.kind == 2 {
                        Symbol::Folder
                    } else {
                        Symbol::Info
                    },
                    34,
                    y + 25,
                    ink,
                );
                self.text_fitted_raw(54, y + 10, label, 1, 163, ink);
                self.text_raw(
                    54,
                    y + 30,
                    &if row.kind == 2 {
                        alloc::string::String::from("Папка")
                    } else {
                        alloc::format!("{} байт", row.size)
                    },
                    1,
                    ink,
                );
            } else {
                self.text_fitted_raw(23, y + 18, label, 1, 194, ink);
            }
        }
        let status = if view.busy {
            "Чтение/сохранение…"
        } else {
            view.message
        };
        self.text_center(255, status, 1, t.on_surface_variant);
        if view.count > 3 {
            self.scroll_indicator(
                (u32::from(view.cursor) * 254 / u32::from(view.count - 1)) as u8,
                255,
                t,
            );
        }
    }

    /// Каталог в стиле Material Files: строки содержимого и отдельная панель операций.
    fn file_browser(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.files;
        self.photo_header(f, "Файлы", t);
        let path = core::str::from_utf8(&v.title[..v.title_len as usize]).unwrap_or("/");
        self.text_fitted_raw(12, 58, if path == "/" { "Документы" } else { path }, 1, 166, t.on_surface_variant);
        self.text_raw(190, 58, &alloc::format!("{}", v.items), 1, t.primary);
        if v.items == 0 {
            self.symbol(Symbol::Folder, 120, 126, t.primary);
            self.text_center(162, "Здесь пока пусто", 1, t.on_surface_variant);
        }
        for (i, row) in v.rows.iter().enumerate() {
            if row.kind == 0 || row.len == 0 { continue; }
            let y = 82 + i as i32 * 50;
            self.round_rect(8, y, 224, 46, 12, Self::focus_fill(t.surface, row.selected));
            let ink = Self::focus_ink(t, row.selected);
            let accent = if row.kind == 2 { rgb565(255, 209, 115) } else { rgb565(156, 213, 255) };
            self.round_rect(16, y + 7, 32, 32, 10, accent);
            self.symbol(if row.kind == 2 { Symbol::Folder } else { Symbol::Info }, 32, y + 23, rgb565(8,20,17));
            self.text_fitted_raw(57, y + 7, core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or(""), 1, 164, ink);
            self.text_raw(57, y + 26, &if row.kind == 2 { alloc::string::String::from("Папка") } else { alloc::format!("{} байт · текст/файл", row.size) }, 1, ink);
        }
        let selected = core::str::from_utf8(&v.selected_name[..v.selected_len as usize]).unwrap_or("");
        let hint = if v.busy { alloc::string::String::from("Чтение/сохранение…") }
            else if !v.message.is_empty() { alloc::string::String::from(v.message) }
            else { match v.toolbar { 0 => "Создать файл или папку".into(), 1 => alloc::format!("Действия: {}", selected),
                2 => if v.clipboard { "Вставить в эту папку".into() } else { "Буфер пуст".into() },
                3 => "В родительскую папку".into(), _ => "Нажать: открыть".into() } };
        self.text_fitted_raw(10, 234, &hint, 1, 220, t.on_surface_variant);
        for index in 0..4 {
            let x = 10 + index * 57;
            let focused = v.toolbar == index as u8;
            let enabled = match index { 1 => v.selected_len != 0, 2 => v.clipboard, 3 => v.can_up, _ => true };
            self.round_rect(x, 253, 49, 25, 10, Self::focus_fill(t.surface_high, focused));
            let ink = if !enabled && !focused { t.outline } else { Self::focus_ink(t, focused) };
            let cx = x + 24;
            match index {
                0 => { self.line(cx-6,265,cx+6,265,2,ink); self.line(cx,259,cx,271,2,ink); }
                1 => for dy in [-5,0,5] { self.circle(cx,265+dy,1,ink); },
                2 => { self.round_rect(cx-7,258,14,15,3,ink); self.round_rect(cx-3,256,6,4,1,t.primary); }
                _ => self.symbol(Symbol::Back,cx,265,ink),
            }
        }
    }
}
