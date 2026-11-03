// Модуль аппаратной диагностики SD.
//! Hardware-only SD probe. No filesystem writes, display or radio startup.
#![no_std]
#![no_main]
extern crate alloc;
#[macro_export]
macro_rules! watch_println {
    ($($arg:tt)*) => { esp_println::println!($($arg)*) };
}
use embedded_hal::spi::SpiBus;
use embedded_sdmmc::SdCard;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    main,
};
use esp_println::println;
#[path = "../../src/disk_probe.rs"]
mod disk_probe;
#[allow(dead_code)]
#[path = "../../src/gpt.rs"]
mod gpt;
#[allow(dead_code)]
#[path = "../../src/sd_bitbang.rs"]
mod sd_bitbang;
#[allow(dead_code)]
#[path = "../../src/sd_init.rs"]
mod sd_init;
#[allow(dead_code)]
#[path = "../../src/sd_spi.rs"]
mod sd_spi;
esp_bootloader_esp_idf::esp_app_desc!();
#[main]
/// Запускает аппаратную SD-диагностику на выделенной распиновке платы.
fn main() -> ! {
    esp_alloc::heap_allocator!(size:96*1024);
    let p = esp_hal::init(
        esp_hal::Config::default()
            .with_cpu_clock(CpuClock::max())
            .with_psram(esp_hal::psram::PsramConfig::default()),
    );
    let _backlight = Output::new(p.GPIO6, Level::Low, OutputConfig::default());
    let _display_cs = Output::new(p.GPIO5, Level::High, OutputConfig::default());
    let mut delay = Delay::new();
    println!("SD SELFTEST: no display, radio, RTOS or Rhai; read-only");
    println!("SD SELFTEST: SCK=8 MOSI=9 MISO=7 CS=10; CPU=240MHz; GPIO mode 0");
    let mut cs = Output::new(p.GPIO10, Level::High, OutputConfig::default());
    let mut spi = sd_bitbang::SdBitBang::new(
        Output::new(p.GPIO8, Level::Low, OutputConfig::default()),
        Output::new(p.GPIO9, Level::High, OutputConfig::default()),
        Input::new(p.GPIO7, InputConfig::default().with_pull(Pull::Up)),
        delay,
    )
    .unwrap();
    let _ = cs.set_high();
    delay.delay_millis(100);
    let _ = spi.write(&[0xff; 10]);
    let mut device = sd_spi::SdSpiSession::new(spi, cs, delay);
    let mut attempt = 0u32;
    loop {
        attempt = attempt.wrapping_add(1);
        println!("SD SELFTEST: attempt {attempt}");
        match sd_init::handshake_clocked(&mut device, &mut delay) {
            Err(error) => println!("SD SELFTEST: startup failed: {error}"),
            Ok(kind) => {
                let card = SdCard::new(&mut device, delay);
                // SAFETY: the same device just completed the checked handshake,
                // including addressing/OCR/CRC and block-length negotiation.
                unsafe {
                    card.mark_card_as_init(kind);
                }
                match card.num_bytes() {
                    Ok(n) => println!("SD SELFTEST: capacity={} MiB", n / (1024 * 1024)),
                    Err(e) => println!("SD SELFTEST: CSD failed: {e:?}"),
                }
                disk_probe::report(&card);
                println!("SD SELFTEST: GPT={:?}", gpt::probe(&card));
            }
        }
        let _ = device.finish_command();
        println!("SD SELFTEST: no writes performed; next probe in 3 seconds");
        delay.delay_millis(3000);
    }
}
