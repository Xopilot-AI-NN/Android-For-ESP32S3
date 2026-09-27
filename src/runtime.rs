extern crate alloc;

use alloc::{format, string::{String, ToString}};
use esp_hal::time::Instant;

use crate::material::MaterialFrame;

#[cfg(feature = "rhai-runtime")]
use esp_println::println;
#[cfg(feature = "rhai-runtime")]
use rhai::{AST, CallFnOptions, Engine, ImmutableString, Scope};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError { Script, NonZero, Protocol }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    Sync,
    Home,
    Rotate(i32),
    Press,
}

#[derive(Clone, Debug)]
pub struct LegacyFrame {
    pub title: String,
    pub line1: String,
    pub line2: String,
    pub line3: String,
}

#[derive(Clone, Debug)]
pub enum RuntimeFrame {
    Material(MaterialFrame),
    Legacy(LegacyFrame),
}

impl RuntimeFrame {
    pub fn set_swap_pages(&mut self, pages: u8) {
        if let Self::Material(frame) = self { frame.set_swap_pages(pages); }
    }


    fn parse(raw: &str) -> Result<Self, RuntimeError> {
        if let Some(frame) = MaterialFrame::parse(raw) {
            return Ok(Self::Material(frame));
        }
        let mut parts = raw.split('|');
        let title = parts.next().ok_or(RuntimeError::Protocol)?;
        let line1 = parts.next().ok_or(RuntimeError::Protocol)?;
        let line2 = parts.next().ok_or(RuntimeError::Protocol)?;
        let line3 = parts.next().ok_or(RuntimeError::Protocol)?;
        if parts.next().is_some() { return Err(RuntimeError::Protocol); }
        if title.len() > 22 || line1.len() > 28 || line2.len() > 28 || line3.len() > 28 {
            return Err(RuntimeError::Protocol);
        }
        Ok(Self::Legacy(LegacyFrame {
            title: title.to_string(),
            line1: line1.to_string(),
            line2: line2.to_string(),
            line3: line3.to_string(),
        }))
    }
}

#[cfg(feature = "rhai-runtime")]
pub struct Runtime {
    engine: Engine,
    scope: Scope<'static>,
    ast: AST,
    /// Firmware-owned persistent state.  Rhai functions are intentionally
    /// pure with respect to outer Scope variables, so state crosses the ABI
    /// explicitly instead of relying on closure/global-variable semantics.
    state: i32,
}

#[cfg(feature = "rhai-runtime")]
impl Runtime {
    pub fn start(script: String) -> Result<(Self, RuntimeFrame), RuntimeError> {
        let mut engine = Engine::new();
        // Firmware is trusted/AVB-verified userspace. Keep a bounded parser, but
        // allow normal Material/SystemUI expressions inside function bodies.
        engine.set_max_expr_depths(64, 32);
        engine.set_max_operations(60_000);
        engine.set_max_call_levels(16);

        engine.register_fn("boot_version", || 2_i32);
        engine.register_fn("android_api_level", || 37_i32);
        engine.register_fn("runtime_abi", || "aosp-wear-android17-qpr1-rhai-v6-m3e".to_string());
        engine.register_fn("m3_scene", |
            screen: i32, cursor: i32, brightness: i32, dnd: i32, airplane: i32,
            theme: i32, locked: i32, wifi: i32, bt: i32, adb: i32, notes: i32
        | {
            // Until RTC/NTP sets wall-clock time, SystemUI displays a stable
            // boot-relative clock.  ConnectivityService can later apply an
            // epoch offset without changing the scene ABI.
            let mins = (Instant::now().duration_since_epoch().as_minutes() % (24 * 60)) as i32;
            format!(
                "M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                screen.clamp(0, 8), cursor.clamp(0, 15), brightness.clamp(0, 100),
                if dnd != 0 { 1 } else { 0 }, if airplane != 0 { 1 } else { 0 },
                theme.rem_euclid(4), if locked != 0 { 1 } else { 0 },
                if wifi != 0 { 1 } else { 0 }, if bt != 0 { 1 } else { 0 },
                if adb != 0 { 1 } else { 0 }, notes.clamp(0, 7), mins
            )
        });
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

        let ast = engine.compile(script.as_str()).map_err(|err| {
            println!("rhai compile error: {:?}", err);
            RuntimeError::Script
        })?;
        drop(script);

        let mut scope = Scope::new();
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let state = engine
            .call_fn_with_options::<i32>(options, &mut scope, &ast, "init", ())
            .map_err(|err| {
                println!("rhai init error: {:?}", err);
                RuntimeError::Script
            })?;
        if state < 0 { return Err(RuntimeError::NonZero); }

        let mut runtime = Self { engine, scope, ast, state };
        let frame = runtime.render()?;
        Ok((runtime, frame))
    }

    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let raw = self.engine
            .call_fn_with_options::<ImmutableString>(
                options,
                &mut self.scope,
                &self.ast,
                "render",
                (self.state,),
            )
            .map_err(|err| {
                println!("rhai render error: {:?}", err);
                RuntimeError::Script
            })?;
        RuntimeFrame::parse(raw.as_str())
    }

    pub fn state(&self) -> i32 { self.state }

    pub fn replace_state(&mut self, state: i32) -> Result<RuntimeFrame, RuntimeError> {
        if state < 0 { return Err(RuntimeError::NonZero); }
        self.state = state;
        self.render()
    }

    pub fn dispatch(&mut self, event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        let (kind, value) = match event {
            RuntimeEvent::Sync => ("sync", 0),
            RuntimeEvent::Home => ("home", 0),
            RuntimeEvent::Rotate(delta) => ("rotate", delta),
            RuntimeEvent::Press => ("press", 0),
        };
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let new_state = self.engine
            .call_fn_with_options::<i32>(
                options,
                &mut self.scope,
                &self.ast,
                "on_event",
                (self.state, kind.to_string(), value),
            )
            .map_err(|err| {
                println!("rhai event error: {:?}", err);
                RuntimeError::Script
            })?;
        if new_state < 0 { return Err(RuntimeError::NonZero); }
        self.state = new_state;
        self.render()
    }
}

#[cfg(not(feature = "rhai-runtime"))]
pub struct Runtime;

#[cfg(not(feature = "rhai-runtime"))]
impl Runtime {
    pub fn start(_script: String) -> Result<(Self, RuntimeFrame), RuntimeError> {
        Ok((Self, RuntimeFrame::Legacy(LegacyFrame {
            title: "ANDROID".to_string(),
            line1: "RHAI DISABLED".to_string(),
            line2: "".to_string(),
            line3: "".to_string(),
        })))
    }
    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        Self::start(String::new()).map(|v| v.1)
    }
    pub fn dispatch(&mut self, _event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    pub fn state(&self) -> i32 { 0 }
    pub fn replace_state(&mut self, _state: i32) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
}
