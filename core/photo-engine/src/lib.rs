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
