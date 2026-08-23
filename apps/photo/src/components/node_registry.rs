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

//! Registre des nodes Photo - définitions + helpers de création

use suite_core::{Graph, Node};
use datatypes::{NodeCategory, NodeDefinition, NodeId, ParamValue, SocketDef, SocketType, Vec2};

// ---------------------------------------------------------------------------
// Définitions
// ---------------------------------------------------------------------------

pub fn all_definitions() -> Vec<NodeDefinition> {
    vec![
        NodeDefinition::new("input_image", "Image Source", NodeCategory::Input)
            .output(SocketDef::new("image", "Image", SocketType::Image))
            .header_color([0.25, 0.45, 0.75])
            .description("Source d'image"),
        NodeDefinition::new("output", "Sortie", NodeCategory::Output)
            .input(SocketDef::new("image", "Image", SocketType::Image))
            .header_color([0.65, 0.20, 0.20])
            .description("Sortie finale vers canvas"),
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
        .description("Ajuste luminosité et contraste"),
        NodeDefinition::new("blur", "Flou", NodeCategory::Filter)
            .input(SocketDef::new("image", "Image", SocketType::Image))
            .output(SocketDef::new("image", "Image", SocketType::Image))
            .param("radius", ParamValue::Float(5.0))
            .param("type", ParamValue::Enum("Gaussian".into()))
            .header_color([0.20, 0.55, 0.75])
            .description("Flou gaussien"),
        NodeDefinition::new("mix", "Mélange", NodeCategory::Compositing)
            .input(SocketDef::new("image_a", "Image A", SocketType::Image))
            .input(SocketDef::new("image_b", "Image B", SocketType::Image))
            .input(
                SocketDef::new("factor", "Facteur", SocketType::Float)
                    .with_default(datatypes::DataValue::Float(0.5)),
            )
            .output(SocketDef::new("image", "Image", SocketType::Image))
            .param("blend_mode", ParamValue::Enum("Mix".into()))
            .param("factor", ParamValue::Float(0.5))
            .header_color([0.45, 0.35, 0.65])
            .description("Mélange deux images"),
        NodeDefinition::new("color_correct", "Correction Couleur", NodeCategory::Color)
            .input(SocketDef::new("image", "Image", SocketType::Image))
            .output(SocketDef::new("image", "Image", SocketType::Image))
            .param("saturation", ParamValue::Float(1.0))
            .param("hue", ParamValue::Float(0.0))
            .header_color([0.85, 0.55, 0.10])
            .description("Correction HSL"),
    ]
}

pub fn definition_for(type_id: &str) -> Option<NodeDefinition> {
    all_definitions().into_iter().find(|d| d.type_id == type_id)
}

// ---------------------------------------------------------------------------
// Graphe de démo
// ---------------------------------------------------------------------------

pub fn create_demo_graph() -> Graph {
    let mut g = Graph::new();

    let input = g.add_node(Node {
        id: NodeId(0),
        type_id: "input_image".into(),
        name: "Image Source".into(),
        position: Vec2::new(40.0, 120.0),
        params: Default::default(),
        preview_enabled: false,
    });
    // Inject default params from definition
    if let Some(def) = definition_for("input_image") {
        if let Some(n) = g.get_mut(input) {
            n.params = def.default_params;
        }
    }

    let bc = g.add_node(Node {
        id: NodeId(0),
        type_id: "brightness_contrast".into(),
        name: "Luminosité / Contraste".into(),
        position: Vec2::new(320.0, 80.0),
        params: Default::default(),
        preview_enabled: false,
    });
    if let Some(def) = definition_for("brightness_contrast") {
        if let Some(n) = g.get_mut(bc) {
            n.params = def.default_params;
        }
    }

    let blur = g.add_node(Node {
        id: NodeId(0),
        type_id: "blur".into(),
        name: "Flou".into(),
        position: Vec2::new(320.0, 260.0),
        params: Default::default(),
        preview_enabled: false,
    });
    if let Some(def) = definition_for("blur") {
        if let Some(n) = g.get_mut(blur) {
            n.params = def.default_params;
        }
    }

    let mix = g.add_node(Node {
        id: NodeId(0),
        type_id: "mix".into(),
        name: "Mélange".into(),
        position: Vec2::new(600.0, 160.0),
        params: Default::default(),
        preview_enabled: false,
    });
    if let Some(def) = definition_for("mix") {
        if let Some(n) = g.get_mut(mix) {
            n.params = def.default_params;
        }
    }

    let output = g.add_node(Node {
        id: NodeId(0),
        type_id: "output".into(),
        name: "Sortie".into(),
        position: Vec2::new(880.0, 160.0),
        params: Default::default(),
        preview_enabled: true,
    });

    // Connexions : input -> bc -> mix.A, input -> blur -> mix.B, mix -> output
    let _ = g.connect(suite_core::Connection::new(
        input,
        "image",
        bc,
        "image",
        SocketType::Image,
    ));
    let _ = g.connect(suite_core::Connection::new(
        input,
        "image",
        blur,
        "image",
        SocketType::Image,
    ));
    let _ = g.connect(suite_core::Connection::new(
        bc,
        "image",
        mix,
        "image_a",
        SocketType::Image,
    ));
    let _ = g.connect(suite_core::Connection::new(
        blur,
        "image",
        mix,
        "image_b",
        SocketType::Image,
    ));
    let _ = g.connect(suite_core::Connection::new(
        mix,
        "image",
        output,
        "image",
        SocketType::Image,
    ));

    g
}

pub fn create_node_for_type(type_id: &str, pos: Vec2, graph: &mut Graph) -> Option<NodeId> {
    let def = definition_for(type_id)?;
    let mut node = Node::new(NodeId(0), def.type_id.clone(), def.name.clone(), pos);
    node.params = def.default_params.clone();
    Some(graph.add_node(node))
}

pub fn create_minimal_graph() -> Graph {
    let mut g = Graph::new();
    let input = g.add_node(Node {
        id: NodeId(0),
        type_id: "input_image".into(),
        name: "Image Source".into(),
        position: Vec2::new(40.0, 120.0),
        params: definition_for("input_image").map(|d| d.default_params).unwrap_or_default(),
        preview_enabled: false,
    });
    let output = g.add_node(Node {
        id: NodeId(0),
        type_id: "output".into(),
        name: "Sortie".into(),
        position: Vec2::new(400.0, 120.0),
        params: Default::default(),
        preview_enabled: true,
    });
    let _ = g.connect(suite_core::Connection::new(input, "image", output, "image", SocketType::Image));
    g
}
