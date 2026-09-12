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
//! - Arbre (haut de pile en premier) : groupes repliables, calques pixels
//!   (avec sous-calques de filtres repliables, façon Affinity), calques
//!   d'ajustement, miniature, nom, œil de visibilité
//! - Barre bas : ajouter (dont dropdown de filtres), dupliquer,
//!   grouper/dégrouper, monter/descendre

use super::row::{
    ICON_ADD, ICON_DELETE, ICON_DUPLICATE, ICON_FILTER_ADD, ICON_IMAGE, ICON_MASK, ICON_PALETTE,
    node_row,
};
use super::{DropPosition, LayerDragState, resolve_drop_target};
use crate::Message;
use crate::layers::{BlendMode, LayerNode};
use iced::widget::{
    Space, button, column, container, mouse_area, pick_list, row, scrollable, slider, text,
};
use iced::{Alignment, Element, Length, Padding};
use photo_engine::Document;
use ui_kit::theme::{colors, metrics};
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    doc: &'a Document,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    drag: &'a LayerDragState,
    hovered: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_fx_stack: &'a std::collections::HashSet<Uuid>,
    filter_menu_open: bool,
    context_menu_open: Option<Uuid>,
    layer_item_radius: f32,
) -> Element<'a, Message> {
    let sel_node = selected.and_then(|id| doc.find(id));
    let sel_filter = selected.and_then(|id| doc.find_filter_layer(id));

    // --- En-tête : mode de fusion + opacité (nœud OU sous-calque) ---
    let blend = selected
        .and_then(|id| doc.blend_of(id))
        .unwrap_or(BlendMode::Normal);
    let opacity = selected.and_then(|id| doc.opacity_of(id)).unwrap_or(100.0);
    let has_sel = sel_node.is_some() || sel_filter.is_some();

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
                container(
                    text(format!("{:.0} %", opacity))
                        .size(11)
                        .color(colors::TEXT_PRIMARY)
                )
                .padding(2)
                .width(Length::Fixed(48.0))
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
        background: Some(colors::BG_TRANSPARENT.into()),
        ..Default::default()
    });

    // --- Arbre des calques (haut de la pile affiché en premier) ---
    let list = tree_column(
        doc,
        &doc.root,
        preview_cache,
        selected,
        drag,
        hovered,
        active_mask,
        expanded_fx_stack,
        context_menu_open,
        layer_item_radius,
        0,
    )
    .padding(6);
    let list_view: Element<'_, Message> = if doc.root.is_empty() {
        container(
            text("Aucun calque — ouvrez une image ou ajoutez un calque")
                .size(11)
                .color(colors::TEXT_MUTED),
        )
        .padding(12)
        .into()
    } else {
        // Quitter la liste efface la cible : un relâchement hors liste
        // abandonne au lieu de valider une ancienne cible.
        mouse_area(scrollable(list).width(Length::Fill).height(Length::Fill))
            .on_exit(Message::LayerDragHover {
                hovered: None,
                position: DropPosition::Before,
            })
            .into()
    };

    // --- Barre d'actions (mini toolbar en bas) ---
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let action_btn = |codepoint: &'a str, _tip: &'a str, msg: Message, enabled: bool| {
        let b = button(text(codepoint).font(material).size(16).color(if enabled {
            colors::TEXT_SECONDARY
        } else {
            colors::TEXT_MUTED
        }))
        .padding(4);

        if enabled {
            b.on_press(msg).style(move |_t, s| {
                let mut st = ui_kit::style::ghost(s);
                st.border.radius = layer_item_radius.into();
                st
            })
        } else {
            b.style(|_t, _s| button::Style::default())
        }
    };

    // Le nœud sélectionné est-il déjà un groupe ? (grouper/dégrouper)
    let sel_filter_parent = selected.and_then(|sid| doc.find_filter_parent(sid));
    let sel_is_filter = sel_filter_parent.is_some();
    // Cible d'ajout de filtre : pixels et ajustements (un sous-calque
    // sélectionné redirige vers son calque porteur).
    let filter_target = match selected {
        Some(sid) if sel_is_filter => doc.find_filter_parent(sid),
        Some(sid) => match doc.find(sid) {
            Some(LayerNode::Pixel(_) | LayerNode::Adjustment(_)) => Some(sid),
            _ => None,
        },
        None => None,
    };
    let can_have_mask =
        matches!(sel_node, Some(LayerNode::Pixel(_) | LayerNode::Group(_))) || sel_is_filter;
    let nil = Uuid::nil();
    // Suppression : sous-calque → RemoveLiveFilter, sinon DeleteLayer.
    let (delete_msg, delete_enabled) = match (selected, sel_filter_parent) {
        (Some(fid), Some(parent)) => (
            Message::RemoveLiveFilter {
                layer_id: parent,
                filter_id: fid,
            },
            true,
        ),
        (Some(sid), None) => (Message::DeleteLayer(sid), has_sel && doc.pixel_count() > 1),
        _ => (Message::DeleteLayer(nil), false),
    };
    let mini_bar = container(
        row![
            action_btn(ICON_IMAGE, "Ouvrir une image", Message::OpenImage, true),
            action_btn(
                ICON_ADD,
                "Nouveau calque vide",
                Message::AddEmptyLayer,
                true
            ),
            action_btn(
                ICON_DUPLICATE,
                "Dupliquer",
                Message::DuplicateLayer(selected.unwrap_or(nil)),
                has_sel
            ),
            action_btn(
                ICON_PALETTE,
                "Nouveau calque uni",
                Message::AddSolidColorLayer,
                true
            ),
            Space::new().width(Length::Fill),
            action_btn(
                ICON_MASK,
                "Ajouter un masque",
                Message::AddLayerMask(selected.unwrap_or(nil)),
                can_have_mask
            ),
            filter_add_button(filter_target.is_some()),
            action_btn(ICON_DELETE, "Supprimer", delete_msg, delete_enabled),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(6.0).left(8.0).right(8.0));

    // Le MouseArea englobe tout le panneau : clic n'importe où (hors menu
    // ouvert via capture interne) → ferme le menu contextuel et le menu
    // filtre. Les boutons du menu contextuel court-circuitent via leur
    // propre `.on_press` qui consomme l'événement avant qu'il ne remonte.
    let content = column![
        header,
        list_view,
        mini_bar,
        filter_add_menu(filter_target, filter_menu_open)
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    mouse_area(content)
        .on_press(Message::CloseContextMenu)
        .into()
}

/// Bouton d'ajout de filtre (ouvre le menu ci-dessous).
fn filter_add_button(enabled: bool) -> Element<'static, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let b = button(
        text(ICON_FILTER_ADD)
            .font(material)
            .size(16)
            .color(if enabled {
                colors::TEXT_SECONDARY
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding(4);
    if enabled {
        b.on_press(Message::ToggleFilterMenu)
            .style(move |_t, s| ui_kit::style::ghost(s))
    } else {
        b.style(|_t, _s| button::Style::default())
    }
    .into()
}

/// Menu d'ajout de filtre (effets image→image du registre) — visible quand
/// le bouton est activé. Choisir referme le menu (côté handler).
fn filter_add_menu(target: Option<Uuid>, open: bool) -> Element<'static, Message> {
    if !open {
        return Space::new().height(Length::Fixed(0.0)).into();
    }
    let Some(tid) = target else {
        return Space::new().height(Length::Fixed(0.0)).into();
    };
    let defs = photo_engine::filterable_types();
    let mut list = iced::widget::Column::new().spacing(2);
    for d in defs {
        let type_id = d.type_id.clone();
        list = list.push(
            button(
                text(format!("+ {}", d.name))
                    .size(11)
                    .color(colors::TEXT_SECONDARY),
            )
            .padding(4)
            .width(Length::Fill)
            .style(|_t, s| ui_kit::style::ghost(s))
            .on_press(Message::AddLiveFilter { id: tid, type_id }),
        );
    }
    container(list.padding(Padding::new(4.0).left(8.0).right(8.0)))
        .style(|_| container::Style {
            background: Some(colors::BG_TRANSPARENT.into()),
            ..Default::default()
        })
        .into()
}

/// Trait d'insertion Before/After : 6 px de zone de survol, ligne ACCENT de
/// 2 px quand la cible moteur est active. Les cibles invalides restent
/// transparentes mais émettent `None` pour effacer la précédente.
fn drop_strip<'a>(
    doc: &Document,
    drag: &LayerDragState,
    hovered: Uuid,
    position: DropPosition,
    depth: usize,
) -> Element<'a, Message> {
    let target = match drag {
        LayerDragState::Dragging { layer_id, .. } => {
            resolve_drop_target(doc, *layer_id, hovered, position)
        }
        _ => None,
    };
    let active = target.is_some() && drag.target() == target;
    let line = if active {
        container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::ACCENT.into()),
                ..Default::default()
            })
    } else {
        container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    };
    let indent = 4.0 + (depth as f32) * 14.0;
    mouse_area(
        container(line)
            .width(Length::Fill)
            .height(Length::Fixed(6.0))
            .padding(Padding::new(0.0).top(2.0).left(indent)),
    )
    .on_enter(Message::LayerDragHover {
        hovered: Some(hovered),
        position,
    })
    .interaction(if target.is_some() {
        iced::mouse::Interaction::Grabbing
    } else {
        iced::mouse::Interaction::NoDrop
    })
    .into()
}

/// Construit la colonne d'une portée (haut-de-pile d'abord) ; les groupes
/// dépliés imbriquent récursivement leurs enfants avec indentation.
/// Pendant un drag uniquement : un trait d'insertion précède chaque ligne et
/// suit la dernière, pour des cibles Before/After explicites.
#[allow(clippy::too_many_arguments)]
fn tree_column<'a>(
    doc: &'a Document,
    nodes: &'a [LayerNode],
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    drag: &'a LayerDragState,
    hovered: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_fx_stack: &'a std::collections::HashSet<Uuid>,
    context_menu_open: Option<Uuid>,
    layer_item_radius: f32,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    let mut list = iced::widget::Column::new().spacing(2);
    for node in nodes.iter().rev() {
        if drag.is_dragging() {
            list = list.push(drop_strip(
                doc,
                drag,
                node.id(),
                DropPosition::Before,
                depth,
            ));
        }
        list = list.push(node_row(
            doc,
            node,
            preview_cache,
            selected,
            drag,
            hovered,
            active_mask,
            expanded_fx_stack,
            context_menu_open,
            layer_item_radius,
            depth,
        ));
        if let LayerNode::Group(g) = node
            && !g.collapsed
        {
            list = list.push(tree_column(
                doc,
                &g.children,
                preview_cache,
                selected,
                drag,
                hovered,
                active_mask,
                expanded_fx_stack,
                context_menu_open,
                layer_item_radius,
                depth + 1,
            ));
        }
    }
    if drag.is_dragging()
        && let Some(last) = nodes.first()
    {
        list = list.push(drop_strip(doc, drag, last.id(), DropPosition::After, depth));
    }
    list
}
