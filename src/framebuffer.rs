use crate::font::glyph;

pub struct Framebuffer<const W: usize, const H: usize, const BYTES: usize> {
    data: [u8; BYTES],
}

impl<const W: usize, const H: usize, const BYTES: usize> Framebuffer<W, H, BYTES> {
    pub const fn new() -> Self {
        assert!(H % 8 == 0);
        assert!(BYTES == W * (H / 8));
        Self { data: [0; BYTES] }
    }

    pub fn as_bytes(&self) -> &[u8] { &self.data }
    pub fn clear(&mut self) { self.data.fill(0); }

    pub fn set_pixel(&mut self, x: i32, y: i32, on: bool) {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 { return; }
        let x = x as usize;
        let y = y as usize;
        let idx = x + (y / 8) * W;
        let mask = 1u8 << (y % 8);
        if on { self.data[idx] |= mask; } else { self.data[idx] &= !mask; }
    }

    pub fn draw_hline(&mut self, x: i32, y: i32, w: i32, on: bool) {
        for dx in 0..w { self.set_pixel(x + dx, y, on); }
    }

    pub fn draw_vline(&mut self, x: i32, y: i32, h: i32, on: bool) {
        for dy in 0..h { self.set_pixel(x, y + dy, on); }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        if w < 1 || h < 1 { return; }
        self.draw_hline(x, y, w, on);
        self.draw_hline(x, y + h - 1, w, on);
        self.draw_vline(x, y, h, on);
        self.draw_vline(x + w - 1, y, h, on);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for dy in 0..h { self.draw_hline(x, y + dy, w, on); }
    }

    pub fn draw_circle(&mut self, cx: i32, cy: i32, r: i32, on: bool) {
        let mut x = r;
        let mut y = 0;
        let mut err = 0;
        while x >= y {
            for (px, py) in [
                (cx + x, cy + y), (cx + y, cy + x), (cx - y, cy + x), (cx - x, cy + y),
                (cx - x, cy - y), (cx - y, cy - x), (cx + y, cy - x), (cx + x, cy - y),
            ] { self.set_pixel(px, py, on); }
            y += 1;
            if err <= 0 { err += 2 * y + 1; }
            if err > 0 { x -= 1; err -= 2 * x + 1; }
        }
    }

    pub fn draw_glyph(&mut self, x: i32, y: i32, ch: u8, scale: i32, on: bool) {
        let scale = scale.max(1);
        let g = glyph(ch.to_ascii_uppercase());
        for (column, bits) in g.iter().enumerate() {
            for row in 0..7 {
                if (bits >> row) & 1 == 0 { continue; }
                self.fill_rect(x + column as i32 * scale, y + row * scale, scale, scale, on);
            }
        }
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, scale: i32, on: bool) {
        let mut cx = x;
        for ch in text.bytes() {
            self.draw_glyph(cx, y, ch, scale, on);
            cx += 6 * scale;
        }
    }

    pub fn text_width(text: &str, scale: i32) -> i32 {
        if text.is_empty() { 0 } else { (text.len() as i32 * 6 - 1) * scale }
    }

    pub fn draw_text_centered(&mut self, y: i32, text: &str, scale: i32, on: bool) {
        let x = (W as i32 - Self::text_width(text, scale)) / 2;
        self.draw_text(x, y, text, scale, on);
    }

    pub fn draw_zephyr_mark(&mut self) {
        // Minimal AOSP-style boot mark for the monochrome fallback panel.
        self.draw_circle(64, 25, 16, true);
        self.draw_text(58, 18, "A", 2, true);
    }

    pub fn draw_spinner(&mut self, phase: u8) {
        const P: [(i32, i32); 8] = [
            (64, 52), (69, 50), (71, 46), (69, 42),
            (64, 40), (59, 42), (57, 46), (59, 50),
        ];
        for (i, &(x, y)) in P.iter().enumerate() {
            let on = i == (phase as usize % P.len()) || i == ((phase as usize + 7) % P.len());
            if on { self.fill_rect(x - 1, y - 1, 3, 3, true); }
            else { self.set_pixel(x, y, true); }
        }
    }
}
