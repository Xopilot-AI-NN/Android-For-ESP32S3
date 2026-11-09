// Модуль отложенного выключения и перезагрузки.
//! Reset hand-off is retained in RTC RAM, validated, and consumed once at boot.
use crate::runtime::SystemAction;
/// Сигнатура, отличающая собственный формат данных от чужих.
const MAGIC: u32 = 0x5750_5752;
#[esp_hal::ram(unstable(rtc_fast, persistent))]
/// Параметр REQUEST компонента отложенного выключения и перезагрузки.
static mut REQUEST: [u32; 3] = [0; 3];
/// Ставит запрос в очередь для обработки владельцем ресурса.
pub fn request(action: SystemAction) -> ! {
    let value = action as u32;
    // The UI task is the only writer. Volatile access never creates a reference
    // to RTC memory that can survive a reset.
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(REQUEST), [MAGIC, value, !value]);
    }
    esp_hal::system::software_reset()
}
/// Забирает и очищает отложенную команду питания.
pub fn take() -> Option<SystemAction> {
    let raw = unsafe {
        let p = core::ptr::addr_of_mut!(REQUEST);
        let value = core::ptr::read_volatile(p);
        core::ptr::write_volatile(p, [0; 3]);
        value
    };
    if raw[0] != MAGIC || raw[1] != !raw[2] {
        return None;
    }
    match raw[1] {
        1 => Some(SystemAction::Reboot),
        2 => Some(SystemAction::Fastboot),
        3 => Some(SystemAction::Recovery),
        4 => Some(SystemAction::PowerOff),
        _ => None,
    }
}

/// On-board recovery navigation works before storage/Rhai are available.
pub fn recovery_board<D: crate::display::BootDisplay>(
    display: &mut D,
    keys: &mut crate::input::BootKeys<'_>,
    delay: &esp_hal::delay::Delay,
) -> crate::boot_policy::BootMode {
    let mut selected = 0u8;
    let mut dirty = true;
    loop {
        if dirty {
            let _ = display.boot_menu(2, selected);
            dirty = false;
        }
        let d = keys.poll_rotation();
        if d != 0 {
            selected = (i32::from(selected) + i32::from(d)).rem_euclid(4) as u8;
            dirty = true;
        }
        if let Some(event) = keys.poll_button_event() {
            match event {
                crate::runtime::RuntimeEvent::Press => match selected {
                    0 => request(SystemAction::Reboot),
                    // Возвращаем владение USB раннему boot-коду, без перезапуска/поиска диска.
                    1 => return crate::boot_policy::BootMode::Fastboot,
                    2 => request(SystemAction::PowerOff),
                    _ => return crate::boot_policy::BootMode::Normal,
                },
                crate::runtime::RuntimeEvent::Home | crate::runtime::RuntimeEvent::Back => return crate::boot_policy::BootMode::Normal,
                _ => {}
            }
        }
        delay.delay_millis(10);
    }
}
