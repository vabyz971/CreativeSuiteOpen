// Cygnus — Suite créative professionnelle open source
// Copyright (C) 2026 vabyz971
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Pool rayon DÉDIÉ et PLAFONNÉ pour les calculs d'image lourds.
//!
//! Invariant RENDERING.md #4 : le compositing n'utilise JAMAIS le pool
//! rayon global — non plafonné, il saturerait tous les cœurs et gèlerait
//! le thread UI même quand le calcul tourne dans `spawn_blocking`.
//! [`run_parallel`] confine tous les `par_iter`/`par_bridge` imbriqués au
//! pool nommé ici (≤ 4 threads), laissant du CPU au reste du système.

use std::sync::OnceLock;

struct RenderExecutor {
    pool: rayon::ThreadPool,
}

impl RenderExecutor {
    fn new(pool: rayon::ThreadPool) -> Self {
        Self { pool }
    }
}

/// Pool borné unique du moteur. `None` seulement si l'OS refuse de créer
/// les threads (état pathologique du système) — on dégrade alors en
/// exécution inline plutôt que de faire tomber l'app.
static EXECUTOR: OnceLock<Option<RenderExecutor>> = OnceLock::new();

fn executor() -> &'static Option<RenderExecutor> {
    EXECUTOR.get_or_init(|| {
        // Bornage volontaire : au-delà de 4 threads le blend est memory-bound
        // et le gain est marginal — tandis que la contention avec l'UI grandit.
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(2, 4);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("creative-render-{i}"))
            .build()
            .map(RenderExecutor::new)
            .ok()
    })
}

/// Exécute `f` sur le pool de rendu dédié. Les appels `par_*` faits depuis
/// `f` (directement ou dans les sous-fonctions) s'exécutent sur CE pool,
/// pas sur le pool global. Si `f` est déjà sur le pool (install imbriquée),
/// rayon l'exécute directement — pas de risque de réentrance.
pub fn run_parallel<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    match executor().as_ref() {
        Some(ex) => ex.pool.install(f),
        None => f(),
    }
}
