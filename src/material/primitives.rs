//! primitives Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn progress_ring(
        &mut self,
        cx: i32,
        cy: i32,
        radius: i32,
        percent: u8,
        active: u16,
        inactive: u16,
    ) {
        const POINTS: [(i32, i32); 37] = [
            (0, -1000),
            (174, -985),
            (342, -940),
            (500, -866),
            (643, -766),
            (766, -643),
            (866, -500),
            (940, -342),
            (985, -174),
            (1000, 0),
            (985, 174),
            (940, 342),
            (866, 500),
            (766, 643),
            (643, 766),
            (500, 866),
            (342, 940),
            (174, 985),
            (0, 1000),
            (-174, 985),
            (-342, 940),
            (-500, 866),
            (-643, 766),
            (-766, 643),
            (-866, 500),
            (-940, 342),
            (-985, 174),
            (-1000, 0),
            (-985, -174),
            (-940, -342),
            (-866, -500),
            (-766, -643),
            (-643, -766),
            (-500, -866),
            (-342, -940),
            (-174, -985),
            (0, -1000),
        ];
        for i in 0..36 {
            let (x0, y0) = POINTS[i];
            let (x1, y1) = POINTS[i + 1];
            self.line(
                cx + x0 * radius / 1000,
                cy + y0 * radius / 1000,
                cx + x1 * radius / 1000,
                cy + y1 * radius / 1000,
                3,
                if i * 100 < usize::from(percent.min(100)) * 36 {
                    active
                } else {
                    inactive
                },
            );
        }
    }

    pub(super) fn scallop(&mut self, cx: i32, cy: i32, fill: u16, outline: u16) {
        for (shrink, color) in [(0, outline), (3, fill)] {
            self.circle(cx, cy, 34 - shrink, color);
            for (x, y) in [
                (0, -31),
                (18, -25),
                (30, -10),
                (30, 10),
                (18, 25),
                (0, 31),
                (-18, 25),
                (-30, 10),
                (-30, -10),
                (-18, -25),
            ] {
                self.circle(
                    cx + x * (100 - shrink * 3) / 100,
                    cy + y * (100 - shrink * 3) / 100,
                    12 - shrink,
                    color,
                );
            }
        }
    }
    pub(super) fn coverage_pixel(&mut self, x: i32, y: i32, color: u16, alpha: u8) {
        if alpha == 0 || x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return;
        }
        let alpha = u32::from(alpha.min(15));
        let bg = u32::from(self.pixels[y as usize * WIDTH + x as usize]);
        let blend = |shift: u32, mask: u32| -> u16 {
            (((((u32::from(color) >> shift) & mask) * alpha
                + ((bg >> shift) & mask) * (15 - alpha)
                + 7)
                / 15)
                << shift) as u16
        };
        self.pixel(x, y, blend(11, 31) | blend(5, 63) | blend(0, 31));
    }
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
                let alpha = Self::circle_coverage(ox, oy, r);
                self.coverage_pixel(x + dx, y + dy, c, alpha);
                self.coverage_pixel(x + w - 1 - dx, y + dy, c, alpha);
                self.coverage_pixel(x + dx, y + h - 1 - dy, c, alpha);
                self.coverage_pixel(x + w - 1 - dx, y + h - 1 - dy, c, alpha);
            }
        }
    }

    fn circle_coverage(x: i32, y: i32, r: i32) -> u8 {
        if r <= 0 {
            return if x == 0 && y == 0 { 15 } else { 0 };
        }
        let d = x * x + y * y;
        if d <= (r - 1) * (r - 1) {
            return 15;
        }
        if d >= (r + 1) * (r + 1) {
            return 0;
        }
        let mut inside = 0;
        for dx in [-3, -1, 1, 3] {
            for dy in [-3, -1, 1, 3] {
                let sx = x * 8 + dx;
                let sy = y * 8 + dy;
                if sx * sx + sy * sy <= r * r * 64 {
                    inside += 1;
                }
            }
        }
        ((inside * 15 + 8) / 16) as u8
    }
    pub(super) fn circle(&mut self, cx: i32, cy: i32, r: i32, c: u16) {
        for y in -r..=r {
            for x in -r..=r {
                let a = Self::circle_coverage(x, y, r);
                self.coverage_pixel(cx + x, cy + y, c, a);
            }
        }
    }
    pub(super) fn ring(&mut self, cx: i32, cy: i32, r: i32, thickness: i32, c: u16) {
        let inner = (r - thickness).max(0);
        for y in -r..=r {
            for x in -r..=r {
                let outer = Self::circle_coverage(x, y, r);
                let hole = if inner > 0 {
                    Self::circle_coverage(x, y, inner)
                } else {
                    0
                };
                self.coverage_pixel(cx + x, cy + y, c, outer.saturating_sub(hole));
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
