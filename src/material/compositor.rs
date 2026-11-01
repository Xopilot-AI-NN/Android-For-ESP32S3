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
        for p in &mut self.pixels {
            *p = mix(*p, 0, u32::from(percent) * 255 / 100);
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
