use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "status";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "board/component health report";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    let sd = ctx.board.probe_sd_card();
    ctx.fb.write_line("Board status:");
    ctx.fb.write_fmt_line(format_args!(
        "  sd_present={}  sd_status={}",
        sd.is_present(),
        sd.as_str()
    ));
    ctx.fb.write_fmt_line(format_args!(
        "  encoder_active={}  power_pressed={}",
        ctx.board.encoder_active(),
        ctx.board.power_button_pressed(),
    ));
    ctx.fb.write_fmt_line(format_args!("  display={:?}", ctx.board.display_status()));
    ctx.fb.write_prompt();
    None
}
