#[cfg(feature = "rhai-runtime")]
use rhai::Engine;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError { Script }

#[cfg(feature = "rhai-runtime")]
pub fn run(script: &str) -> Result<i32, RuntimeError> {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(32, 16);
    engine.set_max_operations(100_000);
    engine.set_max_call_levels(16);

    engine.register_fn("boot_version", || 1_i32);
    #[cfg(feature = "display-ssd1306")]
    {
        engine.register_fn("device_width", || 128_i32);
        engine.register_fn("device_height", || 64_i32);
    }
    #[cfg(feature = "display-st7789")]
    {
        engine.register_fn("device_width", || 240_i32);
        engine.register_fn("device_height", || 280_i32);
    }

    engine.eval::<i32>(script).map_err(|_| RuntimeError::Script)
}

#[cfg(not(feature = "rhai-runtime"))]
pub fn run(_script: &str) -> Result<i32, RuntimeError> { Ok(0) }
