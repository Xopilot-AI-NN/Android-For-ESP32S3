// Модуль смешивания кадров и области обновления.
//! Pixel composition and damage tracking; unchanged frames never reach SPI.
use super::*;
/// Смешивает два цвета RGB565 с заданной прозрачностью.
fn mix(a: u16, b: u16, alpha: u32) -> u16 {
    let channel = |shift: u32, mask: u32| {
        ((((u32::from(a) >> shift & mask) * alpha
            + (u32::from(b) >> shift & mask) * (255 - alpha)
            + 127)
            / 255)
            << shift) as u16
    };
    channel(11, 31) | channel(5, 63) | channel(0, 31)
}
impl Surface {
    /// Масштабирует яркость готового кадра через таблицы каналов.
    pub fn scale_pixels(&mut self, percent: u8) {
        if percent >= 100 {
            return;
        }
        let alpha = u32::from(percent) * 255 / 100;
        let mut five = [0u16; 32];
        let mut six = [0u16; 64];
        for (i, x) in five.iter_mut().enumerate() {
            *x = ((i as u32 * alpha + 127) / 255) as u16;
        }
        for (i, x) in six.iter_mut().enumerate() {
            *x = ((i as u32 * alpha + 127) / 255) as u16;
        }
        for p in &mut self.pixels {
            *p = (five[(*p >> 11) as usize] << 11)
                | (six[((*p >> 5) & 63) as usize] << 5)
                | five[(*p & 31) as usize];
        }
    }
    /// Смешивает новый кадр с предыдущим при переходе.
    pub fn blend_from(&mut self, previous: &[u16], alpha: u8) {
        if alpha == 255 || previous.len() != self.pixels.len() {
            return;
        }
        for (p, old) in self.pixels.iter_mut().zip(previous) {
            *p = mix(*p, *old, u32::from(alpha));
        }
    }
    /// Находит минимальный прямоугольник отличий от прошлого кадра.
    #[cfg(test)]
    pub fn changes_from(&self, previous: &[u16]) -> Option<Rect> {
        if previous.len() != self.pixels.len() {
            return Some(Rect::FULL);
        }
        bounds(&self.pixels, previous, 0)
    }
    /// Bounded damage bands avoid transmitting unchanged pixels between distant edits.
    /// Adjacent/dense damage falls back to a single rectangle when that is cheaper.
    pub fn damage_regions(&self, previous: &[u16]) -> heapless::Vec<Rect, 18> {
        let mut regions = heapless::Vec::new();
        if previous.len() != self.pixels.len() {
            let _ = regions.push(Rect::FULL);
            return regions;
        }
        let mut area = 0;
        let mut overall: Option<Rect> = None;
        for (band, (current, old)) in self
            .pixels
            .chunks(WIDTH * 16)
            .zip(previous.chunks(WIDTH * 16))
            .enumerate()
        {
            if let Some(rect) = bounds(current, old, band * 16) {
                area += rect.w * rect.h;
                overall = Some(match overall {
                    None => rect,
                    Some(a) => Rect {
                        x: a.x.min(rect.x),
                        y: a.y.min(rect.y),
                        w: a.x1().max(rect.x1()) - a.x.min(rect.x) + 1,
                        h: a.y1().max(rect.y1()) - a.y.min(rect.y) + 1,
                    },
                });
                // 280 rows / 16 rounded up = 18; capacity is exact and bounded.
                regions.push(rect).unwrap();
            }
        }
        if let Some(rect) = overall {
            // Allow for command/flush overhead per SPI window.
            if area + (regions.len() as i32 - 1) * 64 >= rect.w * rect.h {
                regions.clear();
                regions.push(rect).unwrap();
            }
        }
        regions
    }
}
/// Row-wise comparison avoids division/modulo for every framebuffer pixel.
fn bounds(current: &[u16], previous: &[u16], y_offset: usize) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (WIDTH, HEIGHT, 0, 0);
    for (row, (pixels, old)) in current
        .chunks(WIDTH)
        .zip(previous.chunks(WIDTH))
        .enumerate()
    {
        if pixels == old {
            continue;
        }
        let first = pixels.iter().zip(old).position(|(a, b)| a != b).unwrap();
        let last = pixels.iter().zip(old).rposition(|(a, b)| a != b).unwrap();
        x0 = x0.min(first);
        x1 = x1.max(last);
        y0 = y0.min(row + y_offset);
        y1 = row + y_offset;
    }
    (x0 != WIDTH).then_some(Rect {
        x: x0 as i32,
        y: y0 as i32,
        w: (x1 as i32 - x0 as i32 + 1),
        h: (y1 as i32 - y0 as i32 + 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wallpaper_shares_pixels_and_replacement_drops_the_old_reference() {
        let mut frame = MaterialFrame::parse("M3|1|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3").unwrap();
        frame.wallpaper_enabled = true;
        frame.wallpaper_index = 0;
        let mut surface = Surface::new();
        surface.set_wallpaper(vec![0xffff; WIDTH * HEIGHT]);
        surface.render(frame);
        assert!(Rc::ptr_eq(
            surface.wallpaper.as_ref().unwrap(),
            surface.custom_wallpaper.as_ref().unwrap()
        ));
        let old = Rc::downgrade(surface.wallpaper.as_ref().unwrap());
        surface.set_wallpaper(vec![0; WIDTH * HEIGHT]);
        surface.render(frame);
        assert!(old.upgrade().is_none());
    }
    #[test]
    fn separated_damage_is_small_and_reconstructs_the_exact_frame() {
        let mut surface = Surface::new();
        let mut previous = surface.pixels.clone();
        surface.pixels[0] = 1;
        surface.pixels[(HEIGHT - 1) * WIDTH + WIDTH - 1] = 2;
        let regions = surface.damage_regions(&previous);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions.iter().map(|r| r.w * r.h).sum::<i32>(), 2);
        for rect in regions {
            for y in rect.y..rect.y + rect.h {
                let start = y as usize * WIDTH + rect.x as usize;
                let end = start + rect.w as usize;
                previous[start..end].copy_from_slice(&surface.pixels[start..end]);
            }
        }
        assert_eq!(previous, surface.pixels);
        assert!(surface.damage_regions(&previous).is_empty());
        assert_eq!(surface.damage_regions(&[]).as_slice(), &[Rect::FULL]);
    }
    #[test]
    fn dense_damage_falls_back_to_one_window_and_sparse_bands_are_bounded() {
        let mut surface = Surface::new();
        let old = surface.pixels.clone();
        surface.pixels.fill(1);
        assert_eq!(surface.damage_regions(&old).as_slice(), &[Rect::FULL]);
        surface.pixels.fill(0);
        for band in 0..18 {
            surface.pixels[(band * 16).min(HEIGHT - 1) * WIDTH + band * 3] = 1;
        }
        let regions = surface.damage_regions(&old);
        assert!(regions.len() <= 18);
        for i in 0..surface.pixels.len() {
            if surface.pixels[i] != old[i] {
                let (x, y) = ((i % WIDTH) as i32, (i / WIDTH) as i32);
                assert!(
                    regions
                        .iter()
                        .any(|r| x >= r.x && x <= r.x1() && y >= r.y && y <= r.y1())
                );
            }
        }
    }
    #[test]
    /// Проверяет просвечивание обоев и единственное наложение строки состояния за кадр.
    fn status_scrim_preserves_wallpaper_and_is_applied_once() {
        let mut frame = MaterialFrame::parse("M3|1|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3").unwrap();
        frame.wallpaper_enabled = true;
        frame.wallpaper_index = 0;
        frame.wallpaper_level = 100;
        let mut s = Surface::new();
        s.set_wallpaper(vec![0xffff; WIDTH * HEIGHT]);
        s.render(frame);
        // A white background remains visible through 8/15 black opacity.
        let expected = (14 << 11) | (29 << 5) | 14;
        assert_eq!(s.pixels()[2 * WIDTH + 100], expected);
        s.render(frame);
        assert_eq!(s.pixels()[2 * WIDTH + 100], expected);
    }
    #[test]
    /// Сверяет табличное масштабирование яркости для всех цветов RGB565.
    fn brightness_lookup_preserves_all_rgb565_colors() {
        for percent in [0u8, 5, 15, 70, 99, 100] {
            let mut surface = Surface::new();
            for (i, p) in surface.pixels.iter_mut().enumerate() {
                *p = i as u16;
            }
            surface.scale_pixels(percent);
            for i in 0..65536usize {
                assert_eq!(
                    surface.pixels[i],
                    mix(i as u16, 0, u32::from(percent) * 255 / 100)
                );
            }
        }
    }
    #[test]
    /// Проверяет границы изменившегося прямоугольника и корректное затемнение пикселей.
    fn damage_and_brightness_are_bounded() {
        let mut s = Surface::new();
        let blank = s.pixels.clone();
        assert_eq!(s.changes_from(&blank), None);
        s.pixels[10 * WIDTH + 7] = 0xffff;
        assert_eq!(
            s.changes_from(&blank),
            Some(Rect {
                x: 7,
                y: 10,
                w: 1,
                h: 1
            })
        );
        s.scale_pixels(50);
        assert_ne!(s.pixels[10 * WIDTH + 7], 0xffff);
        assert_ne!(s.pixels[10 * WIDTH + 7], 0);
    }
}
