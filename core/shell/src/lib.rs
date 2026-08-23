//! Suite Shell — couche minimaliste et modulaire partagée par Photo/Video/Audio
//! Inspiré Final Cut (timeline) et FL Studio (mixer) mais avec même coque.

use datatypes::{NodeDefinition, Vec2};
use suite_core::{Graph, Node};
use datatypes::NodeId;

/// Descripteur d'une app de la suite. Chaque app (photo, video, audio) implémente ce trait.
pub trait SuiteApp {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn icon(&self) -> &'static str; // Material Icon codepoint
    fn node_definitions(&self) -> Vec<NodeDefinition>;
    fn create_demo_graph(&self) -> Graph;
    fn create_empty_graph(&self) -> Graph;
}

/// Kind minimaliste pour le switcher top-bar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppKind {
    Photo,
    Video,
    Audio,
}

impl AppKind {
    pub fn label(&self) -> &'static str {
        match self {
            AppKind::Photo => "Photo",
            AppKind::Video => "Vidéo",
            AppKind::Audio => "Audio",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            AppKind::Photo => "\u{e412}", // camera
            AppKind::Video => "\u{e04b}", // movie
            AppKind::Audio => "\u{e405}", // music_note
        }
    }
}

/// État du shell — agnostique du domaine
#[derive(Debug, Clone)]
pub struct ShellState {
    pub active_app: AppKind,
    pub command_palette_open: bool,
    pub left_rail_collapsed: bool,
    pub right_inspector_collapsed: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            active_app: AppKind::Photo,
            command_palette_open: false,
            left_rail_collapsed: false,
            right_inspector_collapsed: false,
        }
    }
}

/// Fabrique générique d'un graphe minimal (Input -> Output) réutilisable
pub fn minimal_graph_for(kind: AppKind) -> Graph {
    let mut g = Graph::new();
    let (input_type, output_type) = match kind {
        AppKind::Photo => ("input_image", "output"),
        AppKind::Video => ("input_video", "output_video"),
        AppKind::Audio => ("input_audio", "output_audio"),
    };
    let input = g.add_node(Node {
        id: NodeId(0),
        type_id: input_type.into(),
        name: match kind {
            AppKind::Photo => "Image Source".into(),
            AppKind::Video => "Clip Source".into(),
            AppKind::Audio => "Audio Source".into(),
        },
        position: Vec2::new(40.0, 120.0),
        params: Default::default(),
        preview_enabled: false,
        enabled: true,
    });
    let output = g.add_node(Node {
        id: NodeId(0),
        type_id: output_type.into(),
        name: "Sortie".into(),
        position: Vec2::new(400.0, 120.0),
        params: Default::default(),
        preview_enabled: true,
        enabled: true,
    });
    let _ = g.connect(suite_core::Connection::new(
        input,
        "image",
        output,
        "image",
        datatypes::SocketType::Image,
    ));
    g
}
