//! Video Engine — Final Cut-like : timeline, transitions, effets
//! Réutilise suite-core Graph + datatypes, même pattern que photo-engine

use datatypes::{NodeCategory, NodeDefinition, ParamValue, SocketDef, SocketType};

pub fn all_definitions() -> Vec<NodeDefinition> {
    vec![
        NodeDefinition::new("clip_input", "Clip Source", NodeCategory::Input)
            .output(SocketDef::new("video", "Video", SocketType::Image))
            .output(SocketDef::new("audio", "Audio", SocketType::Vector))
            .header_color([0.25, 0.45, 0.75])
            .description("Source clip vidéo"),
        NodeDefinition::new("transition", "Transition", NodeCategory::Compositing)
            .input(SocketDef::new("video_a", "Video A", SocketType::Image))
            .input(SocketDef::new("video_b", "Video B", SocketType::Image))
            .input(SocketDef::new("progress", "Progress", SocketType::Float))
            .output(SocketDef::new("video", "Video", SocketType::Image))
            .param("type", ParamValue::Enum("CrossDissolve".into()))
            .param("progress", ParamValue::Float(0.5))
            .header_color([0.45, 0.35, 0.65])
            .description("Transition Final Cut"),
        NodeDefinition::new("color_grade", "Étalonnage", NodeCategory::Color)
            .input(SocketDef::new("video", "Video", SocketType::Image))
            .output(SocketDef::new("video", "Video", SocketType::Image))
            .param("exposure", ParamValue::Float(0.0))
            .header_color([0.75, 0.55, 0.15])
            .description("Étalonnage couleur"),
        NodeDefinition::new("output_video", "Sortie Vidéo", NodeCategory::Output)
            .input(SocketDef::new("video", "Video", SocketType::Image))
            .header_color([0.65, 0.20, 0.20])
            .description("Master timeline"),
    ]
}
