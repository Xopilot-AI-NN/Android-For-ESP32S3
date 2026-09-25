use super::{CmdCtx, FastbootExit, LedMode};

pub const CMD_NAME: &str = "led";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "led R G B [BRT]  — set status LED  (BRT 0..10, default 10)";

pub fn run(arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    match super::parse_rgb_arg(arg) {
        Some((color, brightness)) => {
            *ctx.led_mode = LedMode::Manual;
            ctx.board.status_led.show(ctx.delay, color.with_brightness(brightness));
            ctx.fb.write_fmt_line(format_args!(
                "LED -> rgb({}, {}, {})  brightness {}",
                color.red, color.green, color.blue, brightness
            ));
        }
        None => {
            ctx.fb.write_line("ERR led: usage  led R G B [BRT]  e.g. led 255 64 0 5");
        }
    }
    ctx.fb.write_prompt();
    None
}
