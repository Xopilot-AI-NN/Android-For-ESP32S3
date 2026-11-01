//! primitives Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn pixel(&mut self, x: i32, y: i32, c: u16) {
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            self.pixels[y as usize * WIDTH + x as usize] = c;
        }
    }

    pub(super) fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u16) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w).min(WIDTH as i32).max(0) as usize;
        let y1 = (y + h).min(HEIGHT as i32).max(0) as usize;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for yy in y0..y1 {
            self.pixels[yy * WIDTH + x0..yy * WIDTH + x1].fill(c);
        }
    }

    pub(super) fn round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: u16) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = r.max(0).min(w / 2).min(h / 2);
        self.fill_rect(x + r, y, w - 2 * r, h, c);
        self.fill_rect(x, y + r, w, h - 2 * r, c);
        for dy in 0..r {
            for dx in 0..r {
                let ox = r - dx;
                let oy = r - dy;
                if ox * ox + oy * oy <= r * r {
                    self.pixel(x + dx, y + dy, c);
                    self.pixel(x + w - 1 - dx, y + dy, c);
                    self.pixel(x + dx, y + h - 1 - dy, c);
                    self.pixel(x + w - 1 - dx, y + h - 1 - dy, c);
                }
            }
        }
    }

    pub(super) fn circle(&mut self, cx: i32, cy: i32, r: i32, c: u16) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r {
                    self.pixel(cx + x, cy + y, c);
                }
            }
        }
    }

    pub(super) fn ring(&mut self, cx: i32, cy: i32, r: i32, thickness: i32, c: u16) {
        let inner = (r - thickness).max(0);
        for y in -r..=r {
            for x in -r..=r {
                let d = x * x + y * y;
                if d <= r * r && d >= inner * inner {
                    self.pixel(cx + x, cy + y, c);
                }
            }
        }
    }

    pub(super) fn line(
        &mut self,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        thickness: i32,
        c: u16,
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.circle(x0, y0, thickness.max(1) / 2, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}
