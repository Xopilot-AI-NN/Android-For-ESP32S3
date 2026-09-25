use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "rtc";
pub const CMD_ALIASES: &[&str] = &[];
pub const CMD_HELP: &str = "current RTC date/time";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    let (year, month, day) = crate::vendor::rtc::get_date();
    let (hour, minute, second, millisecond) = crate::vendor::rtc::get_time();
    ctx.fb.write_fmt_line(format_args!(
        "RTC {:04}-{:02}-{:02}  {:02}:{:02}:{:02}:{:02}",
        year, month, day, hour, minute, second, millisecond
    ));
    ctx.fb.write_prompt();
    None
}
