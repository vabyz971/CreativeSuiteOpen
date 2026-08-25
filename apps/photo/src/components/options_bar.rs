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

//! Barre d'options CONTEXTUELLE (comme la barre d'options de Photoshop) :
//! son contenu change selon l'outil sélectionné.
//!
//! Ajouter des réglages à un outil = ajouter une section dans `render`
//! et une branche au match. Un module par préoccupation :
//! - `brush_section`  → Pinceau (couleur, taille, opacité)
//! - `move_section`   → Déplacement (rotation, échelle, flip, reset, crop)
//! - outils sans réglages → barre vide (hauteur nulle)

use crate::{Message, Tool};
use iced::widget::{button, container, row, text};
use iced::{Alignment, Element, Length, Padding};
use ui::theme::{colors, metrics};

// Codepoints Material Icons — https://fonts.google.com/icons
const ICON_ROTATE_LEFT: &str = "\u{e419}";
const ICON_ROTATE_RIGHT: &str = "\u{e41a}";
const ICON_FLIP: &str = "\u{e3e8}"; // flip
const ICON_CROP: &str = "\u{e3be}";
const ICON_RESET: &str = "\u{e166}"; // restart_alt

/// Barre contextuelle complète. Retourne un élément de hauteur nulle si
/// l'outil courant n'a pas de réglages (aucun espace pris dans le layout).
pub fn render<'a>(
    tool: Tool,
    selected_layer: Option<u64>,
    selected_scale_percent: Option<f32>,
    has_selection: bool,
    brush_color: iced::Color,
    brush_size: f32,
    brush_opacity: f32,
    color_picker_open: bool,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = match tool {
        Tool::Brush => brush_section(brush_color, brush_size, brush_opacity, color_picker_open),
        Tool::Eraser => eraser_section(brush_size, brush_opacity),
        Tool::Move => move_section(selected_layer, selected_scale_percent, has_selection),
        _ => {
            // Aucun réglage pour cet outil : hauteur nulle
            return iced::widget::Space::new()
                .width(Length::Fill)
                .height(Length::Fixed(0.0))
                .into();
        }
    };

    container(
        row![content]
            .spacing(14)
            .align_y(Alignment::Center)
            .padding(Padding::new(0.0).left(12.0).right(12.0)),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(colors::SURFACE_CONTAINER_LOW.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// Section PINCEAU : couleur / taille / opacité
// ---------------------------------------------------------------------------

fn brush_section<'a>(
    brush_color: iced::Color,
    brush_size: f32,
    brush_opacity: f32,
    color_picker_open: bool,
) -> Element<'a, Message> {
    // Cercle couleur → ColorPicker iced_aw
    let swatch = button(
        container(
            iced::widget::Space::new()
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(14.0)),
        )
        .style(move |_| container::Style {
            background: Some(brush_color.into()),
            border: iced::Border {
                width: 1.0,
                color: colors::BORDER_SUBTLE,
                radius: iced::border::Radius::new(7.0),
            },
            ..Default::default()
        }),
    )
    .padding(5)
    .style(|_t, s| {
        let mut st = button::Style::default();
        st.background = Some(if s == button::Status::Hovered {
            colors::HOVER_OVERLAY.into()
        } else {
            iced::Color::TRANSPARENT.into()
        });
        st.border.radius = metrics::RADIUS_BUTTON.into();
        st
    })
    .on_press(Message::ToggleColorPicker);

    let color_circle = iced_aw::widget::ColorPicker::new(
        color_picker_open,
        brush_color,
        swatch,
        Message::ToggleColorPicker,
        Message::SetBrushColor,
    );

    let size_slider = row![
        field_label("Taille"),
        iced::widget::slider(1.0..=200.0, brush_size, Message::SetBrushSize)
            .width(Length::Fixed(110.0)),
        value_label(format!("{:.0}", brush_size)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let opacity_slider = row![
        field_label("Opacité"),
        iced::widget::slider(0.05..=1.0, brush_opacity, Message::SetBrushOpacity)
            .width(Length::Fixed(90.0)),
        value_label(format!("{:.0}%", brush_opacity * 100.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    row![
        field_label("Pinceau"),
        separator(),
        color_circle,
        separator(),
        size_slider,
        opacity_slider,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding(Padding::new(5.0))
    .into()
}

// ---------------------------------------------------------------------------
// Section GOMME : taille + opacité (pas de couleur — efface l'alpha)
// ---------------------------------------------------------------------------

fn eraser_section<'a>(brush_size: f32, brush_opacity: f32) -> Element<'a, Message> {
    let size_slider = row![
        field_label("Taille"),
        iced::widget::slider(1.0..=200.0, brush_size, Message::SetBrushSize)
            .width(Length::Fixed(110.0)),
        value_label(format!("{:.0}", brush_size)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let opacity_slider = row![
        field_label("Opacité"),
        iced::widget::slider(0.05..=1.0, brush_opacity, Message::SetBrushOpacity)
            .width(Length::Fixed(90.0)),
        value_label(format!("{:.0}%", brush_opacity * 100.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    row![
        field_label("Gomme"),
        separator(),
        size_slider,
        opacity_slider,
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding(Padding::new(5.0))
    .into()
}

// ---------------------------------------------------------------------------
// Section DÉPLACEMENT : rotation / échelle / flip / reset / crop
// ---------------------------------------------------------------------------

fn move_section<'a>(
    selected_layer: Option<u64>,
    selected_scale_percent: Option<f32>,
    has_selection: bool,
) -> Element<'a, Message> {
    let id = selected_layer.unwrap_or(0);
    let has_layer = selected_layer.is_some();

    let icon_btn = |codepoint: &'a str, label: &'a str, msg: Message, enabled: bool| {
        let b = button(
            row![
                text(codepoint)
                    .font(ui::icon_button::MATERIAL_ICONS)
                    .size(15)
                    .color(if enabled {
                        colors::TEXT_SECONDARY
                    } else {
                        colors::TEXT_MUTED
                    }),
                text(label).size(11).color(if enabled {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_MUTED
                }),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding(Padding::new(4.0).left(8.0).right(8.0));
        if enabled {
            b.on_press(msg).style(|_t, s| {
                let mut st = button::Style::default();
                st.background = Some(if s == button::Status::Hovered {
                    colors::HOVER_OVERLAY.into()
                } else {
                    iced::Color::TRANSPARENT.into()
                });
                st.border.radius = metrics::RADIUS_BUTTON.into();
                st
            })
        } else {
            b.style(|_t, _s| button::Style::default())
        }
    };

    // Slider d'échelle compact (réutilise SetLayerScale comme le panneau
    // Propriétés — une seule source de vérité pour le réglage)
    let scale_slider: Element<'a, Message> = match selected_scale_percent {
        Some(pct) => row![
            field_label("Échelle"),
            iced::widget::slider(5.0..=800.0, pct.clamp(5.0, 800.0), move |v| {
                Message::SetLayerScale {
                    id,
                    scale: v / 100.0,
                }
            },)
            .width(Length::Fixed(110.0)),
            value_label(format!("{:.0}%", pct)),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into(),
        None => iced::widget::Space::new()
            .width(Length::Fixed(0.0))
            .height(Length::Fixed(0.0))
            .into(),
    };

    row![
        field_label("Transformer"),
        separator(),
        icon_btn(
            ICON_ROTATE_LEFT,
            "Rotation -90°",
            Message::RotateLayer90 {
                id,
                clockwise: false
            },
            has_layer,
        ),
        icon_btn(
            ICON_ROTATE_RIGHT,
            "Rotation +90°",
            Message::RotateLayer90 {
                id,
                clockwise: true
            },
            has_layer,
        ),
        icon_btn(
            ICON_FLIP,
            "Flip H",
            Message::FlipLayer {
                id,
                horizontal: true
            },
            has_layer,
        ),
        icon_btn(
            ICON_FLIP,
            "Flip V",
            Message::FlipLayer {
                id,
                horizontal: false
            },
            has_layer,
        ),
        scale_slider,
        separator(),
        icon_btn(
            ICON_RESET,
            "Réinitialiser",
            Message::ResetLayerTransform(id),
            has_layer
        ),
        icon_btn(
            ICON_CROP,
            "Rogner à la sélection",
            Message::CropLayerToSelection,
            has_layer && has_selection,
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding(Padding::new(5.0))
    .into()
}

// ---------------------------------------------------------------------------
// Briques partagées (labels, séparateurs, valeurs)
// ---------------------------------------------------------------------------

fn field_label(s: &'static str) -> Element<'static, Message> {
    text(s)
        .size(11)
        .font(ui::theme::fonts::SANS_SEMIBOLD)
        .color(colors::TEXT_MUTED)
        .into()
}

fn value_label(s: String) -> Element<'static, Message> {
    text(s).size(11).color(colors::TEXT_SECONDARY).into()
}

fn separator<'a>() -> Element<'a, Message> {
    container(
        iced::widget::Space::new()
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(18.0)),
    )
    .style(|_| container::Style {
        background: Some(colors::BORDER_PANEL.into()),
        ..Default::default()
    })
    .into()
}
