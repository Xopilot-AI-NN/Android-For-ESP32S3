//! Pixel composition and damage tracking; unchanged frames never reach SPI.
use super::*;
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
    pub fn blend_from(&mut self, previous: &[u16], alpha: u8) {
        if alpha == 255 || previous.len() != self.pixels.len() {
            return;
        }
        for (p, old) in self.pixels.iter_mut().zip(previous) {
            *p = mix(*p, *old, u32::from(alpha));
        }
    }
    pub fn changes_from(&self, previous: &[u16]) -> Option<Rect> {
        if previous.len() != self.pixels.len() {
            return Some(Rect::FULL);
        }
        let (mut x0, mut y0, mut x1, mut y1) = (WIDTH, HEIGHT, 0, 0);
        for (i, (p, old)) in self.pixels.iter().zip(previous).enumerate() {
            if p != old {
                let x = i % WIDTH;
                let y = i / WIDTH;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        if x0 == WIDTH {
            None
        } else {
            Some(Rect {
                x: x0 as i32,
                y: y0 as i32,
                w: (x1 - x0 + 1) as i32,
                h: (y1 - y0 + 1) as i32,
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
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
