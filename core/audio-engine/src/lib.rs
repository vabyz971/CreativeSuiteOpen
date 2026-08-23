//! Audio Engine — FL Studio-like : piano roll, mixer, effets
//! Même Graph que Photo/Video mais sockets AudioBuffer (Float/Vector)

use datatypes::{NodeCategory, NodeDefinition, ParamValue, SocketDef, SocketType};

pub fn all_definitions() -> Vec<NodeDefinition> {
    vec![
        NodeDefinition::new("sample_input", "Sample", NodeCategory::Input)
            .output(SocketDef::new("audio", "Audio", SocketType::Vector))
            .header_color([0.20, 0.55, 0.35])
            .description("Sample audio"),
        NodeDefinition::new("oscillator", "Oscillateur", NodeCategory::Utility)
            .output(SocketDef::new("audio", "Audio", SocketType::Vector))
            .param("freq", ParamValue::Float(440.0))
            .param("wave", ParamValue::Enum("Sine".into()))
            .header_color([0.85, 0.55, 0.10])
            .description("FL Studio oscillator"),
        NodeDefinition::new("filter", "Filtre", NodeCategory::Filter)
            .input(SocketDef::new("audio", "Audio", SocketType::Vector))
            .output(SocketDef::new("audio", "Audio", SocketType::Vector))
            .param("cutoff", ParamValue::Float(1000.0))
            .param("res", ParamValue::Float(0.5))
            .header_color([0.20, 0.55, 0.75])
            .description("Filtre passe-bas"),
        NodeDefinition::new("mixer", "Mixer", NodeCategory::Compositing)
            .input(SocketDef::new("audio_a", "Audio A", SocketType::Vector))
            .input(SocketDef::new("audio_b", "Audio B", SocketType::Vector))
            .input(SocketDef::new("gain", "Gain", SocketType::Float))
            .output(SocketDef::new("audio", "Audio", SocketType::Vector))
            .param("gain", ParamValue::Float(0.8))
            .header_color([0.45, 0.35, 0.65])
            .description("Table de mixage"),
        NodeDefinition::new("output_audio", "Master Out", NodeCategory::Output)
            .input(SocketDef::new("audio", "Audio", SocketType::Vector))
            .header_color([0.65, 0.20, 0.20])
            .description("Sortie master"),
    ]
}
