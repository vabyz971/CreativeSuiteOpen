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

//! Registre des nodes Photo — délègue aux effets du dossier `nodes/`.
//! Ajouter un effet : créer `nodes/mon_effet.rs` puis l'ajouter à `nodes::all()`.

use crate::nodes;
use suite_core::{Graph, Node};
use datatypes::{NodeDefinition, NodeId, SocketType, Vec2};

pub fn all_definitions() -> Vec<NodeDefinition> {
    nodes::all().into_iter().map(|e| e.definition).collect()
}

pub fn definition_for(type_id: &str) -> Option<NodeDefinition> {
    all_definitions().into_iter().find(|d| d.type_id == type_id)
}

/// Crée un nœud vide (graphe sans nœuds)
pub fn create_empty_graph() -> Graph {
    Graph::new()
}

/// Crée le graphe minimal Input -> Output utilisé à l'ouverture d'une image
pub fn create_minimal_graph() -> Graph {
    let mut g = Graph::new();
    let input = g.add_node(node_from_def("input_image", NodeId(0), Vec2::new(40.0, 120.0)));
    let output = g.add_node(Node {
        params: Default::default(),
        preview_enabled: true,
        ..node_from_def("output", NodeId(0), Vec2::new(400.0, 120.0))
    });
    let _ = g.connect(suite_core::Connection::new(
        input, "image", output, "image", SocketType::Image,
    ));
    g
}

fn node_from_def(type_id: &str, id: NodeId, pos: Vec2) -> Node {
    let def = definition_for(type_id).expect("type inconnu");
    let mut node = Node::new(id, def.type_id.clone(), def.name.clone(), pos);
    node.params = def.default_params.clone();
    node
}

pub fn create_node_for_type(type_id: &str, pos: Vec2, graph: &mut Graph) -> Option<NodeId> {
    let def = definition_for(type_id)?;
    let mut node = Node::new(NodeId(0), def.type_id.clone(), def.name.clone(), pos);
    node.params = def.default_params.clone();
    Some(graph.add_node(node))
}
