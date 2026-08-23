//! Nœuds d'effets Photo — un fichier par effet.
//! Chaque effet expose :
//!  - `definition()` : sa définition (sockets + params) pour le registre UI
//!  - `apply(ctx, id)` : son évaluation image -> image
//! Ajouter un nouvel effet = créer un fichier ici + l'enregistrer dans `all()`.

pub mod blur;
pub mod brightness_contrast;
pub mod color_correct;
pub mod input;
pub mod layer;
pub mod mix;
pub mod output;

use crate::gpu;
use datatypes::{NodeDefinition, NodeId};
use image::DynamicImage;
use suite_core::Graph;
use std::collections::HashMap;
use std::sync::Arc;

/// Contexte passé à chaque effet lors de l'évaluation du graphe
pub struct NodeCtx<'a> {
    pub graph: &'a Graph,
    /// Cache des sorties déjà calculées (topologique : les inputs sont prêts)
    pub cache: &'a HashMap<NodeId, DynamicImage>,
    /// Image source (entrée du graphe) — fallback si pas de source dédiée
    pub original: &'a DynamicImage,
    /// Images PAR NŒUD d'entrée (multi-calques : chaque input_image/empty_layer
    /// a sa propre image, Arc = partage bon marché avec le worker)
    pub sources: &'a HashMap<NodeId, Arc<DynamicImage>>,
}

impl NodeCtx<'_> {
    /// Image arrivant sur un socket d'entrée donné
    pub fn input(&self, node: NodeId, socket: &str) -> Option<&DynamicImage> {
        let conn = self
            .graph
            .connections
            .iter()
            .find(|c| c.to_node == node && c.to_socket == socket)?;
        self.cache.get(&conn.from_node)
    }

    /// Première image trouvée sur n'importe quel socket (utilisé pour le bypass)
    pub fn any_input(&self, node: NodeId) -> Option<&DynamicImage> {
        for conn in self.graph.connections.iter().filter(|c| c.to_node == node) {
            if let Some(img) = self.cache.get(&conn.from_node) {
                return Some(img);
            }
        }
        None
    }

    /// Valeur float d'un paramètre avec défaut
    pub fn param(&self, node_id: NodeId, key: &str, default: f32) -> f32 {
        self.graph
            .get(node_id)
            .and_then(|n| n.params.get(key))
            .and_then(|v| v.as_float())
            .unwrap_or(default)
    }
}

/// Un effet enregistré : définition UI + fonction d'évaluation
pub struct Effect {
    pub definition: NodeDefinition,
    pub apply: fn(&NodeCtx, NodeId) -> Option<DynamicImage>,
}

/// Tous les effets du moteur — point d'entrée unique pour registre et processeur.
pub fn all() -> Vec<Effect> {
    vec![
        input::effect(),
        output::effect(),
        brightness_contrast::effect(),
        blur::effect(),
        mix::effect(),
        layer::effect(),
        color_correct::effect(),
    ]
}

/// Retrouve un effet par type_id
pub fn find(type_id: &str) -> Option<Effect> {
    all().into_iter().find(|e| e.definition.type_id == type_id)
}

// ---------------------------------------------------------------------------
// Helpers partagés par les effets CPU/GPU
// ---------------------------------------------------------------------------

/// Convertit une DynamicImage en RGBA8 brut
pub fn to_rgba8(img: &DynamicImage) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    img.to_rgba8()
}

pub use gpu::{
    apply_blur_gpu, apply_brightness_contrast_gpu, apply_mix_gpu, apply_saturation_gpu,
};
