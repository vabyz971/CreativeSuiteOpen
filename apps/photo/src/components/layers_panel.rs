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

use crate::Message;
use crate::layers::{BlendMode, FilterLayer, LayerNode};
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
const ICON_MASK: &str = "\u{e3b0}"; // mask
const ICON_FX: &str = "\u{e590}"; // filter_vintage → sous-calque de filtre
const ICON_FILTER_ADD: &str = "\u{e152}"; // filter_list → menu d'ajout de filtre

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    doc: &'a Document,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_masks: &'a std::collections::HashSet<Uuid>,
    expanded_filters: &'a std::collections::HashSet<Uuid>,
    filter_menu_open: bool,
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
        &doc.root,
        preview_cache,
        selected,
        dragged,
        active_mask,
        expanded_masks,
        expanded_filters,
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
                has_sel && !sel_is_group && !sel_is_filter
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
            action_btn(
                ICON_MASK,
                "Ajouter un masque",
                Message::AddLayerMask(selected.unwrap_or(nil)),
                can_have_mask
            ),
            filter_add_button(filter_target.is_some()),
            Space::new().width(Length::Fill),
            action_btn(ICON_DELETE, "Supprimer", delete_msg, delete_enabled),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(6.0).left(8.0).right(8.0))
    .style(|_| container::Style {
        background: Some(colors::BG_TRANSPARENT.into()),
        ..Default::default()
    });

    column![
        header,
        list_view,
        actions,
        filter_add_menu(filter_target, filter_menu_open)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
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

/// Construit la colonne d'une portée (haut-de-pile d'abord) ; les groupes
/// dépliés imbriquent récursivement leurs enfants avec indentation.
#[allow(clippy::too_many_arguments)]
fn tree_column<'a>(
    nodes: &'a [LayerNode],
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_masks: &'a std::collections::HashSet<Uuid>,
    expanded_filters: &'a std::collections::HashSet<Uuid>,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    let mut list = iced::widget::Column::new().spacing(2);
    for node in nodes.iter().rev() {
        list = list.push(node_row(
            node,
            preview_cache,
            selected,
            dragged,
            active_mask,
            expanded_masks,
            expanded_filters,
            depth,
        ));
        if let LayerNode::Group(g) = node
            && !g.collapsed
        {
            list = list.push(tree_column(
                &g.children,
                preview_cache,
                selected,
                dragged,
                active_mask,
                expanded_masks,
                expanded_filters,
                depth + 1,
            ));
        }
    }
    list
}

#[allow(clippy::too_many_arguments)]
fn node_row<'a>(
    node: &'a LayerNode,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_masks: &'a std::collections::HashSet<Uuid>,
    expanded_filters: &'a std::collections::HashSet<Uuid>,
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

    // Champ de nom avec style inline (transparent sauf au focus)
    let name_field = text_input("Nom", node.name())
        .size(12)
        .padding(Padding::new(4.0).top(2.0).bottom(2.0))
        .style(|_t, s| ui_kit::style::inline_name_input(s))
        .on_input(move |s| Message::RenameLayer { id, name: s });

    // Sous-titre par type : opacité • fusion | Ajustement • filtres
    let subtitle = match node {
        LayerNode::Pixel(l) => {
            let filters = if l.filter_layers.is_empty() {
                String::new()
            } else {
                format!(" • {} filtre(s)", l.filter_layers.len())
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
    let mut stack = iced::widget::Column::new().spacing(2).push(
        container(row_btn)
            .width(Length::Fill)
            .padding(Padding::new(0.0).left(indent)),
    );

    // Badge masque dépliable : un calque masquable affiche un badge avec le
    // nombre de masques ; cliquer déplie la liste (miniature + actions).
    let can_mask = matches!(node, LayerNode::Pixel(_) | LayerNode::Group(_));
    if can_mask {
        stack = stack.push(mask_section(
            id,
            node.masks(),
            preview_cache,
            active_mask,
            expanded_masks,
            indent,
        ));
    }

    // Sous-calques de filtres (pixels uniquement) : badge fx + lignes
    // imbriquées façon dossier Affinity.
    if let LayerNode::Pixel(l) = node
        && !l.filter_layers.is_empty()
    {
        let fx_expanded = expanded_filters.contains(&id);
        let fx_badge = row![
            button(
                row![
                    text(ICON_FX).font(material).size(13).color(colors::ACCENT),
                    text(if fx_expanded { ICON_UP } else { ICON_DOWN })
                        .font(material)
                        .size(11)
                        .color(colors::TEXT_MUTED),
                ]
                .align_y(Alignment::Center)
                .spacing(2),
            )
            .padding(2)
            .style(|_t, s| ui_kit::style::ghost_selected(true, s))
            .on_press(Message::ToggleFilterList(id)),
            text(format!("{} filtre(s)", l.filter_layers.len()))
                .size(10)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        stack = stack.push(
            container(fx_badge)
                .width(Length::Fill)
                .padding(Padding::new(2.0).left(indent + 4.0)),
        );

        if fx_expanded {
            // Haut de pile d'abord (dernier appliqué en premier à l'écran).
            for f in l.filter_layers.iter().rev() {
                stack = stack.push(filter_row(
                    id,
                    f,
                    preview_cache,
                    selected,
                    dragged,
                    active_mask,
                    expanded_masks,
                    depth + 1,
                ));
            }
        }
    }

    stack.into()
}

/// Badge + liste dépliable des masques d'un porteur (nœud ou sous-calque
/// de filtre — `owner_id` est l'id du porteur).
#[allow(clippy::too_many_arguments)]
fn mask_section<'a>(
    owner_id: Uuid,
    masks: &'a [photo_engine::LayerMask],
    preview_cache: &'a crate::ui_handles::PreviewCache,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_masks: &'a std::collections::HashSet<Uuid>,
    indent: f32,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let count = masks.len();
    let is_expanded = expanded_masks.contains(&owner_id);
    let mut stack = iced::widget::Column::new().spacing(2);
    let badge = row![
        button(
            row![
                text(ICON_MASK)
                    .font(material)
                    .size(13)
                    .color(colors::TEXT_MUTED),
                text(if is_expanded { ICON_UP } else { ICON_DOWN })
                    .font(material)
                    .size(11)
                    .color(colors::TEXT_MUTED),
            ]
            .align_y(Alignment::Center)
            .spacing(2),
        )
        .padding(2)
        .style(move |_t, s| {
            if count > 0 {
                ui_kit::style::ghost_selected(true, s)
            } else {
                ui_kit::style::ghost(s)
            }
        })
        .on_press(Message::ToggleMaskList(owner_id)),
        text(format!("{count} masque(s)"))
            .size(10)
            .color(colors::TEXT_MUTED),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    stack = stack.push(
        container(badge)
            .width(Length::Fill)
            .padding(Padding::new(2.0).left(indent + 4.0)),
    );

    if is_expanded {
        for (idx, m) in masks.iter().enumerate() {
            let target = crate::message::MaskTarget {
                layer_id: owner_id,
                mask_id: m.id,
            };
            let is_active = active_mask.map(|t| (t.layer_id, t.mask_id)) == Some((owner_id, m.id));
            let thumb = preview_cache.mask_thumb(m.id).cloned().unwrap_or_else(|| {
                iced::widget::image::Handle::from_rgba(36, 24, vec![60, 60, 60, 255])
            });
            let mask_row = row![
                image(thumb)
                    .width(Length::Fixed(36.0))
                    .height(Length::Fixed(24.0)),
                button(
                    text(format!("Masque {}", idx + 1))
                        .size(11)
                        .color(colors::TEXT_SECONDARY),
                )
                .padding(2)
                .style(move |_t, s| {
                    if is_active {
                        ui_kit::style::ghost_selected(true, s)
                    } else {
                        ui_kit::style::ghost(s)
                    }
                })
                .on_press(Message::SetActiveMask(Some(target))),
                button(
                    text(if m.enabled { ICON_VISIBLE } else { ICON_HIDDEN })
                        .font(material)
                        .size(13)
                        .color(colors::TEXT_SECONDARY),
                )
                .padding(2)
                .style(|_t, s| ui_kit::style::ghost(s))
                .on_press(Message::ToggleLayerMaskEnabled(owner_id, m.id)),
                button(
                    text(ICON_DELETE)
                        .font(material)
                        .size(13)
                        .color(colors::ERROR),
                )
                .padding(2)
                .style(|_t, s| ui_kit::style::ghost(s))
                .on_press(Message::RemoveLayerMask(owner_id, m.id)),
            ]
            .spacing(4)
            .align_y(Alignment::Center);
            stack = stack.push(
                container(mask_row)
                    .width(Length::Fill)
                    .padding(Padding::new(2.0).left(indent + 14.0)),
            );
        }
        let add_btn = button(
            text("+ Ajouter un masque")
                .size(10)
                .color(colors::TEXT_SECONDARY),
        )
        .padding(2)
        .style(|_t, s| ui_kit::style::ghost(s))
        .on_press(Message::AddLayerMask(owner_id));
        stack = stack.push(
            container(add_btn)
                .width(Length::Fill)
                .padding(Padding::new(2.0).left(indent + 14.0)),
        );
    }
    stack.into()
}

/// Ligne d'un sous-calque de filtre : icône fx, nom, œil, suppression —
/// pas de poignée de drag (réordre via Monter/Descendre), pas de drop.
#[allow(clippy::too_many_arguments)]
fn filter_row<'a>(
    parent_id: Uuid,
    f: &'a FilterLayer,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    dragged: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_masks: &'a std::collections::HashSet<Uuid>,
    depth: usize,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let fid = f.id;

    let eye = button(
        text(if f.enabled { ICON_VISIBLE } else { ICON_HIDDEN })
            .font(material)
            .size(14)
            .color(if f.enabled {
                colors::TEXT_SECONDARY
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding(2)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::ToggleFilterEnabled {
        layer_id: parent_id,
        filter_id: fid,
    });

    let leading = container(text(ICON_FX).font(material).size(18).color(colors::ACCENT))
        .width(Length::Fixed(40.0))
        .center_x(Length::Shrink);

    let name_field = text_input("Filtre", &f.name)
        .size(11)
        .padding(Padding::new(4.0).top(2.0).bottom(2.0))
        .style(|_t, s| ui_kit::style::inline_name_input(s))
        .on_input(move |s| Message::RenameLayer { id: fid, name: s });

    let subtitle = format!(
        "{} % • {}{}",
        f.opacity as u32,
        f.blend_mode.label(),
        if f.masks.is_empty() {
            String::new()
        } else {
            format!(" • {} masque(s)", f.masks.len())
        }
    );

    let is_selected = Some(fid) == selected;
    let remove = button(
        text(ICON_DELETE)
            .font(material)
            .size(13)
            .color(colors::TEXT_MUTED),
    )
    .padding(2)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(Message::RemoveLiveFilter {
        layer_id: parent_id,
        filter_id: fid,
    });

    // Pas de drop sur un sous-calque ; clic = sélection (si actif).
    let row_action = if dragged.is_some() {
        None
    } else if f.enabled {
        Some(Message::SelectLayer(fid))
    } else {
        None
    };

    let mut row_btn = button(
        row![
            eye,
            leading,
            column![
                name_field,
                text(subtitle).size(10).color(colors::TEXT_MUTED)
            ]
            .spacing(1),
            remove,
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(4.0).left(4.0).right(6.0))
    .width(Length::Fill)
    .style(move |_t, s| {
        if !f.enabled {
            ui_kit::style::ghost(s)
        } else {
            ui_kit::style::ghost_selected(is_selected, s)
        }
    });
    if let Some(msg) = row_action {
        row_btn = row_btn.on_press(msg);
    }

    let indent = 4.0 + (depth as f32) * 14.0;
    let mut stack = iced::widget::Column::new().spacing(2).push(
        container(row_btn)
            .width(Length::Fill)
            .padding(Padding::new(0.0).left(indent)),
    );
    // Masques propres du sous-calque (même badge que les nœuds).
    stack = stack.push(mask_section(
        fid,
        &f.masks,
        preview_cache,
        active_mask,
        expanded_masks,
        indent,
    ));
    stack.into()
}
