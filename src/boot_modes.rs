//! Normal/Recovery/Fastboot boot-mode selection and navigation.
use crate::*;

#[cfg(not(feature = "pc-block-boot"))]
pub(crate) fn enter_fastboot<'d, D: BootDisplay>(
    mut display: DesktopMirror<'d, D>,
    input: DesktopInput<'d>,
    keys: &mut BootKeys<'_>,
    delay: &Delay,
) -> ! {
    // Send the Fastboot screen through the desktop mirror before handing the
    // CDC TX/RX halves to the Fastboot+ command loop.
    let _ = display.fastboot();
    let (mut physical_display, tx) = display.into_parts();
    fastboot::run(input.into_inner(), tx, &mut physical_display, keys, delay)
}

#[cfg(not(feature = "pc-block-boot"))]
pub(crate) fn sample_boot_mode<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
    window_ms: u32,
) -> BootMode {
    let ticks = (window_ms / 10).max(1);
    for _ in 0..ticks {
        if keys.power_pressed() {
            return BootMode::Fastboot;
        }
        if keys.poll_rotation() != 0 {
            return BootMode::Recovery;
        }

        if let Some(event) = desktop.poll() {
            match event {
                DesktopEvent::Sync => {
                    display.hello();
                    let _ = display.boot_logo();
                }
                DesktopEvent::Ping => display.pong(),
                DesktopEvent::Fastboot | DesktopEvent::Power => return BootMode::Fastboot,
                DesktopEvent::Recovery | DesktopEvent::Rotate(_) => return BootMode::Recovery,
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

#[cfg(not(feature = "pc-block-boot"))]
pub(crate) fn recovery_menu<D: BootDisplay>(
    display: &mut DesktopMirror<'_, D>,
    keys: &mut BootKeys<'_>,
    desktop: &mut DesktopInput<'_>,
    delay: &Delay,
) -> BootMode {
    let _ = desktop;
    crate::power::recovery_board(display, keys, delay);
    BootMode::Normal
}
