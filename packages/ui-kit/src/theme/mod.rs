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

//! Thème egui partagé de Cygnus : tokens, typographie et application.
//!
//! SEULE source de vérité pour les couleurs, espacements, rayons et
//! tailles de texte des 3 apps. Aucune couleur codée en dur ailleurs.

pub mod tokens;
pub mod typography;
pub mod visuals;

pub use tokens::{CygnusColors, CygnusRadius, CygnusSpacing, CygnusTheme, CygnusTypography};
pub use visuals::{apply_cygnus_theme, setup_fonts};
