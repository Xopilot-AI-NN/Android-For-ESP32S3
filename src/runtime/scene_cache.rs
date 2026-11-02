//! Cache the script's state-dependent scene; Rust refreshes live clock fields.
use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptStats {
    pub calls: u32,
    pub cache_hits: u32,
    pub last_us: u64,
}

#[derive(Default)]
#[cfg(feature = "rhai-runtime")]
pub(super) struct SceneCache {
    scene: Option<(i32, MaterialFrame)>,
    pub stats: ScriptStats,
}

#[cfg(feature = "rhai-runtime")]
impl SceneCache {
    pub fn render(
        &mut self,
        engine: &Engine,
        scope: &mut Scope<'static>,
        ast: &AST,
        state: i32,
    ) -> Result<RuntimeFrame, RuntimeError> {
        if let Some((key, scene)) = self.scene {
            if key == state {
                self.stats.cache_hits = self.stats.cache_hits.saturating_add(1);
                return Ok(RuntimeFrame::Material(scene));
            }
        }
        let began = Instant::now().duration_since_epoch().as_micros() as u64;
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let raw = engine
            .call_fn_with_options::<ImmutableString>(options, scope, ast, "render", (state,))
            .map_err(|err| {
                println!("rhai render error: {:?}", err);
                RuntimeError::Script
            })?;
        self.stats.calls = self.stats.calls.saturating_add(1);
        self.stats.last_us =
            (Instant::now().duration_since_epoch().as_micros() as u64).saturating_sub(began);
        let frame = RuntimeFrame::parse(raw.as_str())?;
        // Legacy text can depend on arbitrary script data; only the Material
        // ABI has native live-field refresh and is safe to reuse this way.
        self.scene = match &frame {
            RuntimeFrame::Material(scene) => Some((state, *scene)),
            _ => None,
        };
        Ok(frame)
    }
}
