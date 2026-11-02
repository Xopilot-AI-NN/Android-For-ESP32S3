//! Whitelisted developer commands; input follows the normal runtime PIN gate.
use crate::{
    adb,
    display::BootDisplay,
    runtime::{Runtime, RuntimeEvent, SystemAction},
    wallpaper,
};
use core::fmt::Write;
use embedded_sdmmc::BlockDevice;
pub fn execute<D: BlockDevice, B: BootDisplay>(
    cmd: &str,
    os: &mut Runtime,
    display: &mut B,
    dev: &D,
) -> heapless::String<512> {
    let mut out = heapless::String::new();
    let mut words = cmd.split_whitespace();
    let name = words.next().unwrap_or("");
    match name {
        "display" => {
            let _ = write!(
                out,
                "state={} script={:?} renderer={:?}",
                os.state(),
                os.script_stats(),
                display.render_stats()
            );
        }
        "state" | "input" | "clock" => {
            let _ = write!(
                out,
                "state={} sensitivity={} prefs={:?}",
                os.state(),
                os.crown_sensitivity(),
                os.preference_config()
            );
        }
        "home" | "back" | "press" | "power-menu" => {
            let event = match name {
                "home" => RuntimeEvent::Home,
                "back" => RuntimeEvent::Back,
                "power-menu" => RuntimeEvent::Power,
                _ => RuntimeEvent::Press,
            };
            let _ = os.dispatch(event);
            let _ = out.push_str("OK");
        }
        "rotate" => {
            if let Some(n) = words.next().and_then(|x| x.parse::<i32>().ok()) {
                let _ = os.dispatch(RuntimeEvent::Rotate(n.clamp(-10, 10)));
                let _ = out.push_str("OK");
            }
        }
        "reboot" | "poweroff" => {
            let action = if name == "poweroff" {
                SystemAction::PowerOff
            } else {
                match words.next() {
                    Some("recovery") => SystemAction::Recovery,
                    Some("fastboot") => SystemAction::Fastboot,
                    _ => SystemAction::Reboot,
                }
            };
            let _ = out.push_str(if os.request_system_action(action) {
                "OK"
            } else {
                "locked"
            });
        }
        "wallpaper-reload" => {
            if os.is_locked() {
                let _ = out.push_str("locked");
            } else if let Some(pixels) = wallpaper::load(dev) {
                display.set_wallpaper(pixels);
                os.wallpaper_available(true);
                let _ = out.push_str("OK");
            } else {
                let _ = out.push_str("no valid wallpaper");
            }
        }
        "screencap" => {
            if os.is_locked() {
                let _ = out.push_str("locked");
            } else {
                let saved = display
                    .snapshot_rgb565()
                    .map(|pixels| wallpaper::save_screenshot(dev, pixels))
                    .unwrap_or(false);
                let _ = out.push_str(if saved {
                    "OK userdata+1MiB"
                } else {
                    "capture failed"
                });
            }
        }
        "help" => {
            let _=out.push_str("state display input clock home back press rotate N power-menu reboot [recovery|fastboot] poweroff screencap wallpaper-reload");
        }
        _ => adb::shell_command(cmd, &mut out),
    }
    out
}
