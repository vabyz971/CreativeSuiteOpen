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

//! Nœud Mélange : superpose de 2 à 6 images (entrées dynamiques) dans l'ordre
//! des slots — image 1 en dessous, la dernière au-dessus. Chaque entrée
//! non connectée est ignorée. C'est l'outil pour empiler plus de deux calques.
//!
//! Réutilise la fusion du Calque (modes + alpha compositing).

use super::layer::{apply_effect, mix_socket, MIX_MAX_INPUTS};
use super::{Effect, NodeCtx};
use datatypes::{
    NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType,
};
use image::DynamicImage;

pub fn definition() -> NodeDefinition {
    let mut def = NodeDefinition::new("mix", "Mélange", NodeCategory::Compositing)
        .param("count", ParamValue::Int(2))
        .param("blend_mode", ParamValue::Enum("Normal".into()))
        .header_color([0.45, 0.35, 0.65])
        .description("Superpose 2 à 6 images (ajouter/retirer des entrées)");
    for i in 1..=MIX_MAX_INPUTS {
        def = def.input(SocketDef::new(
            mix_socket(i),
            format!("Image {i}"),
            SocketType::Image,
        ));
    }
    def.output(SocketDef::new("image", "Image", SocketType::Image))
}

fn count_of(ctx: &NodeCtx, id: NodeId) -> usize {
    let n = ctx
        .graph
        .get(id)
        .and_then(|node| node.params.get("count"))
        .and_then(|v| match v {
            ParamValue::Int(i) => Some(*i as usize),
            ParamValue::Float(f) => Some(*f as usize),
            _ => None,
        })
        .unwrap_or(2);
    n.clamp(2, MIX_MAX_INPUTS)
}

fn apply(ctx: &NodeCtx, id: NodeId) -> Option<DynamicImage> {
    let count = count_of(ctx, id);
    let blend_mode = ctx
        .graph
        .get(id)
        .and_then(|n| n.params.get("blend_mode"))
        .and_then(|v| v.as_enum())
        .unwrap_or("Normal")
        .to_string();

    // Composite séquentiel : image 1 en dessous, chaque image suivante par-dessus.
    // Les slots non connectés sont sautés.
    let mut result: Option<DynamicImage> = None;
    for i in 1..=count {
        if let Some(img) = ctx.input(id, &mix_socket(i)) {
            result = match result {
                None => Some(img.clone()),
                Some(prev) => Some(apply_effect(&prev, img, 100.0, &blend_mode, 0.0, 0.0)),
            };
        }
    }
    result
}

pub fn effect() -> Effect {
    Effect {
        definition: definition(),
        apply,
    }
}
