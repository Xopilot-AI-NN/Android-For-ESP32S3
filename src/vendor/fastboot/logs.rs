use super::{CmdCtx, FastbootExit, LedMode};

pub const CMD_NAME: &str = "logs";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "logs on|off  — live status streaming";

pub fn run(arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    match arg {
        b"on" => {
            *ctx.stream_logs = true;
            *ctx.led_mode = LedMode::Manual;
            ctx.board.status_led.show_success(ctx.delay);
            ctx.fb.write_line("Live fastboot logs enabled.");
        }
        b"off" => {
            *ctx.stream_logs = false;
            *ctx.led_mode = LedMode::Manual;
            ctx.board.status_led.show_success(ctx.delay);
            ctx.fb.write_line("Live fastboot logs disabled.");
        }
        _ => {
            ctx.fb.write_line("ERR logs: use `logs on` or `logs off`");
        }
    }
    ctx.fb.write_prompt();
    None
}
