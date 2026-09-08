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

//! Crate `ui` — cross-cutting widget library for the suite.
//!
//! Layered architecture (bottom to top):
//!
//! 1. **`theme`** — SINGLE source of colors, sizes, radii, shadows
//!    (DESIGN.md tokens). No other module hard-codes a color.
//! 2. **`style`** — canonical styles per visual family (buttons, cards).
//!    Components reference these functions instead of writing closures.
//! 3. **Cross-cutting primitives** — reusable by ALL apps without
//!    business logic: `icon_button`, `spinner`, `dropdown`, `settings`,
//!    `shortcuts`.
//! 4. **Structural layouts** — common interface compositions:
//!    `shell`, `menu`, `base_panel`.
//! 5. **Domain canvases** — specialized displays potentially shared
//!    between apps: `image_canvas`, `layer_canvas`, `timeline`,
//!    `piano_roll`.
//!
//! App-SPECIFIC elements do NOT live here: they stay in
//! `apps/<app>/src/components/`. A component is promoted to `ui/`
//! only when a second app needs it.

pub mod base_panel;
pub mod dropdown;
pub mod icon_button;
pub mod image_canvas;
pub mod layer_canvas;
pub mod menu;
pub mod piano_roll;
pub mod settings;
pub mod shell;
pub mod shortcuts;
pub mod spinner;
pub mod style;
pub mod theme;
pub mod timeline;
