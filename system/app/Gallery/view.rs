use super::*;
impl Surface {
    /// Показ только реального preview; во время I/O предыдущая картинка не выдаётся за новую.
    pub(super) fn gallery_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.gallery;
        if v.source == 2 {
            self.remote_app_page(f, t);
            return;
        }
        self.photo_header(f, "Галерея", t);
        if v.source == 0 {
            self.symbol(Symbol::Folder, 120, 87, t.primary);
            self.text_center_raw(124, "Ваши фотографии", 1, t.on_surface_variant);
            self.action_chip(163, 43, "На часах", v.cursor == 0, t);
            self.action_chip(217, 43, "Google Фото", v.cursor == 1, t);
            return;
        }
        if v.image && !v.busy {
            if let Some(pixels) = crate::apps::gallery::pixels() {
                if pixels.len() == 240 * 200 {
                    for y in 0..200 {
                        for x in 0..240 {
                            self.pixel(x as i32, y as i32 + 54, pixels[y * 240 + x]);
                        }
                    }
                }
            }
        } else if v.total == 0 {
            self.empty_state(Symbol::Folder, "Галерея пуста", "Фото через ADB", t);
        } else {
            let start = usize::from(v.cursor)
                .saturating_sub(1)
                .min(usize::from(v.total).saturating_sub(3));
            for i in 0..3 {
                if v.lengths[i] != 0 {
                    let name =
                        core::str::from_utf8(&v.names[i][..v.lengths[i] as usize]).unwrap_or("");
                    self.wear_row(
                        70 + i as i32 * 55,
                        name,
                        start + i == usize::from(v.cursor),
                        t,
                    );
                }
            }
        }
        if !v.message.is_empty() {
            self.round_rect(0, 237, 240, 20, 0, t.bg);
            self.text_fitted_raw(12, 241, v.message, 1, 216, t.on_surface_variant);
        }
        if v.total > 0 {
            self.text_center_raw(
                264,
                &alloc::format!(
                    "{} / {}",
                    if v.total == 0 { 0 } else { v.cursor + 1 },
                    v.total
                ),
                1,
                t.on_surface_variant,
            );
        }
    }
}
