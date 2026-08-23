//! Nœud source : fournit l'image originale au graphe

use super::{Effect, NodeCtx};
use datatypes::{NodeCategory, NodeDefinition, NodeId, SocketDef, SocketType};
use image::DynamicImage;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new("input_image", "Image Source", NodeCategory::Input)
        .output(SocketDef::new("image", "Image", SocketType::Image))
        .header_color([0.25, 0.45, 0.75])
        .description("Source d'image")
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    if let Some(arc) = ctx.sources.get(&id) {
        return Some(arc.as_ref().clone());
    }
    // Pas d'image assignée : transparent aux dimensions de l'original (évite de réafficher la 1ère image)
    let (w, h) = (ctx.original.width().max(1), ctx.original.height().max(1));
    Some(DynamicImage::ImageRgba8(
        image::ImageBuffer::from_pixel(w, h, image::Rgba([0, 0, 0, 0])),
    ))
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
