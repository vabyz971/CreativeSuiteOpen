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

//! Panneau Calques façon Photoshop/Affinity :
//! - En-tête : mode de fusion + opacité du calque sélectionné
//! - Liste (haut de pile en premier) : miniature, nom, œil de visibilité
//! - Barre bas : ajouter, dupliquer, supprimer, monter/descendre

use crate::Message;
use crate::layers::{BLEND_MODES, Layer};
use iced::widget::{
    Space, button, column, container, image, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{Alignment, Element, Length, Padding};
use ui::theme::{colors, metrics};

const ICON_ADD: &str = "\u{e145}"; // add
const ICON_IMAGE: &str = "\u{e3f4}"; // image
const ICON_DUPLICATE: &str = "\u{e14d}"; // content_copy
const ICON_DELETE: &str = "\u{e872}"; // delete
const ICON_UP: &str = "\u{e316}"; // arrow_upward
const ICON_DOWN: &str = "\u{e313}"; // arrow_downward
const ICON_VISIBLE: &str = "\u{e8f4}"; // visibility
const ICON_HIDDEN: &str = "\u{e8f5}"; // visibility_off

pub fn render<'a>(
    layers: &'a [Layer],
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<u64>,
) -> Element<'a, Message> {
    let sel_layer = selected.and_then(|id| layers.iter().find(|l| l.id == id));

    // --- En-tête : mode de fusion + opacité ---
    let blend = sel_layer
        .map(|l| l.blend_mode.clone())
        .unwrap_or_else(|| "Normal".to_string());
    let opacity = sel_layer.map(|l| l.opacity).unwrap_or(100.0);

    let header = container(
        column![
            row![
                text("Fusion").size(11).color(colors::TEXT_MUTED),
                Space::new().width(Length::Fill),
                pick_list(
                    BLEND_MODES
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                    Some(blend),
                    move |m: String| {
                        Message::SetLayerBlend {
                            id: selected.unwrap_or(0),
                            mode: m,
                        }
                    },
                )
                .width(Length::Fixed(130.0))
                .placeholder("—"),
            ]
            .align_y(Alignment::Center)
            .spacing(6),
            row![
                text("Opacité").size(11).color(colors::TEXT_MUTED),
                Space::new().width(Length::Fill),
                container(
                    text(format!("{:.0} %", opacity))
                        .size(11)
                        .color(colors::TEXT_PRIMARY)
                )
                .padding(2)
                .width(Length::Fixed(44.0))
                .style(|_t| container::Style {
                    background: Some(colors::SURFACE_CONTAINER_HIGH.into()),
                    border: iced::Border {
                        radius: metrics::RADIUS_BUTTON.into(),
                        width: 1.0,
                        color: colors::BORDER_PANEL,
                    },
                    ..Default::default()
                }),
            ]
            .align_y(Alignment::Center)
            .spacing(6),
            slider(0.0..=100.0, opacity, move |v| Message::SetLayerOpacity {
                id: selected.unwrap_or(0),
                opacity: v,
            })
            .step(1.0_f32),
        ]
        .spacing(8)
        .padding(10),
    )
    .style(|_| container::Style {
        background: Some(colors::BG_MENU_BAR.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    });

    // --- Liste des calques (haut de la pile affiché en premier) ---
    let mut list = column![].spacing(2).padding(6);
    for layer in layers.iter().rev() {
        list = list.push(layer_row(layer, preview_cache, Some(layer.id) == selected));
    }
    let list_view: Element<'_, Message> = if layers.is_empty() {
        container(
            text("Aucun calque — ouvrez une image ou ajoutez un calque")
                .size(11)
                .color(colors::TEXT_MUTED),
        )
        .padding(12)
        .into()
    } else {
        scrollable(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    // --- Barre d'actions ---
    let material = ui::icon_button::MATERIAL_ICONS;
    let action_btn = |codepoint: &'a str, _tip: &'a str, msg: Message, enabled: bool| {
        let b = button(text(codepoint).font(material).size(16).color(if enabled {
            colors::TEXT_SECONDARY
        } else {
            colors::TEXT_MUTED
        }))
        .padding(4);

        if enabled {
            b.on_press(msg).style(move |_t, s| ui::style::ghost(s))
        } else {
            b.style(|_t, _s| button::Style::default())
        }
    };

    let has_sel = sel_layer.is_some();
    let actions = container(
        row![
            action_btn(
                ICON_ADD,
                "Nouveau calque vide",
                Message::AddEmptyLayer,
                true
            ),
            action_btn(
                ICON_IMAGE,
                "Calque depuis une image",
                Message::OpenImage,
                true
            ),
            action_btn(
                ICON_DUPLICATE,
                "Dupliquer",
                Message::DuplicateLayer(selected.unwrap_or(0)),
                has_sel
            ),
            action_btn(
                ICON_UP,
                "Monter",
                Message::MoveLayerUp(selected.unwrap_or(0)),
                has_sel
            ),
            action_btn(
                ICON_DOWN,
                "Descendre",
                Message::MoveLayerDown(selected.unwrap_or(0)),
                has_sel
            ),
            Space::new().width(Length::Fill),
            action_btn(
                ICON_DELETE,
                "Supprimer",
                Message::DeleteLayer(selected.unwrap_or(0)),
                has_sel && layers.len() > 1
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(6.0).left(8.0).right(8.0))
    .style(|_| container::Style {
        background: Some(colors::BG_MENU_BAR.into()),
        border: iced::Border {
            width: 1.0,
            color: colors::BORDER_PANEL,
            ..Default::default()
        },
        ..Default::default()
    });

    column![header, list_view, actions]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn layer_row<'a>(
    layer: &'a Layer,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    is_selected: bool,
) -> Element<'a, Message> {
    let material = ui::icon_button::MATERIAL_ICONS;
    let id = layer.id;

    let eye = button(
        text(if layer.visible {
            ICON_VISIBLE
        } else {
            ICON_HIDDEN
        })
        .font(material)
        .size(15)
        .color(if layer.visible {
            colors::TEXT_SECONDARY
        } else {
            colors::TEXT_MUTED
        }),
    )
    .padding(2)
    .style(|_t, s| ui::style::ghost(s))
    .on_press(Message::ToggleLayerVisible(id));

    // Cache synchronisé après chaque update ; repli neutre si absent
    let thumb_handle = preview_cache
        .thumb(layer.id)
        .cloned()
        .unwrap_or_else(|| iced::widget::image::Handle::from_rgba(1, 1, vec![0, 0, 0, 0]));
    let thumb_bg = container(
        image(thumb_handle)
            .width(Length::Fixed(48.0))
            .height(Length::Fixed(32.0)),
    )
    .style(|_| {
        ui::style::inset_card(
            colors::SURFACE_CONTAINER_LOWEST,
            ui::theme::metrics::RADIUS_SM,
        )
    });

    let name_field = text_input("Nom", &layer.name)
        .size(12)
        .padding(Padding::new(4.0).top(2.0).bottom(2.0))
        .on_input(move |s| Message::RenameLayer { id, name: s });

    let row_btn = button(
        row![
            eye,
            thumb_bg,
            column![
                name_field,
                text(format!("{} % • {}", layer.opacity as u32, layer.blend_mode))
                    .size(10)
                    .color(colors::TEXT_MUTED),
            ]
            .spacing(1),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(4.0).left(4.0).right(6.0))
    .width(Length::Fill)
    .style(move |_t, s| ui::style::ghost_selected(is_selected, s))
    .on_press(Message::SelectLayer(id));

    container(row_btn).width(Length::Fill).into()
}
