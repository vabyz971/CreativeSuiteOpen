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

//! Shared datatypes for `CreativeSuiteOpen`
//! Defines generic node bricks used by Photo, Vector, Video...

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "n{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketId(pub u32);

// ---------------------------------------------------------------------------
// Socket types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketType {
    Image,
    Float,
    Color,
    Vector,
    Bool,
}

impl SocketType {
    #[must_use]
    pub fn color(self) -> [f32; 3] {
        match self {
            SocketType::Image => [0.65, 0.45, 0.95],  // violet
            SocketType::Float => [0.55, 0.55, 0.55],  // gris
            SocketType::Color => [0.95, 0.85, 0.25],  // jaune
            SocketType::Vector => [0.30, 0.60, 0.95], // bleu
            SocketType::Bool => [0.85, 0.35, 0.35],   // rouge
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SocketType::Image => "Image",
            SocketType::Float => "Float",
            SocketType::Color => "Color",
            SocketType::Vector => "Vector",
            SocketType::Bool => "Bool",
        }
    }
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub enum DataValue {
    #[default]
    None,
    Float(f32),
    Color([f32; 4]),
    Vector([f32; 3]),
    Bool(bool),
    Image(ImageMeta),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
    pub path: Option<String>,
}

impl DataValue {
    #[must_use]
    pub fn socket_type(&self) -> Option<SocketType> {
        match self {
            DataValue::Float(_) => Some(SocketType::Float),
            DataValue::Color(_) => Some(SocketType::Color),
            DataValue::Vector(_) => Some(SocketType::Vector),
            DataValue::Bool(_) => Some(SocketType::Bool),
            DataValue::Image(_) => Some(SocketType::Image),
            DataValue::None => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Editable node parameters (Properties inspector)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Color([f32; 4]),
    Text(String),
    Enum(String),
}

impl ParamValue {
    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        if let Self::Float(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    #[must_use]
    pub fn as_enum(&self) -> Option<&str> {
        if let Self::Enum(v) = self {
            Some(v.as_str())
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Socket definition (metadata)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SocketDef {
    pub id: String,
    pub name: String,
    pub socket_type: SocketType,
    pub default: Option<DataValue>,
}

impl SocketDef {
    pub fn new(id: impl Into<String>, name: impl Into<String>, ty: SocketType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            socket_type: ty,
            default: None,
        }
    }

    #[must_use]
    pub fn with_default(mut self, v: DataValue) -> Self {
        self.default = Some(v);
        self
    }
}

// ---------------------------------------------------------------------------
// Categories (for Add Node library)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeCategory {
    Input,
    Output,
    Color,
    Filter,
    Transform,
    Compositing,
    Utility,
}

impl NodeCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            NodeCategory::Input => "Entrée",
            NodeCategory::Output => "Sortie",
            NodeCategory::Color => "Couleur",
            NodeCategory::Filter => "Filtre",
            NodeCategory::Transform => "Transformation",
            NodeCategory::Compositing => "Compositing",
            NodeCategory::Utility => "Utilitaire",
        }
    }
}

// ---------------------------------------------------------------------------
// Node type definition (registry)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NodeDefinition {
    pub type_id: String,
    pub name: String,
    pub category: NodeCategory,
    pub inputs: Vec<SocketDef>,
    pub outputs: Vec<SocketDef>,
    pub default_params: HashMap<String, ParamValue>,
    /// Node header color
    pub header_color: [f32; 3],
    pub description: String,
}

impl NodeDefinition {
    pub fn new(
        type_id: impl Into<String>,
        name: impl Into<String>,
        category: NodeCategory,
    ) -> Self {
        Self {
            type_id: type_id.into(),
            name: name.into(),
            category,
            inputs: Vec::new(),
            outputs: Vec::new(),
            default_params: HashMap::new(),
            header_color: [0.18, 0.18, 0.20],
            description: String::new(),
        }
    }

    #[must_use]
    pub fn input(mut self, def: SocketDef) -> Self {
        self.inputs.push(def);
        self
    }

    #[must_use]
    pub fn output(mut self, def: SocketDef) -> Self {
        self.outputs.push(def);
        self
    }

    /// Adds a default parameter.
    #[must_use]
    pub fn param(mut self, key: impl Into<String>, value: ParamValue) -> Self {
        self.default_params.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn header_color(mut self, rgb: [f32; 3]) -> Self {
        self.header_color = rgb;
        self
    }

    /// Sets the node description.
    #[must_use]
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
}

// ---------------------------------------------------------------------------
// Shared UI geometry helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_type_color_is_distinct() {
        assert_ne!(SocketType::Image.color(), SocketType::Float.color());
    }

    #[test]
    fn node_def_builder() {
        let def = NodeDefinition::new(
            "brightness_contrast",
            "Luminosité / Contraste",
            NodeCategory::Color,
        )
        .input(SocketDef::new("image", "Image", SocketType::Image))
        .output(SocketDef::new("image", "Image", SocketType::Image))
        .param("brightness", ParamValue::Float(0.0))
        .param("contrast", ParamValue::Float(0.0));
        assert_eq!(def.inputs.len(), 1);
        assert_eq!(def.outputs.len(), 1);
        assert_eq!(def.default_params.len(), 2);
    }
}
