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

//! Styles canoniques de la suite — UNE définition par famille visuelle.
//!
//! Règle : un composant n'écrit JAMAIS une closure de style à la main ;
//! il référence une fonction de ce module. Toute couleur/rayon consommé ici
//! provient de [`crate::theme`] (DESIGN.md). Ajouter une variante ici plutôt
//! que de dupliquer ailleurs.

use iced::widget::{button, container};
use iced::{Border, Color, Shadow};

use crate::theme::{colors, metrics};

/// Bouton « fantôme » : transparent au repos, voile blanc au survol.
/// Famille Affinity — barres d'outils, en-têtes, actions discrètes.
pub fn ghost(status: button::Status) -> button::Style {
    ghost_variant(status, false)
}

/// Variante sélectionnée du bouton fantôme : teinte accent fondue.
pub fn ghost_selected(selected: bool, status: button::Status) -> button::Style {
    ghost_variant(status, selected)
}

fn ghost_variant(status: button::Status, selected: bool) -> button::Style {
    let background: Option<Color> = if selected {
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
            radius: metrics::RADIUS_BUTTON.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

/// Bouton d'outil de palette flottante (façon macOS) : icône discrète au
/// repos, éclaircie au survol, sélection = teinte accent arrondie.
/// Le texte enfant ne doit PAS fixer sa couleur : c'est ce style qui pilote.
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

/// Entrée de menu / dropdown : plein ACCENT au survol, texte inversé.
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

/// Bouton primaire (DESIGN.md « Buttons > Primary ») : ACCENT plein,
/// éclairci au survol. Actions principales uniquement (Créer, Exporter…).
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

/// Puce posée sur une barre (sélecteur de projet, presets d'accueil) :
/// surface avec bordure subtile, éclaircie au survol.
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

/// Bouton fantôme à connotation destructive : voile rouge au survol
/// (fermer/vider un réglage sensible).
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

/// Petit bouton d'action compact (tableaux de raccourcis, lignes de
/// réglages) : pastille grise au repos, ACCENT au survol.
pub fn action_chip(status: button::Status) -> button::Style {
    action_chip_colored(status, colors::ACCENT)
}

/// Variante destructive de [`action_chip`] : voile rouge au survol.
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

/// Carte flottante (dropdown, menu de tâches, palette d'outils) — panneau
/// « Floating Panel » du DESIGN.md : surface + bordure subtile + ombre.
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

/// Carte posée dans un panneau (pas d'ombre) — fonds de listes, vignettes.
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
