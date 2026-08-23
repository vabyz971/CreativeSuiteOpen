//! Effet Luminosité / Contraste (GPU compute + fallback CPU rayon)

use super::{to_rgba8, Effect, NodeCtx};
use datatypes::{NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType};
use image::DynamicImage;
use rayon::prelude::*;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new(
        "brightness_contrast",
        "Luminosité / Contraste",
        NodeCategory::Color,
    )
    .input(SocketDef::new("image", "Image", SocketType::Image))
    .output(SocketDef::new("image", "Image", SocketType::Image))
    .param("brightness", ParamValue::Float(0.0))
    .param("contrast", ParamValue::Float(0.0))
    .header_color([0.75, 0.55, 0.15])
    .description("Ajuste luminosité et contraste")
}

pub fn apply_effect(img: &DynamicImage, brightness: f32, contrast: f32) -> DynamicImage {
    if let Some(gpu_out) =
        super::apply_brightness_contrast_gpu(img, brightness, contrast)
    {
        return gpu_out;
    }
    let b = (brightness * 2.55) as i32;
    let contrast_factor = if contrast < 0.0 {
        1.0 + contrast / 100.0
    } else {
        1.0 + contrast / 50.0
    };
    let mut out = to_rgba8(img);
    out.as_flat_samples_mut().samples.par_chunks_mut(4).for_each(|px| {
        for c in 0..3 {
            let mut v = px[c] as f32;
            v = (v - 128.0) * contrast_factor + 128.0;
            v += b as f32;
            px[c] = v.clamp(0.0f32, 255.0f32) as u8;
        }
    });
    DynamicImage::ImageRgba8(out)
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    let input = ctx.input(id, "image")?;
    let b = ctx.param(id, "brightness", 0.0);
    let c = ctx.param(id, "contrast", 0.0);
    Some(apply_effect(input, b, c))
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
