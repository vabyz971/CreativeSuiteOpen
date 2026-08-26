// CreativeSuiteOpen — Suite créative professionnelle open source
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
