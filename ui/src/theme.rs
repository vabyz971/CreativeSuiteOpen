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

//! Design tokens unifiés - Thème pro dark façon Photoshop / Blender
//! Centralise toutes les couleurs, radius, shadows pour cohérence suite.

use iced::{Color, Font, Shadow, Vector};

// ---------------------------------------------------------------------------
// Typographie — DESIGN.md : Hanken Grotesk (chargée dans chaque app via .font())
// ---------------------------------------------------------------------------

pub mod fonts {
    use super::Font;
    use iced::font::Weight;

    pub const FAMILY: &str = "Hanken Grotesk";

    /// Corps de texte par défaut (400)
    pub const SANS: Font = Font::with_name(FAMILY);
    /// Titres de panneaux, labels importants (600)
    pub const SANS_SEMIBOLD: Font = Font {
        weight: Weight::Semibold,
        ..SANS
    };
    /// Headlines (700)
    pub const SANS_BOLD: Font = Font {
        weight: Weight::Bold,
        ..SANS
    };
}

// ---------------------------------------------------------------------------
// Palette dark pro
// ---------------------------------------------------------------------------

/// Tokens officiels du design system "Lumina Creative" — voir DESIGN.md.
/// Toute couleur doit passer par ces constantes, jamais codée en dur.
pub mod colors {
    use super::Color;

    // --- Surfaces (échelle Material dark, DESIGN.md) ---
    pub const SURFACE: Color = Color::from_rgb(0.0745, 0.0745, 0.0745); // #131313
    pub const SURFACE_CONTAINER_LOWEST: Color = Color::from_rgb(0.0549, 0.0549, 0.0549); // #0E0E0E
    pub const SURFACE_CONTAINER_LOW: Color = Color::from_rgb(0.1098, 0.1059, 0.1059); // #1C1B1B
    pub const SURFACE_CONTAINER: Color = Color::from_rgb(0.1255, 0.1216, 0.1216); // #201F1F
    pub const SURFACE_CONTAINER_HIGH: Color = Color::from_rgb(0.1647, 0.1647, 0.1647); // #2A2A2A
    pub const SURFACE_CONTAINER_HIGHEST: Color = Color::from_rgb(0.2078, 0.2078, 0.2039); // #353534
    pub const SURFACE_BRIGHT: Color = Color::from_rgb(0.2235, 0.2235, 0.2235); // #393939

    // --- Textes ---
    pub const TEXT_PRIMARY: Color = Color::WHITE; // #FFFFFF
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.6275, 0.6275, 0.6275); // #A0A0A0
    pub const ON_SURFACE: Color = Color::from_rgb(0.898, 0.8863, 0.8824); // #E5E2E1
    pub const ON_SURFACE_VARIANT: Color = Color::from_rgb(0.7569, 0.7765, 0.8431); // #C1C6D7
    pub const TEXT_MUTED: Color = Color::from_rgb(0.5451, 0.5647, 0.6275); // outline #8B90A0
    pub const TEXT_ON_ACCENT: Color = Color::WHITE;

    // --- Accent & actions primaires ---
    pub const ACCENT: Color = Color::from_rgb(0.0, 0.4784, 1.0); // #007AFF
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.169, 0.557, 1.0); // #2B8EFF
    pub const PRIMARY: Color = Color::from_rgb(0.6784, 0.7765, 1.0); // #ADC6FF

    // --- Bordures ---
    pub const BORDER_SUBTLE: Color = Color::from_rgb(0.1765, 0.1765, 0.1765); // #2D2D2D
    pub const OUTLINE_VARIANT: Color = Color::from_rgb(0.2549, 0.2784, 0.3333); // #414755

    // --- Alias legacy (mapping sur les tokens ci-dessus) ---
    pub const BG_APP: Color = SURFACE;
    pub const BG_PANEL: Color = SURFACE_CONTAINER_LOW;
    pub const BG_PANEL_HEADER: Color = SURFACE_CONTAINER;
    /// Teinte focalisée : accent fondu dans la surface
    pub const BG_PANEL_HEADER_FOCUSED: Color = Color::from_rgb(0.078, 0.145, 0.235);
    pub const BG_MENU_BAR: Color = SURFACE;
    pub const BG_DROPDOWN: Color = SURFACE_CONTAINER;
    pub const BG_NODE: Color = SURFACE_CONTAINER;
    pub const BG_NODE_HEADER: Color = SURFACE_CONTAINER_HIGH;
    pub const BG_NODE_SELECTED: Color = BG_PANEL_HEADER_FOCUSED;
    pub const BG_CANVAS_CHECKER_A: Color = SURFACE_CONTAINER;
    pub const BG_CANVAS_CHECKER_B: Color = SURFACE_CONTAINER_HIGH;
    pub const BG_GRAPH_GRID: Color = SURFACE_CONTAINER_LOWEST;
    pub const BG_GRAPH_DOT: Color = SURFACE_CONTAINER_HIGHEST;

    pub const BORDER_PANEL: Color = BORDER_SUBTLE;
    pub const BORDER_FOCUSED: Color = ACCENT;
    pub const BORDER_NODE: Color = SURFACE_CONTAINER_LOWEST;
    pub const BORDER_NODE_SELECTED: Color = ACCENT;

    // Hover neutre (blanc translucide, façon Affinity)
    pub const HOVER_OVERLAY: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);

    // --- Erreur ---
    pub const ERROR: Color = Color::from_rgb(1.0, 0.7059, 0.6706); // #FFB4AB
    pub const ON_ERROR: Color = Color::from_rgb(0.4078, 0.0, 0.0196); // #690005
    pub const ERROR_CONTAINER: Color = Color::from_rgb(0.5765, 0.0, 0.0392); // #93000A
    pub const SUCCESS: Color = Color::from_rgb(0.4, 0.85, 0.4);

    // En-têtes de nœuds par catégorie (RVB)
    pub const ACCENT_NODE_HEADER_IMAGE: [f32; 3] = [0.42, 0.28, 0.75];
    pub const ACCENT_NODE_HEADER_COLOR: [f32; 3] = [0.75, 0.55, 0.15];
    pub const ACCENT_NODE_HEADER_FILTER: [f32; 3] = [0.20, 0.55, 0.75];
    pub const ACCENT_NODE_HEADER_OUTPUT: [f32; 3] = [0.65, 0.20, 0.20];

    // Sockets (mirroir SocketType::color mais en Color)
    pub fn socket_color(ty: datatypes::SocketType) -> Color {
        let [r, g, b] = ty.color();
        Color::from_rgb(r, g, b)
    }

    // Cables
    pub const CABLE_SHADOW: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.4);
}

// ---------------------------------------------------------------------------
// Dimensions & Radius
// ---------------------------------------------------------------------------

pub mod metrics {
    pub const RADIUS_PANEL: f32 = 4.0;
    pub const RADIUS_NODE: f32 = 8.0;
    pub const RADIUS_BUTTON: f32 = 4.0;
    pub const RADIUS_DROPDOWN: f32 = 6.0;

    pub const BORDER_WIDTH_PANEL: f32 = 1.0;
    pub const BORDER_WIDTH_NODE: f32 = 1.0;
    pub const BORDER_WIDTH_NODE_SELECTED: f32 = 1.5;

    pub const NODE_WIDTH: f32 = 180.0;
    pub const NODE_HEADER_HEIGHT: f32 = 28.0;
    pub const NODE_ROW_HEIGHT: f32 = 22.0;
    pub const NODE_SOCKET_RADIUS: f32 = 6.0;
    pub const NODE_SOCKET_HIT_RADIUS: f32 = 12.0;

    pub const CABLE_WIDTH: f32 = 2.5;
    pub const CABLE_WIDTH_SELECTED: f32 = 3.5;

    pub const MENU_BAR_HEIGHT: f32 = 32.0;
    pub const TOOLBAR_WIDTH: f32 = 60.0;
}

// ---------------------------------------------------------------------------
// Shadows
// ---------------------------------------------------------------------------

pub mod shadows {
    use super::{Color, Shadow, Vector};

    pub fn panel() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        }
    }

    pub fn dropdown() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 15.0,
        }
    }

    pub fn node() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers container/button styles
// ---------------------------------------------------------------------------

pub fn panel_container_style(focused: bool) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(colors::BG_PANEL.into()),
        border: iced::Border {
            width: metrics::BORDER_WIDTH_PANEL,
            color: if focused {
                colors::BORDER_FOCUSED
            } else {
                colors::BORDER_PANEL
            },
            radius: metrics::RADIUS_PANEL.into(),
        },
        ..Default::default()
    }
}

pub fn node_container_style(selected: bool) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(if selected {
            colors::BG_NODE_SELECTED.into()
        } else {
            colors::BG_NODE.into()
        }),
        border: iced::Border {
            width: if selected {
                metrics::BORDER_WIDTH_NODE_SELECTED
            } else {
                metrics::BORDER_WIDTH_NODE
            },
            color: if selected {
                colors::BORDER_NODE_SELECTED
            } else {
                colors::BORDER_NODE
            },
            radius: metrics::RADIUS_NODE.into(),
        },
        shadow: shadows::node(),
        ..Default::default()
    }
}
