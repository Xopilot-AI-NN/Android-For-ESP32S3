// Модуль переходов экрана и сохранения состояния.
//! Framework events and Activity-local page-out/page-in.
use crate::*;

/// Serialize requested swap configuration, graphic reads and ADB uploads.
pub(crate) fn service_disk_requests<D: BlockDevice, B: BootDisplay>(
    dev: &D,
    display: &mut B,
    os: &mut runtime::Runtime,
) {
    crate::persistent_log::service(dev);
    if crate::file_store::service(dev) {
        if let Ok(frame) = os.render() {
            let _ = display.runtime(&frame);
        }
    }
    crate::weather::save_location(dev);
    if crate::swap_config::service(dev) {
        if let Ok(frame) = os.render() {
            let _ = display.runtime(&frame);
        }
    }
    if crate::ui_resources::service(dev) {
        if let Ok(frame) = os.render() {
            let _ = display.runtime(&frame);
        }
    }
    if os.diagnostics_active() {
        crate::apps::storage::sample(dev, display.render_stats().last_us);
    }
    if let Some(pixels) = wallpaper::take_upload() {
        let ok = wallpaper::save(dev, &pixels);
        if ok {
            display.set_wallpaper(pixels);
            os.use_uploaded_wallpaper();
        }
        wallpaper::complete_upload(ok);
        println!(
            "WallpaperManager: uploaded wallpaper {}",
            if ok { "saved" } else { "save failed" }
        );
    }
}

/// Меняет один системный флаг без повреждения остальных битов.
pub(crate) fn set_runtime_flag(
    os: &mut runtime::Runtime,
    bit: u32,
    enabled: bool,
) -> Result<runtime::RuntimeFrame, runtime::RuntimeError> {
    let mask = 1i32 << bit;
    let state = if enabled {
        os.state() | mask
    } else {
        os.state() & !mask
    };
    os.replace_state(state)
}

/// Обрабатывает ввод и сохраняет только локальное состояние покидаемого экрана.
pub(crate) fn dispatch_with_pager<D: BlockDevice>(
    os: &mut runtime::Runtime,
    event: runtime::RuntimeEvent,
    pager: Option<&page_store::PageStore>,
    dev: &D,
    used: &mut u32,
) -> Result<runtime::RuntimeFrame, runtime::RuntimeError> {
    let old_state = os.state();
    let old_screen = (old_state & 0x1f) as u32;

    let mut frame = os.dispatch(event)?;
    let new_state = os.state();
    let new_screen = (new_state & 0x1f) as u32;
    if new_screen != old_screen {
        // Log meaningful navigation, not every redraw or crown sample.
        crate::watch_println!(
            "I ({}) ActivityTaskManager: screen {} -> {}",
            esp_hal::time::Instant::now()
                .duration_since_epoch()
                .as_millis(),
            old_screen,
            new_screen,
        );
        if let Some(pager) = pager {
            // Only evict an Activity when it actually leaves the foreground.
            // Crown movement inside one screen must not turn the SD/USB flash
            // into a write-every-20ms pseudo-swap device.
            let old_tag = 0x5a50_0000u32 | old_screen;
            if pager.write_i32(dev, old_screen, old_tag, old_state).is_ok() {
                if old_screen < 32 {
                    *used |= 1u32 << old_screen;
                }
            }

            let new_tag = 0x5a50_0000u32 | new_screen;
            if let Ok(saved) = pager.read_i32(dev, new_screen, new_tag) {
                // Page-in only Activity-local cursor bits. Global settings and
                // connectivity flags stay from the current system state.
                let restored = (new_state & !0x000001e0) | (saved & 0x000001e0);
                if let Ok(restored_frame) = os.replace_state(restored) {
                    frame = restored_frame;
                }
            }
        }
    }
    frame.set_swap_pages(used.count_ones().min(255) as u8);
    Ok(frame)
}
