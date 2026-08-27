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

//! Panneau Calques façon Affinity (arbre hiérarchique) :
//! - En-tête : mode de fusion + opacité du nœud sélectionné
//! - Arbre (haut de pile en premier) : groupes repliables, calques pixels,
//!   calques d'ajustement, miniature, nom, œil de visibilité
//! - Barre bas : ajouter, dupliquer, grouper/dégrouper, monter/descendre

use crate::Message;
use crate::layers::{BlendMode, LayerNode};
use iced::widget::{
    Space, button, column, container, image, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{Alignment, Element, Length, Padding};
use photo_engine::Document;
use ui_kit::theme::{colors, metrics};
use uuid::Uuid;

const ICON_ADD: &str = "\u{e145}"; // add
const ICON_IMAGE: &str = "\u{e3f4}"; // image
const ICON_DUPLICATE: &str = "\u{e14d}"; // content_copy
const ICON_DELETE: &str = "\u{e872}"; // delete
const ICON_UP: &str = "\u{e316}"; // keyboard_arrow_up
const ICON_DOWN: &str = "\u{e313}"; // keyboard_arrow_down
const ICON_VISIBLE: &str = "\u{e8f4}"; // visibility
const ICON_HIDDEN: &str = "\u{e8f5}"; // visibility_off
const ICON_FOLDER: &str = "\u{e2c8}"; // folder_open
const ICON_GROUP: &str = "\u{e2cc}"; // create_new_folder
const ICON_ADJUST: &str = "\u{e39e}"; // filter_b_and_w → ajustement
const ICON_PALETTE: &str = "\u{e40a}"; // palette → couleur uni

pub fn render<'a>(
    doc: &'a Document,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
) -> Element<'a, Message> {
    let sel_node = selected.and_then(|id| doc.find(id));

    // --- En-tête : mode de fusion + opacité ---
    let blend = sel_node
        .and_then(|n| n.blend_mode())
        .unwrap_or(BlendMode::Normal);
    let opacity = sel_node.map(|n| n.opacity()).unwrap_or(100.0);
    let has_sel = sel_node.is_some();

    let header = container(
        column![
            row![
                text("Fusion").size(11).color(colors::TEXT_MUTED),
                Space::new().width(Length::Fill),
                pick_list(BlendMode::ALL, Some(blend), move |m: BlendMode| {
                    Message::SetLayerBlend {
                        id: selected.unwrap_or_else(Uuid::nil),
                        mode: m,
                    }
                },)
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
                id: selected.unwrap_or_else(Uuid::nil),
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

    // --- Arbre des calques (haut de la pile affiché en premier) ---
    let list = tree_column(&doc.root, preview_cache, selected, dragged, 0).padding(6);
    let list_view: Element<'_, Message> = if doc.root.is_empty() {
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
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let action_btn = |codepoint: &'a str, _tip: &'a str, msg: Message, enabled: bool| {
        let b = button(text(codepoint).font(material).size(16).color(if enabled {
            colors::TEXT_SECONDARY
        } else {
            colors::TEXT_MUTED
        }))
        .padding(4);

        if enabled {
            b.on_press(msg).style(move |_t, s| ui_kit::style::ghost(s))
        } else {
            b.style(|_t, _s| button::Style::default())
        }
    };

    // Le nœud sélectionné est-il déjà un groupe ? (grouper/dégrouper)
    let sel_is_group = matches!(sel_node, Some(LayerNode::Group(_)));
    let nil = Uuid::nil();
    let actions = container(
        row![
            action_btn(
                ICON_ADD,
                "Nouveau calque vide",
                Message::AddEmptyLayer,
                true
            ),
            action_btn(
                ICON_PALETTE,
                "Calque couleur uni",
                Message::AddSolidColorLayer,
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
                Message::DuplicateLayer(selected.unwrap_or(nil)),
                has_sel
            ),
            action_btn(
                ICON_GROUP,
                "Grouper la sélection",
                Message::GroupLayers(selected.unwrap_or(nil)),
                has_sel && !sel_is_group
            ),
            action_btn(
                ICON_FOLDER,
                "Dissoudre le groupe",
                Message::UngroupLayers(selected.unwrap_or(nil)),
                sel_is_group
            ),
            action_btn(
                ICON_UP,
                "Monter",
                Message::MoveLayerUp(selected.unwrap_or(nil)),
                has_sel
            ),
            action_btn(
                ICON_DOWN,
                "Descendre",
                Message::MoveLayerDown(selected.unwrap_or(nil)),
                has_sel
            ),
            Space::new().width(Length::Fill),
            action_btn(
                ICON_DELETE,
                "Supprimer",
                Message::DeleteLayer(selected.unwrap_or(nil)),
                has_sel && doc.pixel_count() > 1
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

/// Construit la colonne d'une portée (haut-de-pile d'abord) ; les groupes
/// dépliés imbriquent récursivement leurs enfants avec indentation.
fn tree_column<'a>(
    nodes: &'a [LayerNode],
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    let mut list = iced::widget::Column::new().spacing(2);
    for node in nodes.iter().rev() {
        list = list.push(node_row(node, preview_cache, selected, dragged, depth));
        if let LayerNode::Group(g) = node
            && !g.collapsed
        {
            list = list.push(tree_column(
                &g.children,
                preview_cache,
                selected,
                dragged,
                depth + 1,
            ));
        }
    }
    list
}

fn node_row<'a>(
    node: &'a LayerNode,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    depth: usize,
) -> Element<'a, Message> {
    let _ = dragged;
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let id = node.id();

    // Œil de visibilité (commun à tous les types)
    let eye = button(
        text(if node.visible() {
            ICON_VISIBLE
        } else {
            ICON_HIDDEN
        })
        .font(material)
        .size(15)
        .color(if node.visible() {
            colors::TEXT_SECONDARY
        } else {
            colors::TEXT_MUTED
        }),
    )
    .padding(2)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleLayerVisible(id));

    // Chevron de repli pour les groupes, vignette sinon
    let leading: iced::Element<'a, Message> = match node {
        LayerNode::Group(g) => {
            let chevron = if g.collapsed { ICON_DOWN } else { ICON_UP };
            container(
                button(
                    text(chevron)
                        .font(material)
                        .size(15)
                        .color(colors::TEXT_MUTED),
                )
                .padding(2)
                .style(|_t: &iced::Theme, s| ui_kit::style::ghost(s))
                .on_press(Message::ToggleGroupCollapsed(id)),
            )
            .into()
        }
        LayerNode::Pixel(l) => {
            // Cache synchronisé après chaque update ; repli neutre si absent
            let thumb_handle = preview_cache
                .thumb(l.id)
                .cloned()
                .unwrap_or_else(|| iced::widget::image::Handle::from_rgba(1, 1, vec![0, 0, 0, 0]));
            container(
                image(thumb_handle)
                    .width(Length::Fixed(48.0))
                    .height(Length::Fixed(32.0)),
            )
            .style(|_| {
                ui_kit::style::inset_card(
                    colors::SURFACE_CONTAINER_LOWEST,
                    ui_kit::theme::metrics::RADIUS_SM,
                )
            })
            .into()
        }
        LayerNode::Adjustment(_) => container(
            text(ICON_ADJUST)
                .font(material)
                .size(20)
                .color(colors::ACCENT),
        )
        .width(Length::Fixed(52.0))
        .center_x(Length::Shrink)
        .into(),
    };

    let name_field = text_input("Nom", node.name())
        .size(12)
        .padding(Padding::new(4.0).top(2.0).bottom(2.0))
        .on_input(move |s| Message::RenameLayer { id, name: s });

    // Sous-titre par type : opacité • fusion | Ajustement • filtres
    let subtitle = match node {
        LayerNode::Pixel(l) => {
            let filters = if l.live_filters.is_empty() {
                String::new()
            } else {
                format!(" • {} filtre(s)", l.live_filters.len())
            };
            format!(
                "{} % • {}{}",
                l.opacity as u32,
                l.blend_mode.label(),
                filters
            )
        }
        LayerNode::Group(g) => {
            format!("{} % • {} • groupe", g.opacity as u32, g.blend_mode.label())
        }
        LayerNode::Adjustment(a) => {
            format!(
                "Ajustement • {} % • {} filtre(s)",
                a.opacity as u32,
                a.filters.len()
            )
        }
    };

    let is_visible = node.visible();
    let is_selected = Some(id) == selected;
    let is_dragged = dragged == Some(id);
    let drag_handle = button(
        text("\u{e945}")
            .font(material)
            .size(14)
            .color(if is_dragged {
                colors::ACCENT
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding(2)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::SetDraggedLayer(id));

    // Si un drag est en cours, cliquer sur une autre ligne = drop avant celle-ci
    let row_action = if let Some(dragged_id) = dragged {
        if dragged_id != id {
            Some(Message::DropLayerOn(id))
        } else {
            // Cliquer sur la source annule le drag
            Some(Message::SetDraggedLayer(id))
        }
    } else if is_visible {
        Some(Message::SelectLayer(id))
    } else {
        None
    };

    let mut row_btn = button(
        row![
            drag_handle,
            eye,
            leading,
            column![
                name_field,
                text(subtitle).size(10).color(colors::TEXT_MUTED)
            ]
            .spacing(1),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(4.0).left(4.0).right(6.0))
    .width(Length::Fill)
    .style(move |_t, s| {
        if is_dragged {
            ui_kit::style::ghost_selected(true, s)
        } else if !is_visible {
            ui_kit::style::ghost(s)
        } else {
            ui_kit::style::ghost_selected(is_selected, s)
        }
    });
    if let Some(msg) = row_action {
        row_btn = row_btn.on_press(msg);
    }

    // Indentation hiérarchique — scope drag & drop au panel calque
    let indent = 4.0 + (depth as f32) * 14.0;
    container(row_btn)
        .width(Length::Fill)
        .padding(Padding::new(0.0).left(indent))
        .into()
}
