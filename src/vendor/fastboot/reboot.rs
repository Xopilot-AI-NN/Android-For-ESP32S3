use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "reboot";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "software reset";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    ctx.fb.write_line("Rebooting...");
    ctx.board.status_led.show_write(ctx.delay);
    Some(FastbootExit::Reboot)
}