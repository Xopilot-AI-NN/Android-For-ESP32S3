// drivers/ssd1306.rs
// I2C-бэкенд для SSD1306: инициализация, flush, предготовые экраны.
// Вся геометрия/текст — в crate::graphic::Framebuffer, этот файл только гоняет байты по I2C.

use esp_hal::{
    Blocking,
    i2c::master::I2c,
};

// src/bin/drivers/ssd1306.rs
use crate::drivers::graphic::Framebuffer; // Было: `crate::graphic` 

const SSD1306_ADDR: u8 = 0x3C;
const WIDTH: usize = 128;
const HEIGHT: usize = 64;
const PAGES: usize = HEIGHT / 8;
const FB_BYTES: usize = WIDTH * PAGES; // 1024 байта

pub struct Ssd1306Display<'d> {
    i2c: I2c<'d, Blocking>,
    fb: Framebuffer<WIDTH, HEIGHT, FB_BYTES>,
}

impl<'d> Ssd1306Display<'d> {
    pub fn new(mut i2c: I2c<'d, Blocking>) -> Result<Self, ()> {
        let init_sequence = [
            0xAE, 0x20, 0x00, 0x40, 0xA1, 0xC8, 0x81, 0x7F, 0xA6, 0xA8, 0x3F, 0xD3, 0x00, 0xD5,
            0x80, 0xD9, 0xF1, 0xDA, 0x12, 0xDB, 0x40, 0x8D, 0x14, 0xAF,
        ];

        for command in init_sequence {
            write_command(&mut i2c, command)?;
        }

        let mut display = Self {
            i2c,
            fb: Framebuffer::new(),
        };
        display.clear()?;
        Ok(display)
    }

    pub fn clear(&mut self) -> Result<(), ()> {
        self.fb.clear();
        self.flush()
    }

    pub fn show_lines(&mut self, lines: [&str; 4]) -> Result<(), ()> {
        self.fb.clear();
        for (row, line) in lines.iter().enumerate() {
            self.fb.draw_text(0, (row as i32) * 16, line, 2, true);
        }
        self.flush()
    }

    pub fn show_fastboot_logo(&mut self) -> Result<(), ()> {
        self.fb.clear();

        let title = "FASTBOOT";
        let title_scale = 2;
        let title_width = Framebuffer::<WIDTH, HEIGHT, FB_BYTES>::text_width(title, title_scale, title_scale);
        let title_x = ((WIDTH as i32 - title_width).max(0)) / 2;
        let title_y = 18;

        // лёгкая "тень" для объёма + основной текст
        self.fb.draw_text(title_x + 2, title_y + 2, title, title_scale, false);
        self.fb.draw_text(title_x, title_y, title, title_scale, true);

        let subtitle = "Zephyr";
        let subtitle_scale = 1;
        let subtitle_spacing = 0;
        let subtitle_width = Framebuffer::<WIDTH, HEIGHT, FB_BYTES>::text_width(subtitle, subtitle_scale, subtitle_spacing);
        let subtitle_x = ((WIDTH as i32 - subtitle_width).max(0)) / 2;
        let subtitle_y = 54;

        self.fb.draw_text_spaced(subtitle_x, subtitle_y, subtitle, subtitle_scale, subtitle_spacing, true);

        self.flush()
    }

    /// Доступ к сырому framebuffer для кастомных экранов (бут-UI, иконки и т.д.).
    pub fn framebuffer(&mut self) -> &mut Framebuffer<WIDTH, HEIGHT, FB_BYTES> {
        &mut self.fb
    }

    /// Отрисовать текущее состояние framebuffer на экран (после ручного рисования через framebuffer()).
    pub fn present(&mut self) -> Result<(), ()> {
        self.flush()
    }

    fn flush(&mut self) -> Result<(), ()> {
        for page in 0..PAGES {
            self.set_cursor(page as u8, 0)?;

            for chunk in self.fb.page_bytes(page).chunks(16) {
                let mut packet = [0u8; 17];
                packet[0] = 0x40;
                packet[1..1 + chunk.len()].copy_from_slice(chunk);
                self.i2c
                    .write(SSD1306_ADDR, &packet[..1 + chunk.len()])
                    .map_err(|_| ())?;
            }
        }

        Ok(())
    }

    fn set_cursor(&mut self, page: u8, column: u8) -> Result<(), ()> {
        write_command(&mut self.i2c, 0xB0 | (page & 0x07))?;
        write_command(&mut self.i2c, column & 0x0F)?;
        write_command(&mut self.i2c, 0x10 | ((column >> 4) & 0x0F))?;
        Ok(())
    }
}

fn write_command(i2c: &mut I2c<'_, Blocking>, command: u8) -> Result<(), ()> {
    i2c.write(SSD1306_ADDR, &[0x00, command]).map_err(|_| ())
}