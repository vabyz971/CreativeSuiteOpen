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

//! Panneau Calques découpé par responsabilité :
//! - [`panel`] : rendu du panneau, itération des lignes, indicateurs de drop ;
//! - [`row`] : rendu d'une ligne (sélection, visibilité, expansion) ;
//! - [`drag`] : machine d'état du geste (pressé, déplacement, relâchement) ;
//! - [`drop_target`] : calcul pur des cibles Before/After/Inside.

pub mod drag;
pub mod drop_target;
pub mod panel;
pub mod row;

pub use drag::{DRAG_DEADBAND, LayerDragRelease, LayerDragState};
pub use drop_target::{DropPosition, LayerDropTarget, resolve_drop_target};
