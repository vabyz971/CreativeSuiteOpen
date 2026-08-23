//! Effet Flou gaussien (GPU compute + fallback CPU `image::blur`)

use super::{Effect, NodeCtx};
use datatypes::{NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType};
use image::DynamicImage;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new("blur", "Flou", NodeCategory::Filter)
        .input(SocketDef::new("image", "Image", SocketType::Image))
        .output(SocketDef::new("image", "Image", SocketType::Image))
        .param("radius", ParamValue::Float(5.0))
        .param("type", ParamValue::Enum("Gaussian".into()))
        .header_color([0.20, 0.55, 0.75])
        .description("Flou gaussien")
}

pub fn apply_effect(img: &DynamicImage, radius: f32) -> DynamicImage {
    if radius <= 0.1 {
        return img.clone();
    }
    if let Some(gpu_out) = super::apply_blur_gpu(img, radius) {
        return gpu_out;
    }
    img.blur(radius)
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    let input = ctx.input(id, "image")?;
    let r = ctx.param(id, "radius", 5.0);
    Some(apply_effect(input, r))
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
