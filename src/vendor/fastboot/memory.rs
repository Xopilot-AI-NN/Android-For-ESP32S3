use super::{CmdCtx, FastbootExit};

pub const CMD_NAME: &str = "memory";
pub const CMD_ALIASES: &[&str] = &["mem"];
pub const CMD_HELP: &str = "memory snapshot (written to main console)";

pub fn run(_arg: &[u8], ctx: &mut CmdCtx<'_, '_>) -> Option<FastbootExit> {
    ctx.fb.write_line("Memory snapshot written to main console.");
    crate::components::diagnostics::log_memory(ctx.ram, ctx.cpu_to_gpu);
    ctx.fb.write_prompt();
    None
}
