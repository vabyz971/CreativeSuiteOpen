//! Photo Engine — logique métier Photo extraite de apps/photo
//! Utilise suite-core + datatypes, exposé à shell et aux apps

pub mod document;
pub mod gpu;
pub mod nodes;
pub mod processor;
pub mod registry;

pub use gpu::GpuContext;
pub use processor::{evaluate, evaluate_incremental, evaluate_with_cache, to_handle};
pub use registry::{all_definitions, create_empty_graph, create_minimal_graph, create_node_for_type, definition_for};
