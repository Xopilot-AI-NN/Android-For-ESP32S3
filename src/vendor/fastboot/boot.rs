use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "boot";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "exit fastboot+ and continue boot (requires SD card)";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    let sd = ctx.board.probe_sd_card();
    if sd.is_present() {
        ctx.fb.write_line("SD detected — leaving fastboot+ and continuing boot.");
        ctx.board.status_led.show_success(ctx.delay);
        return Some(FastbootExit::ContinueBoot);
    }
    ctx.fb.write_fmt_line(format_args!(
        "ERR SD probe: {}  — boot denied.",
        sd.as_str()
    ));
    ctx.fb.write_prompt();
    None
}
