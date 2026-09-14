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

//! Crate `ui_kit` — design system egui de la suite Cygnus.
//!
//! Architecture en couches (bas vers haut) :
//!
//! 1. **`theme`** — SEULE source des couleurs, espacements, rayons,
//!    typo. Aucun autre module ne code de valeur en dur.
//! 2. **`widgets`** — boutons, sliders, inputs, dropdown, toggles,
//!    tooltips, icônes (`CygnusIcon`, seul contact avec
//!    `egui_material_icons`) et liste réordonnable générique.
//! 3. **`panels`** — conteneurs (panneau titré, split
//!    redimensionnable, onglets, repliable, toolbar). La disposition
//!    reste propre à chaque app (aucun layout partagé).
//! 4. **`viewport`** — état zoom/pan générique + affichage texture.
//! 5. **`dialogs`** — modales, sélecteurs de fichiers, progression.
//! 6. **`utils`** — état de drag & drop générique (index).
//!
//! INTERDIT ici : toute référence aux types métier des apps et aux
//! engines. Les widgets métier vivent dans `apps/*/src/ui/`.

pub mod dialogs;
pub mod panels;
pub mod theme;
pub mod utils;
pub mod viewport;
pub mod widgets;
