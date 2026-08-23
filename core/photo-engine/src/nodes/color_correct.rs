//! Effet Correction Couleur (saturation HSL simplifiée)

use super::{to_rgba8, Effect, NodeCtx};
use datatypes::{NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType};
use image::DynamicImage;
use rayon::prelude::*;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new("color_correct", "Correction Couleur", NodeCategory::Color)
        .input(SocketDef::new("image", "Image", SocketType::Image))
        .output(SocketDef::new("image", "Image", SocketType::Image))
        .param("saturation", ParamValue::Float(1.0))
        .param("hue", ParamValue::Float(0.0))
        .header_color([0.85, 0.55, 0.10])
        .description("Correction HSL")
}

pub fn apply_effect(img: &DynamicImage, saturation: f32) -> DynamicImage {
    if let Some(gpu_out) = super::apply_saturation_gpu(img, saturation) {
        return gpu_out;
    }
    let s = saturation.clamp(0.0, 3.0);
    if (s - 1.0).abs() < 0.01 {
        return img.clone();
    }
    let mut out = to_rgba8(img);
    out.as_flat_samples_mut().samples.par_chunks_mut(4).for_each(|px| {
        let r = px[0] as f32;
        let g = px[1] as f32;
        let b = px[2] as f32;
        let gray = 0.299 * r + 0.587 * g + 0.114 * b;
        px[0] = (gray + (r - gray) * s).clamp(0.0f32, 255.0f32) as u8;
        px[1] = (gray + (g - gray) * s).clamp(0.0f32, 255.0f32) as u8;
        px[2] = (gray + (b - gray) * s).clamp(0.0f32, 255.0f32) as u8;
    });
    DynamicImage::ImageRgba8(out)
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    let input = ctx.input(id, "image")?;
    let s = ctx.param(id, "saturation", 1.0);
    Some(apply_effect(input, s))
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
