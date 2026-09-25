use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "pins";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "configured pin map";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    ctx.fb.write_line("Pin map:");
    ctx.fb.write_line("  RGB_LED=21");
    ctx.fb.write_line("  SD: CS=12  CLK=disabled  MOSI=11  MISO=10  CD=none");
    ctx.fb.write_line("  DISPLAY: SDA=2  SCL=1");
    ctx.fb.write_line("  ENCODER: A=8  B=7");
    ctx.fb.write_line("  POWER_BUTTON=9");
    ctx.fb.write_prompt();
    None
}
