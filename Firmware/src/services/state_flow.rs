// Модуль переходов экрана и сохранения состояния.
//! Framework events and Activity-local page-out/page-in.
use crate::*;
static ACTIVITY_PAGES: critical_section::Mutex<
    core::cell::RefCell<crate::activity_pages::ActivityPages>,
> = critical_section::Mutex::new(core::cell::RefCell::new(
    crate::activity_pages::ActivityPages::new(),
));

/// Serialize requested swap configuration, graphic reads and ADB uploads.
#[inline(never)]
pub(crate) fn service_disk_requests<D: BlockDevice, B: BootDisplay>(
    dev: &D,
    display: &mut B,
    os: &mut runtime::Runtime,
) {
    crate::temperature::sample();
    crate::persistent_log::service(dev);
    crate::google_gateway::service_storage(dev);
    // Service every queue, then render once. Keep the large frame out of this
    // disk-service stack frame, especially during the first BLE/Wi-Fi startup.
    let mut redraw = crate::file_store::service(dev);
    crate::weather::save_location(dev);
    redraw |= crate::swap_config::service(dev);
    redraw |= crate::ui_resources::service(dev);
    if redraw {
        redraw_after_disk(display, os);
    }
    if os.diagnostics_active() {
        let stats = display.render_stats();
        crate::apps::storage::sample(dev, stats.last_us);
        crate::apps::storage::sample_display(crate::apps::storage::DisplaySnapshot {
            drawn: stats.drawn,
            skipped: stats.skipped,
            last_us: stats.last_us,
            max_us: stats.max_us,
            pixels: stats.last_pixels,
            raster_us: stats.raster_us,
            compose_us: stats.compose_us,
            transfer_us: stats.transfer_us,
        });
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
            let changed = critical_section::with(|cs| {
                ACTIVITY_PAGES
                    .borrow_ref(cs)
                    .needs_write(old_screen, old_state)
            });
            let written = if changed {
                let result = pager.write_i32(dev, old_screen, old_tag, old_state);
                if let Err(error) = result {
                    println!("ZPager: cursor save failed: {:?}", error);
                }
                result.is_ok()
            } else {
                false
            };
            if written {
                critical_section::with(|cs| {
                    ACTIVITY_PAGES
                        .borrow_ref_mut(cs)
                        .remember(old_screen, Some(old_state))
                });
                if old_screen < 32 {
                    *used |= 1u32 << old_screen;
                }
            }

            let new_tag = 0x5a50_0000u32 | new_screen;
            let cached = critical_section::with(|cs| ACTIVITY_PAGES.borrow_ref(cs).get(new_screen));
            let saved = match cached {
                Some(value) => value,
                None => {
                    let result = pager.read_i32(dev, new_screen, new_tag);
                    let value = result.ok();
                    // Retry transport failures on the next visit; cache only
                    // validated cursors or terminal empty/corrupt records.
                    if !matches!(result, Err(crate::page_store::PageStoreError::Gpt(_))) {
                        critical_section::with(|cs| {
                            ACTIVITY_PAGES
                                .borrow_ref_mut(cs)
                                .remember(new_screen, value)
                        });
                    }
                    value
                }
            };
            if let Some(saved) = saved {
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

#[inline(never)]
fn redraw_after_disk<B: BootDisplay>(display: &mut B, os: &mut runtime::Runtime) {
    if let Ok(frame) = os.render() {
        let _ = display.runtime(&frame);
    }
}
