// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2025 vabyz971
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

//! Nœud Calque : superpose un contenu (`top`) sur une base (`base`)
//! avec opacité et mode de fusion. C'est la brique du système de calques :
//! empilable en chaîne, chaque calque ajouté s'insère au-dessus du précédent.
//!
//! Le flag `enabled` du nœud sert de visibilité (bypass = calque masqué).

use super::{to_rgba8, Effect, NodeCtx};
use datatypes::{
    NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType,
};
use image::{DynamicImage, ImageBuffer, Rgba};
use rayon::prelude::*;

pub fn definition() -> NodeDefinition {
    NodeDefinition::new("layer", "Calque", NodeCategory::Compositing)
        .input(SocketDef::new("base", "Dessous", SocketType::Image))
        .input(SocketDef::new("top", "Dessus", SocketType::Image))
        .output(SocketDef::new("image", "Image", SocketType::Image))
        .param("opacity", ParamValue::Float(100.0))
        .param("blend_mode", ParamValue::Enum("Normal".into()))
        .param("offset_x", ParamValue::Float(0.0))
        .param("offset_y", ParamValue::Float(0.0))
        .header_color([0.45, 0.55, 0.85])
        .description("Calque — superpose son image (ou entrée top) sur la base")
}

/// Identifiant de socket d'entrée n° i du nœud Mélange (1-based)
pub fn mix_socket(i: usize) -> String {
    format!("image_{i}")
}

/// Nombre maximal d'entrées du Mélange
pub const MIX_MAX_INPUTS: usize = 6;

fn mode_id(mode: &str) -> u32 {
    match mode {
        "Multiply" => 1,
        "Screen" => 2,
        "Overlay" => 3,
        "Darken" => 4,
        "Lighten" => 5,
        _ => 0, // Normal
    }
}

/// Fusion d'un pixel : renvoie la couleur composée (top sur base)
#[inline]
fn blend_pixel(b: [f32; 4], t: [f32; 4], mode: u32) -> [f32; 4] {
    let blended = match mode {
        1 => [b[0] * t[0], b[1] * t[1], b[2] * t[2]],           // Multiply
        2 => [                                                  // Screen
            1.0 - (1.0 - b[0]) * (1.0 - t[0]),
            1.0 - (1.0 - b[1]) * (1.0 - t[1]),
            1.0 - (1.0 - b[2]) * (1.0 - t[2]),
        ],
        3 => [                                                  // Overlay
            if b[0] < 0.5 { 2.0 * b[0] * t[0] } else { 1.0 - 2.0 * (1.0 - b[0]) * (1.0 - t[0]) },
            if b[1] < 0.5 { 2.0 * b[1] * t[1] } else { 1.0 - 2.0 * (1.0 - b[1]) * (1.0 - t[1]) },
            if b[2] < 0.5 { 2.0 * b[2] * t[2] } else { 1.0 - 2.0 * (1.0 - b[2]) * (1.0 - t[2]) },
        ],
        4 => [b[0].min(t[0]), b[1].min(t[1]), b[2].min(t[2])],  // Darken
        5 => [b[0].max(t[0]), b[1].max(t[1]), b[2].max(t[2])],  // Lighten
        _ => [t[0], t[1], t[2]],                                // Normal
    };
    // Alpha compositing : top au-dessus de base, pondéré par l'alpha du top
    let ta = t[3];
    let a = ta + b[3] * (1.0 - ta);
    let mut out = [b[0], b[1], b[2], a];
    if a > 0.0001 {
        for c in 0..3 {
            out[c] = blended[c] * ta + b[c] * b[3] * (1.0 - ta);
            out[c] /= a;
        }
    }
    out
}

pub fn apply_effect(
    base: &DynamicImage,
    top: &DynamicImage,
    opacity: f32,
    blend_mode: &str,
    offset_x: f32,
    offset_y: f32,
) -> DynamicImage {    // Tenter le GPU (bandes, opérations ponctuelles exactes)
    if let Some(out) = crate::gpu::apply_blend_gpu(base, top, opacity, mode_id(blend_mode), offset_x, offset_y) {
        return out;
    }

    // Fallback CPU rayon — gère le décalage du calque
    let op = (opacity / 100.0).clamp(0.0, 1.0);
    let mode = mode_id(blend_mode);
    let b_img = to_rgba8(base);
    let t_img = to_rgba8(top);
    let (w, h) = (b_img.width(), b_img.height());
    let (tw, th) = (t_img.width() as i32, t_img.height() as i32);
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;

    let mut out: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
    let b_raw = b_img.as_raw();
    let t_raw = t_img.as_raw();
    out.as_flat_samples_mut()
        .samples
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, px)| {
            let x = (i as u32 % w) as i32;
            let y = (i as u32 / w) as i32;
            let b_off = i * 4;
            let b: [f32; 4] = [
                b_raw[b_off] as f32 / 255.0,
                b_raw[b_off + 1] as f32 / 255.0,
                b_raw[b_off + 2] as f32 / 255.0,
                b_raw[b_off + 3] as f32 / 255.0,
            ];
            // Échantillonne le dessus à (x - ox, y - oy) — transparent hors bornes
            let tx = x - ox;
            let ty = y - oy;
            let mut t: [f32; 4] = if tx >= 0 && tx < tw && ty >= 0 && ty < th {
                let t_off = (ty as u32 * tw as u32 + tx as u32) as usize * 4;
                [
                    t_raw[t_off] as f32 / 255.0,
                    t_raw[t_off + 1] as f32 / 255.0,
                    t_raw[t_off + 2] as f32 / 255.0,
                    t_raw[t_off + 3] as f32 / 255.0,
                ]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            };
            t[3] *= op;
            let o = if t[3] <= 0.001 { b } else { blend_pixel(b, t, mode) };
            // Si le calque est transparent à ce pixel, on garde la base
            let is_transparent = t[3] <= 0.001;
            let out_px = if is_transparent { b } else { o };
            px[0] = (out_px[0].clamp(0.0, 1.0) * 255.0) as u8;
            px[1] = (out_px[1].clamp(0.0, 1.0) * 255.0) as u8;
            px[2] = (out_px[2].clamp(0.0, 1.0) * 255.0) as u8;
            px[3] = (out_px[3].clamp(0.0, 1.0) * 255.0) as u8;
        });
    DynamicImage::ImageRgba8(out)
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    // Le dessus peut venir soit d'une connexion `top`, soit de l'image stockée du calque lui-même.
    let top_input = ctx.input(id, "top");
    let top_stored = ctx.sources.get(&id).map(|a| a.as_ref() as &DynamicImage);
    let top = top_input.or(top_stored);
    let base = ctx.input(id, "base");
    match (base, top) {
        (Some(b), Some(t)) => {
            let opacity = ctx.param(id, "opacity", 100.0);
            let ox = ctx.param(id, "offset_x", 0.0);
            let oy = ctx.param(id, "offset_y", 0.0);
            let blend_mode = ctx
                .graph
                .get(id)
                .and_then(|n| n.params.get("blend_mode"))
                .and_then(|v| v.as_enum())
                .unwrap_or("Normal")
                .to_string();
            Some(apply_effect(b, t, opacity, &blend_mode, ox, oy))
        }
        (Some(b), None) => Some(b.clone()),
        (None, Some(t)) => {
            // Calque isolé (pas de base) : on applique quand même opacité et décalage
            let opacity = ctx.param(id, "opacity", 100.0);
            let ox = ctx.param(id, "offset_x", 0.0);
            let oy = ctx.param(id, "offset_y", 0.0);
            if ox == 0.0 && oy == 0.0 && (opacity - 100.0).abs() < 0.01 {
                Some(t.clone())
            } else {
                // Fond transparent de même taille que le dessus, puis fusion
                let (w, h) = (t.width(), t.height());
                let transparent = DynamicImage::ImageRgba8(
                    ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0])),
                );
                let blend_mode = ctx
                    .graph
                    .get(id)
                    .and_then(|n| n.params.get("blend_mode"))
                    .and_then(|v| v.as_enum())
                    .unwrap_or("Normal")
                    .to_string();
                Some(apply_effect(&transparent, t, opacity, &blend_mode, ox, oy))
            }
        }
        (None, None) => None,
    }
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
