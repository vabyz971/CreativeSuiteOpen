// Engines extraits vers core/photo-engine (modulaire) — ces modules restent pour compatibilité
// et délèguent désormais à photo_engine. Voir core/photo-engine/src/lib.rs
pub mod gpu {
    pub use photo_engine::gpu::*;
}
pub mod node_registry {
    pub use photo_engine::registry::*;
}
pub mod layers_panel;
pub mod options;
pub mod properties;
pub mod toolpanel;
pub mod toolbar;
pub mod workspace;
