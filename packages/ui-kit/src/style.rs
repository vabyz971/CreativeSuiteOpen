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

//! Canonical suite styles — ONE definition per visual family.
//!
//! Rule: a component NEVER writes a style closure by hand;
//! it references a function from this module. Every color/radius consumed here
//! comes from [`crate::theme`] (DESIGN.md). Add a variant here rather
//! than duplicating elsewhere.

use iced::widget::{button, container, text_input};
use iced::{Border, Color, Shadow};

use crate::theme::{colors, metrics};

/// Ghost button: transparent at rest, white veil on hover.
/// Affinity family — toolbars, headers, discreet actions.
#[must_use]
pub fn ghost(status: button::Status) -> button::Style {
    ghost_variant(status, false)
}

/// Selected variant of ghost button: blended accent tint with rounded radius.
#[must_use]
pub fn ghost_selected(selected: bool, status: button::Status) -> button::Style {
    ghost_variant(status, selected)
}

fn ghost_variant(status: button::Status, selected: bool) -> button::Style {
    let background: Option<Color> = if selected {
        // Fond bleu translucide pour la ligne sélectionnée
        Some(colors::BG_PANEL_HEADER_FOCUSED)
    } else if status == button::Status::Hovered {
        Some(colors::HOVER_OVERLAY)
    } else {
        None
    };
    button::Style {
        background: background.map(iced::Background::Color),
        text_color: colors::TEXT_PRIMARY,
        border: Border {
            radius: metrics::RADIUS_BUTTON.into(), // 10.0
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// Primary button in pill shape (Export button) — RADIUS_PILL=20
#[must_use]
pub fn primary_pill(status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        colors::ACCENT_HOVER
    } else {
        colors::ACCENT
    };
    button::Style {
        background: Some(background.into()),
        text_color: colors::TEXT_ON_ACCENT,
        border: Border {
            radius: metrics::RADIUS_PILL.into(), // 20.0
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Floating palette tool button (macOS-style): discreet icon at
/// rest, brightened on hover, selection = rounded accent tint.
/// Child text must NOT set its color: this style drives it.
#[must_use]
pub fn tool_button(selected: bool, status: button::Status) -> button::Style {
    let hovered = status == button::Status::Hovered;
    let background: Option<Color> = if selected {
        Some(colors::BG_PANEL_HEADER_FOCUSED)
    } else if hovered {
        Some(colors::HOVER_OVERLAY)
    } else {
        None
    };
    button::Style {
        background: background.map(iced::Background::Color),
        text_color: if selected || hovered {
            colors::TEXT_PRIMARY
        } else {
            colors::TEXT_SECONDARY
        },
        border: Border {
            radius: metrics::RADIUS_DROPDOWN.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// Menu/dropdown entry: solid ACCENT on hover, inverted text.
#[must_use]
pub fn menu_item(status: button::Status) -> button::Style {
    if status == button::Status::Hovered {
        button::Style {
            background: Some(colors::ACCENT.into()),
            text_color: colors::TEXT_ON_ACCENT,
            border: Border {
                radius: metrics::RADIUS_BUTTON.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    } else {
        button::Style {
            background: Some(Color::TRANSPARENT.into()),
            text_color: colors::ON_SURFACE,
            border: Border {
                radius: metrics::RADIUS_BUTTON.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Primary button (DESIGN.md "Buttons > Primary"): solid ACCENT,
/// brightened on hover. Primary actions only (Create, Export...).
#[must_use]
pub fn primary(status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        colors::ACCENT_HOVER
    } else {
        colors::ACCENT
    };
    button::Style {
        background: Some(background.into()),
        text_color: colors::TEXT_ON_ACCENT,
        border: Border {
            radius: metrics::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Chip on a bar (project selector, home presets):
/// surface with subtle border, brightened on hover.
#[must_use]
pub fn chip(status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        colors::SURFACE_CONTAINER_HIGH
    } else {
        colors::SURFACE_CONTAINER
    };
    button::Style {
        background: Some(background.into()),
        text_color: colors::TEXT_PRIMARY,
        border: Border {
            radius: metrics::RADIUS_DROPDOWN.into(),
            width: 1.0,
            color: colors::BORDER_SUBTLE,
        },
        ..Default::default()
    }
}

/// Ghost button with destructive connotation: red veil on hover
/// (close/clear a sensitive setting).
#[must_use]
pub fn ghost_danger(status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        Some(colors::ERROR_CONTAINER)
    } else {
        None
    };
    button::Style {
        background: background.map(iced::Background::Color),
        text_color: colors::TEXT_PRIMARY,
        border: Border {
            radius: metrics::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Small compact action button (shortcut tables, setting rows):
/// grey pill at rest, ACCENT on hover.
#[must_use]
pub fn action_chip(status: button::Status) -> button::Style {
    action_chip_colored(status, colors::ACCENT)
}

/// Destructive variant of [`action_chip`]: red veil on hover.
#[must_use]
pub fn action_chip_danger(status: button::Status) -> button::Style {
    action_chip_colored(status, colors::ERROR_CONTAINER)
}

fn action_chip_colored(status: button::Status, hover: Color) -> button::Style {
    let hovered = status == button::Status::Hovered;
    button::Style {
        background: Some(
            if hovered {
                hover
            } else {
                colors::SURFACE_CONTAINER_HIGH
            }
            .into(),
        ),
        text_color: if hovered {
            colors::TEXT_ON_ACCENT
        } else {
            colors::TEXT_SECONDARY
        },
        border: Border {
            radius: metrics::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Floating card (dropdown, task menu, tool palette) — panel
/// DESIGN.md "Floating Panel": surface + subtle border + shadow.
#[must_use]
pub fn floating_card(background: Color, radius: f32, shadow: Shadow) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border {
            width: metrics::BORDER_WIDTH_PANEL,
            color: colors::BORDER_SUBTLE,
            radius: radius.into(),
        },
        shadow,
        ..Default::default()
    }
}

/// Card inside a panel (no shadow) — list backgrounds, thumbnails.
#[must_use]
pub fn inset_card(background: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: Border {
            width: metrics::BORDER_WIDTH_PANEL,
            color: colors::BORDER_SUBTLE,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

/// Inline text input for layer names: transparent at rest, subtle accent border on focus.
/// Eliminates the "form field" look on every layer row.
#[must_use]
pub fn inline_name_input(status: text_input::Status) -> text_input::Style {
    let is_focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: if is_focused {
            iced::Background::Color(colors::SURFACE_CONTAINER_LOWEST)
        } else {
            iced::Background::Color(Color::TRANSPARENT)
        },
        border: iced::Border {
            radius: iced::border::Radius::from(metrics::RADIUS_SM),
            width: if is_focused { 1.0 } else { 0.0 },
            color: if is_focused {
                colors::ACCENT
            } else {
                Color::TRANSPARENT
            },
        },
        icon: colors::TEXT_MUTED,
        placeholder: colors::TEXT_MUTED,
        value: colors::TEXT_PRIMARY,
        selection: colors::ACCENT,
    }
}
