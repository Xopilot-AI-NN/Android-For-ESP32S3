use super::*;
impl Surface {
    /// Показ только реального preview; во время I/O предыдущая картинка не выдаётся за новую.
    pub(super) fn gallery_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.gallery;
        self.photo_header(f, "Галерея", t);
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
            self.symbol(Symbol::Folder, 120, 115, t.primary);
            self.text_center_raw(155, "Изображений пока нет", 1, t.on_surface_variant);
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
            self.round_rect(0, 230, 240, 26, 0, t.bg);
            self.text_center_raw(237, v.message, 1, t.on_surface_variant);
        }
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
