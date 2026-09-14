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
//! Panneau des calques de l'app PHOTO (widget métier).
//!
//! Les types (`PhotoLayerInfo`) sont dérivés de `photo-engine` ; toute
//! la mécanique de drag & drop est déléguée à
//! `ui_kit::ReorderableList`. Les commandes partent au moteur via le
//! channel (`PhotoEngineCommand`, voir [`super::engine_bridge`]).

pub mod item;
pub mod panel;
pub mod types;

pub use item::{LayerItemAction, LayerRenameState, draw_photo_layer_item};
pub use panel::{LayerPanelAction, PHOTO_LAYER_ITEM_HEIGHT, draw_photo_layer_panel};
pub use types::{PhotoLayerInfo, PhotoLayerKind, PhotoSubLayerInfo, snapshot_layers};
