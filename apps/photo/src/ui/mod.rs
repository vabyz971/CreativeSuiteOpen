// Cygnus — Suite créative professionnelle open source
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

#![allow(dead_code)] // TODO(Phase 5) : levé au câblage dans app.rs.
#![allow(unused_imports)] // TODO(Phase 5) : idem.
//! Widgets egui MÉTIER de l'app Photo (migration Iced → egui, prompt v2).
//!
//! Ces widgets manipulent les types de `photo-engine` et composent les
//! briques GÉNÉRIQUES de `ui_kit` (thème, icônes, `ReorderableList`,
//! `Viewport`). Ils vivront dans l'app Phase 5 (eframe) ; en Phase 3
//! ils sont testés en headless.

pub mod canvas;
pub mod colorpanel;
pub mod dialogs;
pub mod engine_bridge;
pub mod layers;
pub mod menubar;
pub mod modebar;
pub mod optionsbar;
pub mod properties;
pub mod toolbar;

pub use canvas::{PhotoBrushSettings, PhotoCanvas, PhotoCanvasOutcome, PhotoCanvasTool};
pub use colorpanel::draw_color_panel;
pub use dialogs::{
    DocOrientation, ExportDialogState, ExportRequest, NewDocumentDialogState, draw_export_dialog,
    draw_help_dialog, draw_new_document_dialog,
};
pub use engine_bridge::{
    PhotoEngineCommand, PhotoEngineResponse, PreviewImage, apply_command, display_to_doc_index,
    spawn_photo_engine_worker,
};
pub use layers::{
    LayerItemAction, LayerPanelAction, LayerRenameState, PhotoLayerInfo, PhotoLayerKind,
    PhotoSubLayerInfo, draw_photo_layer_panel,
};
pub use menubar::{MenuAvailability, PhotoMenuAction, draw_menu_bar};
pub use modebar::{PhotoEditMode, draw_photo_modebar};
pub use optionsbar::draw_tool_options;
pub use toolbar::draw_tool_rail;
