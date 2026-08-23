//! Nœud de sortie : transmet l'image finale au canvas

use super::{Effect, NodeCtx};
use datatypes::{NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType};
use image::DynamicImage;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new("output", "Sortie", NodeCategory::Output)
        .input(SocketDef::new("image", "Image", SocketType::Image))
        .param("width", ParamValue::Int(1920))
        .param("height", ParamValue::Int(1080))
        .header_color([0.65, 0.20, 0.20])
        .description("Sortie finale — rognée au gizmo (dimensions choisies)")
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    // Pour l'affichage, la sortie transmet l'image telle quelle ;
    // les dimensions (gizmo) ne servent qu'à l'export et à l'overlay visuel.
    // Le rognage à l'export se fera au moment de l'export, pas ici.
    ctx.input(id, "image").cloned()
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
