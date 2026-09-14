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

//! Widgets de base partagés par les 3 apps (boutons, sliders, inputs…).
//!
//! Tous les widgets utilisent exclusivement les tokens de
//! [`crate::theme`] : aucune couleur ni taille en dur ici.

pub mod button;
pub mod dropdown;
pub mod icon;
pub mod input;
pub mod reorderable_list;
pub mod slider;
pub mod toggle;
pub mod tooltip;

pub use button::{CygnusButton, CygnusButtonStyle};
pub use dropdown::CygnusDropdown;
pub use icon::{ALL_ICONS, CygnusIcon, icon_button};
pub use input::{CygnusNumberInput, CygnusTextInput};
pub use reorderable_list::{
    ReorderableList, compute_target_index, draw_drop_indicator, item_background,
};
pub use slider::CygnusSlider;
pub use toggle::CygnusToggle;
