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

//! Crate `ui` — bibliothèque de widgets transverses de la suite.
//!
//! Architecture en couches (du bas vers le haut) :
//!
//! 1. **`theme`** — SEULE source de couleurs, tailles, rayons, ombres
//!    (tokens du DESIGN.md). Aucun autre module ne code une couleur en dur.
//! 2. **`style`** — styles canoniques par famille visuelle (boutons, cartes).
//!    Les composants référencent ces fonctions au lieu d'écrire des closures.
//! 3. **Primitives transverses** — réutilisables par TOUTES les apps sans
//!    logique métier : `icon_button`, `spinner`, `dropdown`, `settings`,
//!    `shortcuts`.
//! 4. **Layouts structurels** — compositions d'interface communes :
//!    `shell`, `menu`, `base_panel`.
//! 5. **Canvas domaine** — affichages spécialisés potentiellement partagés
//!    entre apps : `image_canvas`, `layer_canvas`, `node_graph`,
//!    `timeline`, `piano_roll`.
//!
//! Les éléments SPÉCIFIQUES à une app ne vivent PAS ici : ils restent dans
//! `apps/<app>/src/components/`. Un composant est promu dans `ui/`
//! uniquement quand une deuxième app en a besoin.

pub mod base_panel;
pub mod dropdown;
pub mod icon_button;
pub mod image_canvas;
pub mod layer_canvas;
pub mod menu;
pub mod node_graph;
pub mod piano_roll;
pub mod settings;
pub mod shell;
pub mod shortcuts;
pub mod spinner;
pub mod style;
pub mod theme;
pub mod timeline;
