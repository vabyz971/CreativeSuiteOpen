# engines/photo-engine/src/render_pool.rs

- RenderExecutor · struct · L27-L29 — struct RenderExecutor
- new · function · L32-L34 — fn new(pool: rayon::ThreadPool) -> Self
- EXECUTOR · constant · L40-L40 — static EXECUTOR: OnceLock<Option<RenderExecutor>> = OnceLock::new();
- executor · function · L42-L57 — fn executor() -> &'static Option<RenderExecutor>
- run_parallel · function · L63-L68 — pub fn run_parallel<R: Send>(f: impl FnOnce() -> R + Send) -> R
