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

//! Moteur de graphe nodal générique pour CreativeSuiteOpen
//! Utilisé par Photo, Vector, Video...

use datatypes::{DataValue, NodeId, ParamValue, SocketType, Vec2};
use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Node instance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub type_id: String,
    pub name: String,
    pub position: Vec2,
    pub params: HashMap<String, ParamValue>,
    pub preview_enabled: bool,
    /// Nœud actif : si false, l'effet est bypassé (le flux passe à travers)
    pub enabled: bool,
}

impl Node {
    pub fn new(id: NodeId, type_id: impl Into<String>, name: impl Into<String>, pos: Vec2) -> Self {
        Self {
            id,
            type_id: type_id.into(),
            name: name.into(),
            position: pos,
            params: HashMap::new(),
            preview_enabled: false,
            enabled: true,
        }
    }

    pub fn param_float(&self, key: &str, default: f32) -> f32 {
        self.params
            .get(key)
            .and_then(|v| v.as_float())
            .unwrap_or(default)
    }
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_socket: String,
    pub to_node: NodeId,
    pub to_socket: String,
    pub socket_type: SocketType,
}

impl Connection {
    pub fn new(
        from_node: NodeId,
        from_socket: impl Into<String>,
        to_node: NodeId,
        to_socket: impl Into<String>,
        ty: SocketType,
    ) -> Self {
        Self {
            from_node,
            from_socket: from_socket.into(),
            to_node,
            to_socket: to_socket.into(),
            socket_type: ty,
        }
    }

    pub fn create(
        from_node: NodeId,
        from_socket: impl Into<String>,
        to_node: NodeId,
        to_socket: impl Into<String>,
        ty: SocketType,
    ) -> Self {
        Self {
            from_node,
            from_socket: from_socket.into(),
            to_node,
            to_socket: to_socket.into(),
            socket_type: ty,
        }
    }
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: HashMap<NodeId, Node>,
    pub connections: Vec<Connection>,
    next_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    NodeNotFound(NodeId),
    SocketNotFound(String),
    TypeMismatch {
        from: SocketType,
        to: SocketType,
    },
    CycleDetected,
    InputAlreadyConnected {
        node: NodeId,
        socket: String,
    },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::NodeNotFound(id) => write!(f, "Node {id} introuvable"),
            GraphError::SocketNotFound(s) => write!(f, "Socket {s} introuvable"),
            GraphError::TypeMismatch { from, to } => {
                write!(f, "Type mismatch {:?} -> {:?}", from, to)
            }
            GraphError::CycleDetected => write!(f, "Cycle détecté dans le graphe"),
            GraphError::InputAlreadyConnected { node, socket } => {
                write!(f, "Input déjà connecté {node}:{socket}")
            }
        }
    }
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_node(&mut self, mut node: Node) -> NodeId {
        if node.id.0 == 0 {
            node.id = self.alloc_id();
        } else {
            self.next_id = self.next_id.max(node.id.0 + 1);
        }
        let id = node.id;
        self.nodes.insert(id, node);
        id
    }

    pub fn remove_node(&mut self, id: NodeId) -> Option<Node> {
        self.connections
            .retain(|c| c.from_node != id && c.to_node != id);
        self.nodes.remove(&id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn move_node(&mut self, id: NodeId, pos: Vec2) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.position = pos;
        }
    }

    pub fn update_param(&mut self, id: NodeId, key: impl Into<String>, value: ParamValue) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.params.insert(key.into(), value);
        }
    }

    pub fn connect(&mut self, conn: Connection) -> Result<(), GraphError> {
        if !self.nodes.contains_key(&conn.from_node) {
            return Err(GraphError::NodeNotFound(conn.from_node));
        }
        if !self.nodes.contains_key(&conn.to_node) {
            return Err(GraphError::NodeNotFound(conn.to_node));
        }
        // Vérifie input pas déjà connecté (single input)
        if self
            .connections
            .iter()
            .any(|c| c.to_node == conn.to_node && c.to_socket == conn.to_socket)
        {
            return Err(GraphError::InputAlreadyConnected {
                node: conn.to_node,
                socket: conn.to_socket.clone(),
            });
        }
        // Cycle check via topo sort après insertion tentative
        self.connections.push(conn);
        if self.topological_order().is_err() {
            self.connections.pop();
            return Err(GraphError::CycleDetected);
        }
        Ok(())
    }

    pub fn disconnect_input(&mut self, to_node: NodeId, to_socket: &str) {
        self.connections
            .retain(|c| !(c.to_node == to_node && c.to_socket == to_socket));
    }

    pub fn disconnect(&mut self, conn: &Connection) {
        self.connections.retain(|c| c != conn);
    }

    pub fn connections_from(&self, node: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.from_node == node)
            .collect()
    }

    pub fn connections_to(&self, node: NodeId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.to_node == node)
            .collect()
    }

    /// Tri topologique (Kahn). Erreur si cycle.
    pub fn topological_order(&self) -> Result<Vec<NodeId>, GraphError> {
        let mut indeg: HashMap<NodeId, usize> =
            self.nodes.keys().map(|id| (*id, 0)).collect();
        for c in &self.connections {
            *indeg.entry(c.to_node).or_insert(0) += 1;
        }
        let mut q: VecDeque<NodeId> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::new();
        let mut indeg_mut = indeg;

        while let Some(n) = q.pop_front() {
            order.push(n);
            for c in self.connections.iter().filter(|c| c.from_node == n) {
                let e = indeg_mut.get_mut(&c.to_node).unwrap();
                *e -= 1;
                if *e == 0 {
                    q.push_back(c.to_node);
                }
            }
        }

        if order.len() != self.nodes.len() {
            Err(GraphError::CycleDetected)
        } else {
            Ok(order)
        }
    }

    /// Évaluation mock : parcourt en topo, produit DataValue factices.
    /// Remplacera par vrai moteur wgpu/image plus tard.
    pub fn evaluate(&self) -> Result<HashMap<NodeId, DataValue>, GraphError> {
        let order = self.topological_order()?;
        let mut results: HashMap<NodeId, DataValue> = HashMap::new();

        for id in order {
            let node = &self.nodes[&id];
            // Pour l'instant on simule : Output -> Image 1920x1080, autres -> Float
            let val = match node.type_id.as_str() {
                "output" => DataValue::Image(datatypes::ImageMeta {
                    width: 1920,
                    height: 1080,
                    path: None,
                }),
                "input_image" => DataValue::Image(datatypes::ImageMeta {
                    width: 1920,
                    height: 1080,
                    path: Some("input.jpg".into()),
                }),
                _ => {
                    // Si connecté en entrée Image, propage Image
                    let has_image_input = self
                        .connections
                        .iter()
                        .any(|c| c.to_node == id && c.socket_type == SocketType::Image);
                    if has_image_input {
                        DataValue::Image(datatypes::ImageMeta {
                            width: 1920,
                            height: 1080,
                            path: None,
                        })
                    } else {
                        DataValue::Float(node.param_float("value", 0.0))
                    }
                }
            };
            results.insert(id, val);
        }
        Ok(results)
    }

    pub fn find_output_node(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|(_, n)| n.type_id == "output")
            .map(|(id, _)| *id)
    }

    /// Vérifie si le node est connecté (transitivement) à l'Output - évite recalcul si déconnecté
    pub fn is_connected_to_output(&self, node_id: NodeId) -> bool {
        let Some(output) = self.find_output_node() else {
            return false;
        };
        if node_id == output {
            return true;
        }
        // BFS inverse depuis Output en suivant les connexions à l'envers
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![output];
        visited.insert(output);
        while let Some(current) = stack.pop() {
            for conn in self.connections.iter().filter(|c| c.to_node == current) {
                let from = conn.from_node;
                if from == node_id {
                    return true;
                }
                if visited.insert(from) {
                    stack.push(from);
                }
            }
        }
        false
    }

    /// Retourne tous les ancêtres de l'Output (nœuds qui influencent le rendu final)
    pub fn output_ancestors(&self) -> std::collections::HashSet<NodeId> {
        let mut ancestors = std::collections::HashSet::new();
        let Some(output) = self.find_output_node() else {
            return ancestors;
        };
        let mut stack = vec![output];
        ancestors.insert(output);
        while let Some(current) = stack.pop() {
            for conn in self.connections.iter().filter(|c| c.to_node == current) {
                if ancestors.insert(conn.from_node) {
                    stack.push(conn.from_node);
                }
            }
        }
        ancestors
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Vérifie si une connexion est valide (types compatibles)
    pub fn can_connect(&self, from_ty: SocketType, to_ty: SocketType) -> bool {
        from_ty == to_ty || (from_ty == SocketType::Float && to_ty == SocketType::Float)
    }

    /// Retourne tous les descendants (enfants, récursif) d'un nœud, inclusif
    pub fn downstream_nodes(&self, start: NodeId) -> std::collections::HashSet<NodeId> {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![start];
        visited.insert(start);
        while let Some(cur) = stack.pop() {
            for conn in self.connections.iter().filter(|c| c.from_node == cur) {
                if visited.insert(conn.to_node) {
                    stack.push(conn.to_node);
                }
            }
        }
        visited
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use datatypes::Vec2;

    fn make_graph() -> Graph {
        let mut g = Graph::new();
        let a = g.add_node(Node::new(NodeId(0), "input_image", "Input", Vec2::new(0.0, 0.0)));
        let b = g.add_node(Node::new(
            NodeId(0),
            "brightness_contrast",
            "BC",
            Vec2::new(200.0, 0.0),
        ));
        let c = g.add_node(Node::new(NodeId(0), "output", "Output", Vec2::new(400.0, 0.0)));
        g.connect(Connection::create(a, "image", b, "image", SocketType::Image))
            .unwrap();
        g.connect(Connection::create(b, "image", c, "image", SocketType::Image))
            .unwrap();
        g
    }

    #[test]
    fn topo_order_ok() {
        let g = make_graph();
        let order = g.topological_order().unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn cycle_rejected() {
        let mut g = make_graph();
        // Trouve explicitement l'input (indeg 0) et l'output (dernier)
        let input_id = g
            .nodes
            .iter()
            .find(|(_, n)| n.type_id == "input_image")
            .map(|(id, _)| *id)
            .unwrap();
        let last = *g.topological_order().unwrap().last().unwrap();
        // Crée un cycle output -> input en utilisant un socket libre
        g.disconnect_input(input_id, "image");
        let res = g.connect(Connection::create(
            last,
            "image",
            input_id,
            "image",
            SocketType::Image,
        ));
        assert_eq!(res, Err(GraphError::CycleDetected));
    }

    #[test]
    fn evaluate_produces_image() {
        let g = make_graph();
        let res = g.evaluate().unwrap();
        assert_eq!(res.len(), 3);
    }

    #[test]
    fn input_already_connected() {
        let mut g = make_graph();
        let extra = g.add_node(Node::new(NodeId(0), "blur", "Blur", Vec2::new(200.0, 100.0)));
        let target = g
            .nodes
            .iter()
            .find(|(_, n)| n.type_id == "brightness_contrast")
            .map(|(id, _)| *id)
            .unwrap();
        let res = g.connect(Connection::create(
            extra,
            "image",
            target,
            "image",
            SocketType::Image,
        ));
        assert!(matches!(res, Err(GraphError::InputAlreadyConnected { .. })));
    }
}
