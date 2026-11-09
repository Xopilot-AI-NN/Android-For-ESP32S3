// Модуль сведений о накопителе.
//! Read storage metadata once, on demand, through the main loop's disk owner.
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embedded_sdmmc::BlockDevice;
/// Параметр READY компонента сведений о накопителе.
static READY: AtomicBool = AtomicBool::new(false);
/// Параметр DISK_MIB компонента сведений о накопителе.
static DISK_MIB: AtomicU32 = AtomicU32::new(0);
/// Параметр USER_MIB компонента сведений о накопителе.
static USER_MIB: AtomicU32 = AtomicU32::new(0);
/// Параметр SUPER_MIB компонента сведений о накопителе.
static SUPER_MIB: AtomicU32 = AtomicU32::new(0);
/// Параметр FRAME_US компонента сведений о накопителе.
static FRAME_US: AtomicU32 = AtomicU32::new(0);
/// Сообщает, получены ли необходимые данные.
pub fn ready() -> bool {
    READY.load(Ordering::Acquire)
}
/// Возвращает согласованный снимок текущего состояния.
pub fn snapshot() -> (u32, u32, u32, u32) {
    (
        DISK_MIB.load(Ordering::Relaxed),
        USER_MIB.load(Ordering::Relaxed),
        SUPER_MIB.load(Ordering::Relaxed),
        FRAME_US.load(Ordering::Relaxed),
    )
}
/// Считывает сведения о накопителе и обновляет диагностический снимок.
pub fn sample<D: BlockDevice>(dev: &D, frame_us: u64) {
    FRAME_US.store(frame_us.min(u64::from(u32::MAX)) as u32, Ordering::Relaxed);
    if ready() {
        return;
    }
    if let Ok(blocks) = dev.num_blocks() {
        DISK_MIB.store(blocks.0 / 2048, Ordering::Relaxed);
    }
    for (name, destination) in [("userdata", &USER_MIB), ("super", &SUPER_MIB)] {
        if let Ok(part) = crate::gpt::find_partition(dev, name) {
            destination.store((part.size_bytes() / 1048576) as u32, Ordering::Relaxed);
        }
    }
    READY.store(true, Ordering::Release);
}

/// Copied by the disk/display owner; painting diagnostics never touches either bus.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DisplaySnapshot {
    pub drawn: u32,
    pub skipped: u32,
    pub last_us: u64,
    pub max_us: u64,
    pub pixels: u32,
    pub raster_us: u64,
    pub compose_us: u64,
    pub transfer_us: u64,
}
static DISPLAY: critical_section::Mutex<core::cell::RefCell<DisplaySnapshot>> =
    critical_section::Mutex::new(core::cell::RefCell::new(DisplaySnapshot {
        drawn: 0,
        skipped: 0,
        last_us: 0,
        max_us: 0,
        pixels: 0,
        raster_us: 0,
        compose_us: 0,
        transfer_us: 0,
    }));
pub fn display_snapshot() -> DisplaySnapshot {
    critical_section::with(|cs| *DISPLAY.borrow_ref(cs))
}
pub fn sample_display(value: DisplaySnapshot) {
    critical_section::with(|cs| *DISPLAY.borrow_ref_mut(cs) = value);
}
