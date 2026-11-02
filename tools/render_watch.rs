//! Host renderer for the exact RGB565 firmware surface; no display mockups.
//! Build through tools/runtime-check/Cargo.toml, bin render-watch (host target).
extern crate alloc;
#[allow(dead_code)]
#[path = "../src/device_info.rs"]
mod device_info;
#[allow(dead_code)]
#[path = "../src/device_name.rs"]
mod device_name;
#[allow(dead_code)]
#[path = "../src/gpt.rs"]
mod gpt;
#[allow(dead_code)]
#[path = "../src/wallpaper.rs"]
mod wallpaper;
#[allow(dead_code)]
#[path = "../src/material.rs"]
mod material;
use std::{fs, io::Write};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "out/ui-preview".into());
    fs::create_dir_all(&out).unwrap();
    for (name, screen, cursor) in [
        ("watch", 1, 0),
        ("analog",1,0),
        ("complications",1,0),
        ("power",1,0),
        ("splash",1,0),
        ("fastboot",1,0),
        ("recovery",1,0),
        ("display",14,3),
        ("media-playing",10,1),
        ("quick", 4, 0),
        ("settings", 5, 0),
        ("connectivity", 7, 0),
        ("wifi", 12, 0),
        ("bluetooth", 13, 1),
        ("pairing", 30, 0),
        ("media", 10, 3),
        ("about", 8, 0),
        ("launcher", 2, 2),
        ("settings-scroll", 5, 4),
        ("clock", 11, 0),
        ("timer-presets", 11, 2),
        ("timer-custom", 11, 4),
        ("list-launcher", 26, 6),
        ("tiles", 9, 1),
        ("alarm", 31, 0),
        ("timer-alert", 11, 0),
        ("brightness", 28, 0),
        ("keyboard", 24, 0),
        ("gestures", 18, 0),
        ("security", 20, 1),
        ("pin-entry", 0, 0),
        ("lock-timeout", 20, 0),
    ] {
        let raw = format!("M3|{screen}|{cursor}|60|0|0|0|0|1|1|0|0|131|0|0|1|1|1|3");
        let mut frame = material::MaterialFrame::parse(&raw).unwrap();
        if name == "timer-presets" {
            frame.timer_editor = 1;
        }
        if name == "timer-custom" {
            frame.timer_editor = 2;
        }
        if name == "pin-entry" {
            frame.has_pin = true;
            frame.security_view = 3;
            frame.pin_count = 2;
            frame.pin_digit = 5;
        }
        if name == "lock-timeout" {
            frame.has_pin = true;
            frame.security_view = 5;
            frame.pin_digit = 10;
        }
        frame.bt_status = 3;
        frame.wifi_link_state = 4;
        frame.wifi_ip = Some([10, 214, 249, 238]);
        if name == "timer-alert" {
            frame.clock_alert = 1;
        }
        if screen == 30 {
            frame.bt_pin = Some(123456);
        }
        frame.time_minutes = 23*60+29; frame.time_seconds=37;
        frame.date_day=1;frame.date_month=11;
        if name=="analog" {frame.watch_face=1;}
        if name=="complications" {frame.watch_face=2;}
        if name=="power" {frame.power_menu=true;}
        if name=="media-playing" {frame.media_known=true;frame.media_playing=true;frame.media_progress=420;frame.animation_phase=100;}
        let mut surface = material::Surface::new();
        if name=="splash" {surface.boot_frame(255);}
        else if name=="fastboot" {surface.boot_menu_frame(1,2);}
        else if name=="recovery" {surface.boot_menu_frame(2,1);}
        else {surface.render(frame);}
        let mut file = fs::File::create(format!("{out}/{name}.ppm")).unwrap();
        write!(file, "P6\n240 280\n255\n").unwrap();
        for &p in surface.pixels() {
            file.write_all(&[
                (((p >> 11) & 31) * 255 / 31) as u8,
                (((p >> 5) & 63) * 255 / 63) as u8,
                ((p & 31) * 255 / 31) as u8,
            ])
            .unwrap();
        }
    }
}
