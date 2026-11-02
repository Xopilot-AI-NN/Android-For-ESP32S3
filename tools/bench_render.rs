//! CPU-only benchmark of the firmware renderer (no SPI/USB timing claims).
extern crate alloc;
#[allow(dead_code)]
#[path = "../src/material.rs"]
mod material;
use std::{hint::black_box, time::Instant};
fn main() {
    for (name, screen) in [("digital", 1), ("settings", 5), ("media", 10)] {
        let frame = material::MaterialFrame::parse(&format!(
            "M3|{screen}|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3"
        ))
        .unwrap();
        let mut surface = material::Surface::new();
        for _ in 0..5 {
            surface.render(frame);
        }
        let began = Instant::now();
        for _ in 0..200 {
            surface.render(black_box(frame));
            black_box(surface.pixels());
        }
        let us = began.elapsed().as_micros() / 200;
        let hash = surface
            .pixels()
            .iter()
            .fold(0xcbf29ce484222325u64, |h, &p| {
                (h ^ u64::from(p)).wrapping_mul(0x100000001b3)
            });
        println!("{name}: {us} us/frame; pixel_hash={hash:016x}");
    }
}
