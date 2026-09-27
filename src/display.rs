#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayError { Bus }

use crate::runtime::RuntimeFrame;

pub trait BootDisplay {
    fn boot_logo(&mut self) -> Result<(), DisplayError>;
    fn boot_progress(&mut self, label: &str, phase: u8) -> Result<(), DisplayError>;
    fn status(&mut self, title: &str, l1: &str, l2: &str, l3: &str) -> Result<(), DisplayError>;
    fn runtime(&mut self, frame: &RuntimeFrame) -> Result<(), DisplayError> {
        match frame {
            RuntimeFrame::Legacy(f) => self.status(&f.title, &f.line1, &f.line2, &f.line3),
            RuntimeFrame::Material(f) => {
                let (title, l1, l2, l3) = f.fallback();
                self.status(title, l1, l2, l3)
            }
        }
    }
    fn fastboot(&mut self) -> Result<(), DisplayError> {
        self.status("FASTBOOT MODE", "USB: READY", "PWR: REBOOT", "CMD: HELP")
    }
    fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError>;
}

// ---------------------------------------------------------------------------
// SSD1306 128x64 monochrome backend (GPIO5 SDA, GPIO6 SCL)
// ---------------------------------------------------------------------------
#[cfg(feature = "display-ssd1306")]
mod oled {
    use embedded_hal::i2c::I2c;
    use super::{BootDisplay, DisplayError};
    use crate::{config, framebuffer::Framebuffer};
    pub type OledFramebuffer = Framebuffer<128, 64, 1024>;

    pub struct Ssd1306Display<I2C> {
        i2c: I2C,
        fb: OledFramebuffer,
    }

    impl<I2C> Ssd1306Display<I2C>
    where I2C: I2c,
    {
        pub fn new(mut i2c: I2C) -> Result<Self, DisplayError> {
            let init: [u8; 26] = [
                0x00, 0xAE, 0xD5, 0x80, 0xA8, 0x3F, 0xD3, 0x00, 0x40,
                0x8D, 0x14, 0x20, 0x00, 0xA1, 0xC8, 0xDA, 0x12, 0x81,
                0x8F, 0xD9, 0xF1, 0xDB, 0x40, 0xA4, 0xA6, 0xAF,
            ];
            i2c.write(config::OLED_ADDR, &init).map_err(|_| DisplayError::Bus)?;
            Ok(Self { i2c, fb: OledFramebuffer::new() })
        }

        fn flush(&mut self) -> Result<(), DisplayError> {
            self.command(&[0x21, 0, 127, 0x22, 0, 7])?;
            for page in 0..8 {
                let mut packet = [0u8; 129];
                packet[0] = 0x40;
                let begin = page * 128;
                packet[1..].copy_from_slice(&self.fb.as_bytes()[begin..begin + 128]);
                self.i2c.write(config::OLED_ADDR, &packet).map_err(|_| DisplayError::Bus)?;
            }
            Ok(())
        }

        fn command(&mut self, bytes: &[u8]) -> Result<(), DisplayError> {
            let mut packet = [0u8; 16];
            if bytes.len() + 1 > packet.len() { return Err(DisplayError::Bus); }
            packet[0] = 0x00;
            packet[1..1 + bytes.len()].copy_from_slice(bytes);
            self.i2c.write(config::OLED_ADDR, &packet[..1 + bytes.len()]).map_err(|_| DisplayError::Bus)
        }
    }

    impl<I2C> BootDisplay for Ssd1306Display<I2C>
    where I2C: I2c,
    {
        fn boot_logo(&mut self) -> Result<(), DisplayError> {
            self.fb.clear();
            self.fb.draw_zephyr_mark();
            self.fb.draw_text_centered(49, "ANDROID", 1, true);
            self.flush()
        }

        fn boot_progress(&mut self, label: &str, phase: u8) -> Result<(), DisplayError> {
            self.fb.clear();
            self.fb.draw_zephyr_mark();
            self.fb.draw_text_centered(45, label, 1, true);
            self.fb.draw_spinner(phase);
            self.flush()
        }

        fn status(&mut self, title: &str, l1: &str, l2: &str, l3: &str) -> Result<(), DisplayError> {
            self.fb.clear();
            self.fb.draw_text_centered(2, title, 1, true);
            self.fb.draw_hline(5, 12, 118, true);
            self.fb.draw_text(4, 20, l1, 1, true);
            self.fb.draw_text(4, 32, l2, 1, true);
            self.fb.draw_text(4, 44, l3, 1, true);
            self.flush()
        }

        fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError> {
            self.fb.clear();
            self.fb.draw_rect(2, 2, 124, 60, true);
            self.fb.draw_text_centered(8, "BOOT FAILED", 1, true);
            self.fb.draw_text_centered(25, code, 1, true);
            self.fb.draw_text_centered(42, detail, 1, true);
            self.flush()
        }
    }

    pub use Ssd1306Display as Display;
}

#[cfg(feature = "display-ssd1306")]
pub use oled::Display as Ssd1306Display;

// ---------------------------------------------------------------------------
// ST7789V3 240x280 RGB565 backend (GPIO1..6). No framebuffer allocation:
// boot UI is rendered directly so it fits comfortably on ESP32-S3 internal RAM.
// ---------------------------------------------------------------------------
#[cfg(feature = "display-st7789")]
mod tft {
    use embedded_hal::{delay::DelayNs, digital::OutputPin, spi::SpiBus};
    use super::{BootDisplay, DisplayError};
    use crate::{config, font::glyph, material::{Rect, Surface}, runtime::RuntimeFrame};

    const W: i32 = 240;
    const H: i32 = 280;
    const BLACK: u16 = 0x0000;
    const WHITE: u16 = 0xFFFF;
    const BLUE: u16 = 0x249F;
    const GREEN: u16 = 0x3E08;
    const YELLOW: u16 = 0xFEC0;
    const RED: u16 = 0xF986;
    const DIM: u16 = 0x7BEF;

    pub struct St7789Display<SPI, CS, DC, RST, BL> {
        spi: SPI,
        cs: CS,
        dc: DC,
        _rst: RST,
        bl: BL,
        material_surface: Surface,
    }

    impl<SPI, CS, DC, RST, BL> St7789Display<SPI, CS, DC, RST, BL>
    where
        SPI: SpiBus<u8>,
        CS: OutputPin,
        DC: OutputPin,
        RST: OutputPin,
        BL: OutputPin,
    {
        pub fn new<D: DelayNs>(spi: SPI, mut cs: CS, mut dc: DC, mut rst: RST, mut bl: BL, delay: &mut D) -> Result<Self, DisplayError> {
            cs.set_high().map_err(|_| DisplayError::Bus)?;
            dc.set_low().map_err(|_| DisplayError::Bus)?;
            bl.set_low().map_err(|_| DisplayError::Bus)?;
            rst.set_low().map_err(|_| DisplayError::Bus)?;
            delay.delay_ms(20);
            rst.set_high().map_err(|_| DisplayError::Bus)?;
            delay.delay_ms(120);
            let mut s = Self { spi, cs, dc, _rst: rst, bl, material_surface: Surface::new() };
            s.cmd(0x01, &[])?; // SWRESET
            delay.delay_ms(120);
            s.cmd(0x11, &[])?; // SLPOUT
            delay.delay_ms(120);
            s.cmd(0x13, &[])?; // NORON
            s.cmd(0x3A, &[0x55])?; // RGB565
            s.cmd(0x36, &[config::TFT_MADCTL])?; // MADCTL
            s.cmd(0xB2, &[0x0C, 0x0C, 0x00, 0x33, 0x33])?;
            s.cmd(0xB7, &[0x35])?;
            s.cmd(0xBB, &[0x28])?;
            s.cmd(0xC0, &[0x0C])?;
            s.cmd(0xC2, &[0x01, 0xFF])?;
            s.cmd(0xC3, &[0x10])?;
            s.cmd(0xC4, &[0x20])?;
            s.cmd(0xC6, &[0x0F])?;
            s.cmd(0xD0, &[0xA4, 0xA1])?;
            s.cmd(0xE0, &[0xD0,0x00,0x02,0x07,0x0A,0x28,0x32,0x44,0x42,0x06,0x0E,0x12,0x14,0x17])?;
            s.cmd(0xE1, &[0xD0,0x00,0x02,0x07,0x0A,0x28,0x31,0x54,0x47,0x0E,0x1C,0x17,0x1B,0x1E])?;
            s.cmd(0x21, &[])?; // INVON (IPS panels commonly require inversion)
            s.cmd(0x29, &[])?; // DISPON
            delay.delay_ms(120);
            s.bl.set_high().map_err(|_| DisplayError::Bus)?;
            s.clear(BLACK)?;
            Ok(s)
        }

        fn cmd(&mut self, cmd: u8, data: &[u8]) -> Result<(), DisplayError> {
            self.cs.set_low().map_err(|_| DisplayError::Bus)?;
            self.dc.set_low().map_err(|_| DisplayError::Bus)?;
            self.spi.write(&[cmd]).map_err(|_| DisplayError::Bus)?;
            if !data.is_empty() {
                self.dc.set_high().map_err(|_| DisplayError::Bus)?;
                self.spi.write(data).map_err(|_| DisplayError::Bus)?;
            }
            self.cs.set_high().map_err(|_| DisplayError::Bus)
        }

        fn set_window(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) -> Result<(), DisplayError> {
            let x0 = x0.clamp(0, W - 1) as u16;
            let y0 = y0.clamp(0, H - 1) as u16 + config::TFT_Y_OFFSET;
            let x1 = x1.clamp(0, W - 1) as u16;
            let y1 = y1.clamp(0, H - 1) as u16 + config::TFT_Y_OFFSET;
            self.cmd(0x2A, &[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8])?;
            self.cmd(0x2B, &[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])?;
            self.cs.set_low().map_err(|_| DisplayError::Bus)?;
            self.dc.set_low().map_err(|_| DisplayError::Bus)?;
            self.spi.write(&[0x2C]).map_err(|_| DisplayError::Bus)?;
            self.dc.set_high().map_err(|_| DisplayError::Bus)
        }

        fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u16) -> Result<(), DisplayError> {
            if w <= 0 || h <= 0 || x >= W || y >= H || x + w <= 0 || y + h <= 0 { return Ok(()); }
            let x0 = x.max(0);
            let y0 = y.max(0);
            let x1 = (x + w - 1).min(W - 1);
            let y1 = (y + h - 1).min(H - 1);
            self.set_window(x0, y0, x1, y1)?;
            let count = ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize;
            let px = [(color >> 8) as u8, color as u8];
            let mut buf = [0u8; 128];
            for p in buf.chunks_exact_mut(2) { p.copy_from_slice(&px); }
            let mut left = count;
            while left != 0 {
                let pixels = left.min(buf.len() / 2);
                self.spi.write(&buf[..pixels * 2]).map_err(|_| DisplayError::Bus)?;
                left -= pixels;
            }
            self.cs.set_high().map_err(|_| DisplayError::Bus)
        }

        fn clear(&mut self, color: u16) -> Result<(), DisplayError> { self.fill_rect(0, 0, W, H, color) }

        fn flush_material_rect(&mut self, rect: Rect) -> Result<(), DisplayError> {
            let x0 = rect.x.max(0);
            let y0 = rect.y.max(0);
            let x1 = rect.x1().min(W - 1);
            let y1 = rect.y1().min(H - 1);
            if x1 < x0 || y1 < y0 { return Ok(()); }
            self.set_window(x0, y0, x1, y1)?;
            let pixels = self.material_surface.pixels();
            let mut buf = [0u8; 256];
            for y in y0..=y1 {
                let row = &pixels[y as usize * W as usize + x0 as usize .. y as usize * W as usize + x1 as usize + 1];
                let mut pos = 0;
                while pos < row.len() {
                    let count = (row.len() - pos).min(buf.len() / 2);
                    for (i, &px) in row[pos..pos + count].iter().enumerate() {
                        buf[i * 2] = (px >> 8) as u8;
                        buf[i * 2 + 1] = px as u8;
                    }
                    self.spi.write(&buf[..count * 2]).map_err(|_| DisplayError::Bus)?;
                    pos += count;
                }
            }
            self.cs.set_high().map_err(|_| DisplayError::Bus)
        }

        fn glyph(&mut self, x: i32, y: i32, ch: u8, scale: i32, color: u16) -> Result<(), DisplayError> {
            let g = glyph(ch.to_ascii_uppercase());
            for (col, bits) in g.iter().enumerate() {
                for row in 0..7 {
                    if (bits >> row) & 1 != 0 {
                        self.fill_rect(x + col as i32 * scale, y + row * scale, scale, scale, color)?;
                    }
                }
            }
            Ok(())
        }

        fn text_width(text: &str, scale: i32) -> i32 { if text.is_empty() { 0 } else { (text.len() as i32 * 6 - 1) * scale } }

        fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, color: u16) -> Result<(), DisplayError> {
            let mut cx = x;
            for c in text.bytes() { self.glyph(cx, y, c, scale, color)?; cx += 6 * scale; }
            Ok(())
        }

        fn text_center(&mut self, y: i32, text: &str, scale: i32, color: u16) -> Result<(), DisplayError> {
            let x = (W - Self::text_width(text, scale)) / 2;
            self.text(x, y, text, scale, color)
        }

        fn mark(&mut self) -> Result<(), DisplayError> {
            // Four-color Zephyr mark inspired by the clean Pixel Watch boot composition,
            // deliberately not a copy of Google's G artwork.
            self.fill_rect(91, 62, 29, 10, BLUE)?;
            self.fill_rect(120, 62, 29, 10, RED)?;
            self.fill_rect(91, 72, 29, 10, GREEN)?;
            self.fill_rect(120, 72, 29, 10, YELLOW)?;
            self.text_center(91, "Z", 6, WHITE)
        }

        fn spinner(&mut self, phase: u8) -> Result<(), DisplayError> {
            const P: [(i32, i32); 8] = [(120,214),(136,208),(142,192),(136,176),(120,170),(104,176),(98,192),(104,208)];
            for (i, &(x, y)) in P.iter().enumerate() {
                let c = if i == phase as usize % 8 { WHITE } else { DIM };
                self.fill_rect(x - 3, y - 3, 7, 7, c)?;
            }
            Ok(())
        }
    }

    impl<SPI, CS, DC, RST, BL> BootDisplay for St7789Display<SPI, CS, DC, RST, BL>
    where SPI: SpiBus<u8>, CS: OutputPin, DC: OutputPin, RST: OutputPin, BL: OutputPin,
    {
        fn boot_logo(&mut self) -> Result<(), DisplayError> {
            self.clear(BLACK)?;
            self.mark()?;
            self.text_center(155, "ANDROID", 3, WHITE)
        }
        fn boot_progress(&mut self, label: &str, phase: u8) -> Result<(), DisplayError> {
            self.clear(BLACK)?;
            self.mark()?;
            self.text_center(145, label, 2, WHITE)?;
            self.spinner(phase)
        }
        fn status(&mut self, title: &str, l1: &str, l2: &str, l3: &str) -> Result<(), DisplayError> {
            self.clear(BLACK)?;
            self.text_center(35, title, 3, WHITE)?;
            self.fill_rect(25, 72, 190, 2, DIM)?;
            self.text(24, 104, l1, 2, WHITE)?;
            self.text(24, 142, l2, 2, WHITE)?;
            self.text(24, 180, l3, 2, WHITE)
        }
        fn runtime(&mut self, frame: &RuntimeFrame) -> Result<(), DisplayError> {
            match frame {
                RuntimeFrame::Material(scene) => {
                    let dirty = self.material_surface.render(*scene);
                    self.flush_material_rect(dirty)
                }
                RuntimeFrame::Legacy(f) => self.status(&f.title, &f.line1, &f.line2, &f.line3),
            }
        }
        fn error(&mut self, code: &str, detail: &str) -> Result<(), DisplayError> {
            self.clear(BLACK)?;
            self.text_center(55, "BOOT FAILED", 3, RED)?;
            self.text_center(125, code, 2, WHITE)?;
            self.text_center(165, detail, 2, DIM)
        }
    }

    pub use St7789Display as Display;
}

#[cfg(feature = "display-st7789")]
pub use tft::Display as St7789Display;
