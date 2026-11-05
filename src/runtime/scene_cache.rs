// Модуль кэша результатов Rhai.
//! Cache the script's state-dependent scene; Rust refreshes live clock fields.
use super::*;

#[derive(Clone, Copy, Debug, Default)]
/// Счётчики исполнения Rhai и попаданий кэша компонента кэша результатов Rhai.
pub struct ScriptStats {
    /// Число фактических вызовов вычисления/отрисовки.
    pub calls: u32,
    /// Число повторно использованных результатов кэша.
    pub cache_hits: u32,
    /// Время/интервал last в микросекундах.
    pub last_us: u64,
}

#[derive(Default)]
#[cfg(feature = "rhai-runtime")]
/// Кэш подготовленного Rhai-кадра по состоянию компонента кэша результатов Rhai.
pub(super) struct SceneCache {
    /// Закэшированный кадр и ключ состояния.
    scene: Option<alloc::boxed::Box<(i32, MaterialFrame)>>,
    /// Счётчики вызовов, попаданий кэша и времени обработки.
    pub stats: ScriptStats,
}

#[cfg(feature = "rhai-runtime")]
impl SceneCache {
    /// Формирует или выводит кадр текущего состояния.
    #[inline(never)]
    pub fn render(
        &mut self,
        engine: &Engine,
        scope: &mut Scope<'static>,
        ast: &AST,
        state: i32,
    ) -> Result<RuntimeFrame, RuntimeError> {
        if let Some((key, scene)) = self.scene.as_deref() {
            if *key == state {
                self.stats.cache_hits = self.stats.cache_hits.saturating_add(1);
                return Ok(RuntimeFrame::Material(alloc::boxed::Box::new(*scene)));
            }
        }
        let began = Instant::now().duration_since_epoch().as_micros() as u64;
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let raw = engine
            .call_fn_with_options::<ImmutableString>(options, scope, ast, "render", (state,))
            .map_err(|err| {
                crate::watch_println!("rhai render error: {:?}", err);
                RuntimeError::Script
            })?;
        self.stats.calls = self.stats.calls.saturating_add(1);
        self.stats.last_us =
            (Instant::now().duration_since_epoch().as_micros() as u64).saturating_sub(began);
        let frame = RuntimeFrame::parse(raw.as_str())?;
        // Legacy text can depend on arbitrary script data; only the Material
        // ABI has native live-field refresh and is safe to reuse this way.
        match &frame {
            RuntimeFrame::Material(scene) => {
                if let Some(cached) = self.scene.as_mut() {
                    **cached = (state, **scene);
                } else {
                    self.scene = Some(alloc::boxed::Box::new((state, **scene)));
                }
            }
            _ => self.scene = None,
        }
        Ok(frame)
    }
}
