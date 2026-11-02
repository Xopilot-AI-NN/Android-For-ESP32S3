//! Read storage metadata once, on demand, through the main loop's disk owner.
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embedded_sdmmc::BlockDevice;
static READY: AtomicBool = AtomicBool::new(false);
static DISK_MIB: AtomicU32 = AtomicU32::new(0);
static USER_MIB: AtomicU32 = AtomicU32::new(0);
static SUPER_MIB: AtomicU32 = AtomicU32::new(0);
static FRAME_US: AtomicU32 = AtomicU32::new(0);
pub fn ready() -> bool {
    READY.load(Ordering::Acquire)
}
pub fn snapshot() -> (u32, u32, u32, u32) {
    (
        DISK_MIB.load(Ordering::Relaxed),
        USER_MIB.load(Ordering::Relaxed),
        SUPER_MIB.load(Ordering::Relaxed),
        FRAME_US.load(Ordering::Relaxed),
    )
}
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
