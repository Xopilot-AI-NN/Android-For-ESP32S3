//! Ограниченный загрузчик образов: GPT, A/B, AVB, boot, CPIO и liblp.
//! Не содержит runtime, SystemUI, приложений или их графических ресурсов.
#![no_std]
extern crate alloc;
/// Журнал загрузочного компонента не зависит от системных служб прошивки.
#[macro_export]
macro_rules! watch_println { ($($arg:tt)*) => { esp_println::println!($($arg)*) }; }
/// Ограничения формата, общие для загрузчика и сборщика образов.
pub mod config {
    pub const MAX_SCRIPT_BYTES: usize = 24 * 1024;
    pub const MAX_RUNTIME_BUNDLE_BYTES: usize = 32 * 1024;
}
pub mod android_boot;
pub mod android_storage;
pub mod avb;
pub mod boot_control;
pub mod cpio;
pub mod disk_probe;
pub mod gpt;
pub mod lp;
pub mod sd_bitbang;
pub mod sd_init;
pub mod sd_spi;
