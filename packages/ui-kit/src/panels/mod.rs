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

//! Panneaux de mise en page partagés par les 3 apps.
//!
//! Les panneaux utilisent exclusivement les tokens de
//! [`crate::theme`] : aucune couleur ni taille en dur ici.
//! La DISPOSITION des panneaux reste propre à chaque app
//! (aucun layout partagé imposé) : les apps composent ces
//! conteneurs dans leur `layout.rs`.
//!
//! Pour les zones redimensionnables, voir [`split`].

pub mod collapsible;
pub mod panel;
pub mod split;
pub mod tabs;
pub mod toolbar;

pub use collapsible::CygnusCollapsible;
pub use panel::CygnusPanel;
pub use split::{CygnusSplitPanel, CygnusSplitState};
pub use tabs::CygnusTabs;
pub use toolbar::CygnusToolbar;
