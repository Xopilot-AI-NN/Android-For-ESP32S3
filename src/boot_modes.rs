// Модуль выбора fastboot/recovery.
//! Normal/Recovery/Fastboot boot-mode selection and navigation.
use crate::*;

/// Передаёт управление аппаратному режиму fastboot.
pub(crate) fn enter_fastboot<'d, D: BootDisplay>(
    mut display: DesktopMirror<'d, D>,
    input: DesktopInput<'d>,
    keys: &mut BootKeys<'_>,
    delay: &Delay,
) -> ! {
    // Send the Fastboot screen through the desktop mirror before handing the
    // CDC TX/RX halves to the Fastboot+ command loop.
    keys.consume_boot_press();
    let _ = display.fastboot();
    let (mut physical_display, tx) = display.into_parts();
    fastboot::run(input.into_inner(), tx, &mut physical_display, keys, delay)
}

/// Определяет режим запуска по кнопке в ограниченном временном окне.
pub(crate) fn sample_boot_mode<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
    window_ms: u32,
) -> BootMode {
    let began = esp_hal::time::Instant::now().duration_since_epoch().as_millis();
    let mut next_logo = began;
    loop {
        let time = esp_hal::time::Instant::now().duration_since_epoch().as_millis();
        if time.saturating_sub(began) >= u64::from(window_ms) { break; }
        if time >= next_logo && crate::startup::animation_phase() < 31 {
            let _ = display.boot_logo();
            next_logo = time + 50;
        }
        if keys.power_pressed() {
            return BootMode::Fastboot;
        }
        // Направление физического энкодера не зависит от пользовательского reverse.
        let direction = keys.poll_rotation();
        let selection = crate::boot_policy::direction(i32::from(direction));
        if selection != BootMode::Normal { return selection; }

        if let Some(event) = desktop.poll() {
            match event {
                DesktopEvent::Sync => {
                    display.hello();
                    let _ = display.boot_logo();
                }
                DesktopEvent::Ping => display.pong(),
                DesktopEvent::Fastboot | DesktopEvent::Power => return BootMode::Fastboot,
                DesktopEvent::Recovery => return BootMode::Recovery,
                DesktopEvent::Rotate(d) if d > 0 => return BootMode::Fastboot,
                DesktopEvent::Rotate(d) if d < 0 => return BootMode::Recovery,
                DesktopEvent::Rotate(_) => {}
                DesktopEvent::Swipe(_) => {}
                DesktopEvent::Normal => return BootMode::Normal,
                DesktopEvent::CompanionTime(_)
                | DesktopEvent::AdbCommand(_)
                | DesktopEvent::AdbGetProp
                | DesktopEvent::AdbServices
                | DesktopEvent::AdbPackages
                | DesktopEvent::AdbDumpsys
                | DesktopEvent::WifiScan
                | DesktopEvent::WifiStatus
                | DesktopEvent::WifiConnect(_)
                | DesktopEvent::WifiDisconnect
                | DesktopEvent::BtScan
                | DesktopEvent::BtStatus
                | DesktopEvent::BtConnect(_)
                | DesktopEvent::BtDisconnect => {}
            }
        }
        delay.delay_millis(10);
    }
    BootMode::Normal
}

/// Обслуживает меню восстановления без запуска обычного интерфейса.
pub(crate) fn recovery_menu<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
) -> BootMode {
    let _ = desktop;
    crate::power::recovery_board(display, keys, delay)
}
