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

//! Modal Préférences unifiée (comme Photoshop → Préférences).
//! Sidebar gauche des sections d'options, contenu à droite :
//! Général (matériel, épuré), Raccourcis clavier, À propos.
//! Design : tokens DESIGN.md via ui::theme.

use ui::shortcuts::{Action, Shortcuts};
use ui::theme::{colors, fonts, metrics};

use crate::Message;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding};

/// Sections de la sidebar Préférences
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrefsSection {
    General,
    Shortcuts,
    About,
}

/// Modal centrée prenant la majorité de la vue de l'application.
/// Le scrim et l'empilement sont gérés par l'appelant.
pub fn view<'a>(
    shortcuts: &'a Shortcuts,
    capturing: Option<Action>,
    section: PrefsSection,
    gpu_info: Option<String>,
    gpu_available: bool,
) -> Element<'a, Message> {
    // --- En-tête : titre + fermeture ---
    let header = container(
        row![
            text("Préférences")
                .size(16)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            button(text("✕").size(13).color(colors::TEXT_PRIMARY))
                .padding(5)
                .style(|_, s| ui::style::ghost_danger(s))
                .on_press(Message::ClosePreferences),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(12.0).left(16.0).right(12.0))
    .style(|_| container::Style {
        background: Some(colors::BG_NODE_HEADER.into()),
        ..Default::default()
    });

    // --- Sidebar gauche ---
    let sidebar_entry = |label: &'a str, section_kind: PrefsSection| {
        let is_active = section == section_kind;
        button(
            text(label)
                .size(13)
                .color(if is_active {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                })
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(Padding::new(8.0).left(12.0).right(8.0))
        .style(move |_t, s| ui::style::ghost_selected(is_active, s))
        .on_press(Message::PrefsSection(section_kind))
    };

    let sidebar = container(
        column![
            sidebar_entry("Général", PrefsSection::General),
            sidebar_entry("Raccourcis clavier", PrefsSection::Shortcuts),
            sidebar_entry("À propos", PrefsSection::About),
            Space::new().height(Length::Fill),
        ]
        .spacing(4)
        .padding(10),
    )
    .width(Length::Fixed(190.0))
    .height(Length::Fill)
    .style(|_| container::Style {
        background: Some(colors::SURFACE_CONTAINER_LOW.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    });

    // --- Contenu droit selon la section ---
    let content: Element<'a, Message> = match section {
        PrefsSection::General => general_view(gpu_info, gpu_available),
        PrefsSection::Shortcuts => shortcuts_view(shortcuts, capturing),
        PrefsSection::About => centered_note(
            "Creative Suite Open — Photo",
            "Suite créative professionnelle open source pour Linux, Windows et macOS.\nLicence GPL-3.0-or-later — voir LICENSE.",
        ),
    };

    // --- Panneau : occupe la majeure partie de la vue (marges 40 px) ---
    container(
        container(column![header, row![sidebar, content].height(Length::Fill)])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::BG_PANEL.into()),
                border: iced::Border {
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                    radius: 8.0.into(),
                    ..Default::default()
                },
                shadow: iced::Shadow {
                    color: colors::CABLE_SHADOW,
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
                ..Default::default()
            }),
    )
    .padding(40)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Section Général (épurée : état du rendu matériel, rien d'autre)
// ---------------------------------------------------------------------------

fn general_view<'a>(gpu_info: Option<String>, gpu_available: bool) -> Element<'a, Message> {
    let status = if gpu_available {
        "WGPU Direct"
    } else {
        "CPU Fallback"
    };
    let status_color = if gpu_available {
        colors::SUCCESS
    } else {
        colors::ERROR
    };

    let card = container(
        column![
            row![
                text("Rendu").size(13).color(colors::ON_SURFACE),
                Space::new().width(Length::Fill),
                text(status).size(12).color(status_color),
            ]
            .align_y(Alignment::Center),
            Space::new().height(Length::Fixed(10.0)),
            container(
                text(gpu_info.unwrap_or_else(|| "Détection en cours...".into()))
                    .size(11)
                    .color(colors::TEXT_SECONDARY)
            )
            .padding(10)
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::SURFACE_CONTAINER_LOWEST.into()),
                border: iced::Border {
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                    radius: metrics::RADIUS_DROPDOWN.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Space::new().height(Length::Fixed(10.0)),
            button(
                text("Relancer la détection")
                    .size(12)
                    .color(colors::TEXT_ON_ACCENT)
            )
            .padding(Padding::new(6.0).left(10.0).right(10.0))
            .style(|_, s| ui::style::primary(s))
            .on_press(Message::DetectGpu),
        ]
        .spacing(4),
    )
    .padding(14)
    .width(Length::Fixed(520.0))
    .style(|_| container::Style {
        background: Some(colors::SURFACE_CONTAINER.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            radius: metrics::RADIUS_DROPDOWN.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    container(column![card])
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn centered_note<'a>(title: &'a str, body: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title)
                .size(18)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().height(Length::Fixed(10.0)),
            text(body).size(13).color(colors::TEXT_SECONDARY),
        ]
        .spacing(4),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Section Raccourcis clavier
// ---------------------------------------------------------------------------

fn shortcuts_view<'a>(shortcuts: &'a Shortcuts, capturing: Option<Action>) -> Element<'a, Message> {
    // Ligne d'action : libellé | chip combinaison | Modifier | Défaut
    let action_row = |action: Action| -> Element<'a, Message> {
        let is_capturing = capturing == Some(action);
        let current = shortcuts.label(action);
        let binding_label = if is_capturing {
            "Appuyez sur une touche… (Échap annule)".to_string()
        } else if current.is_empty() {
            "—".to_string()
        } else {
            current.clone()
        };

        let capturing_flag = is_capturing;
        let chip = container(text(binding_label).size(11).color(if is_capturing {
            colors::ACCENT
        } else if current.is_empty() {
            colors::TEXT_MUTED
        } else {
            colors::TEXT_PRIMARY
        }))
        .padding(Padding::new(4.0).left(8.0).right(8.0))
        .style(move |_| container::Style {
            background: Some(if capturing_flag {
                colors::BG_NODE_SELECTED.into()
            } else {
                colors::SURFACE_CONTAINER_HIGH.into()
            }),
            border: iced::Border {
                width: if capturing_flag { 1.0 } else { 0.0 },
                color: colors::ACCENT,
                radius: metrics::RADIUS_BUTTON.into(),
            },
            ..Default::default()
        });

        let small_btn = |label: &'a str, msg: Message, enabled: bool| {
            let b = button(text(label).size(11)).padding(Padding::new(3.0).left(8.0).right(8.0));
            if enabled {
                b.on_press(msg).style(|_, s| ui::style::action_chip(s))
            } else {
                b.style(|_, _| button::Style::default())
            }
        };

        row![
            text(action.label()).size(12).color(colors::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            chip,
            small_btn(
                "Modifier",
                Message::ShortcutCapture(action),
                capturing.is_none(),
            ),
            small_btn(
                "Défaut",
                Message::ShortcutReset(action),
                capturing.is_none(),
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(8)
        .into()
    };

    let mut list = column![].spacing(6);
    let mut last_cat = "";
    for action in Action::all() {
        let cat = action.category();
        if cat != last_cat {
            last_cat = cat;
            list = list.push(
                text(cat)
                    .size(12)
                    .font(fonts::SANS_SEMIBOLD)
                    .color(colors::ACCENT),
            );
        }
        list = list.push(action_row(action));
    }
    let list = list.padding(16);

    let header = container(
        row![
            text("Raccourcis clavier")
                .size(15)
                .font(fonts::SANS_SEMIBOLD)
                .color(colors::TEXT_PRIMARY),
            Space::new().width(Length::Fill),
            button(text("Tout réinitialiser").size(11))
                .padding(Padding::new(4.0).left(10.0).right(10.0))
                .on_press(Message::ShortcutResetAll)
                .style(|_, s| ui::style::action_chip_danger(s)),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::new(12.0).left(16.0).right(16.0));

    let footer = container(
        text("Cliquez sur « Modifier » puis appuyez sur la nouvelle combinaison. Les modifications sont enregistrées automatiquement.")
            .size(11)
            .color(colors::TEXT_MUTED),
    )
    .width(Length::Fill)
    .padding(Padding::new(10.0).left(16.0).right(16.0));

    column![
        header,
        container(scrollable(list))
            .width(Length::Fill)
            .height(Length::Fill),
        footer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
