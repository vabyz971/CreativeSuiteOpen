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

//! Ligne du panneau Calques : sélection, visibilité, expansion, miniature,
//! nom, drag & drop et pile FX/contextuel associés.

use super::{DropPosition, LayerDragState, resolve_drop_target};
use crate::Message;
use crate::layers::{FilterLayer, LayerNode};
use iced::widget::{Space, button, column, container, image, mouse_area, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};
use photo_engine::Document;
use ui_kit::theme::colors;
use uuid::Uuid;

pub(crate) const ICON_ADD: &str = "\u{e145}"; // add
pub(crate) const ICON_IMAGE: &str = "\u{e3f4}"; // image
pub(crate) const ICON_DUPLICATE: &str = "\u{e14d}"; // content_copy
pub(crate) const ICON_DELETE: &str = "\u{e872}"; // delete
pub(crate) const ICON_UP: &str = "\u{e316}"; // keyboard_arrow_up
pub(crate) const ICON_DOWN: &str = "\u{e313}"; // keyboard_arrow_down
pub(crate) const ICON_VISIBLE: &str = "\u{e8f4}"; // visibility
pub(crate) const ICON_HIDDEN: &str = "\u{e8f5}"; // visibility_off
pub(crate) const ICON_FOLDER: &str = "\u{e2c8}"; // folder_open
pub(crate) const ICON_UNGROUP: &str = "\u{e2c7}"; // folder → dégrouper
pub(crate) const ICON_ADJUST: &str = "\u{e39e}"; // filter_b_and_w → ajustement
pub(crate) const ICON_PALETTE: &str = "\u{e40a}"; // palette → couleur uni
pub(crate) const ICON_MASK: &str = "\u{e3b0}"; // mask
pub(crate) const ICON_FX: &str = "\u{e590}"; // filter_vintage → sous-calque de filtre
pub(crate) const ICON_FILTER_ADD: &str = "\u{e152}"; // filter_list → menu d'ajout de filtre
#[allow(clippy::too_many_arguments)]
pub(crate) fn node_row<'a>(
    doc: &'a Document,
    node: &'a LayerNode,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    drag: &'a LayerDragState,
    hovered: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_fx_stack: &'a std::collections::HashSet<Uuid>,
    context_menu_open: Option<Uuid>,
    layer_item_radius: f32,
    depth: usize,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let id = node.id();
    let _ = layer_item_radius;
    let is_context = context_menu_open == Some(id);

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
    let is_dragged = drag.dragged_id() == Some(id);
    let is_hovered = hovered == Some(id);
    // Poignée visuelle
    // Poignée visuelle : toute la ligne est la zone de drag, donc la
    // poignée reste un simple glyphe (pas de bouton imbriqué qui capterait
    // le pressé avant la ligne).
    let drag_grip = text("\u{e945}")
        .font(material)
        .size(14)
        .color(if is_dragged {
            colors::ACCENT
        } else {
            colors::TEXT_MUTED
        });

    // Le pressé part de la ligne entière (boutons enfants exclus : ils
    // capturent leur propre pressé). Un clic sans déplacement sélectionne
    // au relâchement, via l'abonnement global.

    // Bouton de suppression (tout à droite)
    let delete_btn = {
        let can_delete = match node {
            LayerNode::Pixel(_) => doc.pixel_count() > 1,
            LayerNode::Group(_) => true,
            LayerNode::Adjustment(_) => true,
        };
        let msg = match node {
            LayerNode::Pixel(_) => Message::DeleteLayer(id),
            LayerNode::Group(_) => Message::DeleteLayer(id),
            LayerNode::Adjustment(_) => Message::DeleteLayer(id),
        };
        button(
            text(ICON_DELETE)
                .font(material)
                .size(14)
                .color(if can_delete {
                    colors::ERROR
                } else {
                    colors::TEXT_MUTED
                }),
        )
        .padding(2)
        .style(|_t, s| ui_kit::style::ghost(s))
        .on_press_maybe(can_delete.then_some(msg))
    };

    // Survol d'un groupe pendant un drag : candidat Inside, validé moteur.
    let hover_inside = match drag {
        LayerDragState::Dragging { layer_id, .. } if matches!(node, LayerNode::Group(_)) => {
            resolve_drop_target(doc, *layer_id, id, DropPosition::Inside)
        }
        _ => None,
    };
    let inside_active = hover_inside.is_some() && drag.target() == hover_inside;

    // La ligne est un conteneur, pas un bouton : `Button::on_press` ne
    // publie qu'au relâchement, trop tard pour armer le drag. `mouse_area`
    // publie le pressé immédiatement ; la sélection a lieu au relâchement
    // global si le pointeur n'a pas bougé.
    let row_body = container(
        row![
            drag_grip,
            eye,
            leading,
            column![
                name_field,
                text(subtitle).size(10).color(colors::TEXT_MUTED)
            ]
            .spacing(1),
            Space::new().width(Length::Fill),
            delete_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(4.0).left(4.0).right(6.0))
    .width(Length::Fill)
    .style(move |_| {
        let background = if is_dragged || is_selected {
            Some(colors::BG_PANEL_HEADER_FOCUSED.into())
        } else if is_hovered && is_visible {
            Some(colors::HOVER_OVERLAY.into())
        } else {
            None
        };
        container::Style {
            background,
            border: iced::Border {
                // Rayon configuré par préférence (override du radius du style)
                radius: layer_item_radius.into(),
                width: if inside_active { 1.5 } else { 0.0 },
                color: if inside_active {
                    colors::ACCENT
                } else {
                    iced::Color::TRANSPARENT
                },
            },
            ..Default::default()
        }
    });

    // Clic gauche/droit sur la ligne → pressé drag / menu contextuel. Les
    // boutons enfants (œil, chevron, suppression, nom) capturent leur propre
    // pressé et ne démarrent donc aucun drag. Pendant un drag, le survol
    // d'un corps de groupe propose Inside ; le curseur reflète la validité
    // moteur.
    let mut row_area = mouse_area(row_body).on_right_press(Message::OpenContextMenu {
        layer_id: id,
        mouse_pos: (0.0, 0.0),
    });
    if is_visible {
        row_area = row_area.on_press(Message::LayerDragPressed { id });
    }
    row_area = row_area
        .on_enter(Message::LayerRowHovered(id))
        .on_exit(Message::LayerRowUnhovered(id));
    row_area = row_area.interaction(match drag {
        LayerDragState::Dragging { .. } => {
            if inside_active {
                iced::mouse::Interaction::Grabbing
            } else {
                iced::mouse::Interaction::NoDrop
            }
        }
        _ if is_visible => iced::mouse::Interaction::Grab,
        _ => iced::mouse::Interaction::default(),
    });

    // Indentation hiérarchique — scope drag & drop au panel calque
    let indent = 4.0 + (depth as f32) * 14.0;
    let mut stack = iced::widget::Column::new().spacing(2).push(
        container(row_area)
            .width(Length::Fill)
            .padding(Padding::new(0.0).left(indent)),
    );

    // Pile FX unifiée : un seul badge (avec compteurs masques + filtres) qui
    // déplie/replie une liste commune sous le calque — Affinity/Photoshop
    // style, plus besoin de deux sections distinctes.
    stack = stack.push(fx_stack_section(
        id,
        node,
        preview_cache,
        selected,
        active_mask,
        expanded_fx_stack,
        indent,
    ));

    // Menu contextuel déroulant (clic droit sur la ligne)
    if is_context {
        stack = stack.push(context_menu_dropdown(id, node, depth, layer_item_radius));
    }

    stack.into()
}

/// Menu contextuel déroulant — visible sous la ligne quand
/// `context_menu_open == Some(id)`. Style « dropdown Affinity » avec icônes
/// Material et séparateurs entre familles d'actions.
fn context_menu_dropdown<'a>(
    id: Uuid,
    node: &'a LayerNode,
    depth: usize,
    layer_item_radius: f32,
) -> Element<'a, Message> {
    let indent = 4.0 + (depth as f32) * 14.0 + 8.0;
    let mut col = iced::widget::Column::new().spacing(2);

    // Section 1 : masques / filtres / groupe
    let can_mask = matches!(node, LayerNode::Pixel(_) | LayerNode::Group(_));
    let can_filter = matches!(node, LayerNode::Pixel(_) | LayerNode::Adjustment(_));
    let is_group = matches!(node, LayerNode::Group(_));

    if can_mask {
        col = col.push(menu_row(
            ICON_MASK,
            "Ajouter un masque",
            Message::ContextAddMask,
        ));
    }
    if can_filter {
        col = col.push(menu_row(
            ICON_FX,
            "Ajouter un filtre",
            Message::ContextAddFilter,
        ));
    }
    col = col.push(menu_row(
        ICON_DUPLICATE,
        "Dupliquer le calque",
        Message::DuplicateLayer(id),
    ));

    // Séparateur
    if can_mask || can_filter {
        col = col.push(separator());
    }

    // Section 2 : ordre
    col = col.push(menu_row(ICON_UP, "Monter", Message::ContextMoveUp));
    col = col.push(menu_row(ICON_DOWN, "Descendre", Message::ContextMoveDown));

    // Section 3 : visibilité + groupe
    col = col.push(separator());
    let visibility_label = if node.visible() {
        "Masquer"
    } else {
        "Afficher"
    };
    col = col.push(menu_row(
        if node.visible() {
            ICON_VISIBLE
        } else {
            ICON_HIDDEN
        },
        visibility_label,
        Message::ContextToggleVisible,
    ));
    if is_group {
        col = col.push(menu_row(
            ICON_UNGROUP,
            "Dégrouper",
            Message::UngroupLayers(id),
        ));
    } else if !matches!(node, LayerNode::Adjustment(_)) {
        col = col.push(menu_row(
            ICON_FOLDER,
            "Grouper la sélection",
            Message::GroupLayers(id),
        ));
    }

    // Section 4 : suppression
    col = col.push(separator());
    col = col.push(menu_row(ICON_DELETE, "Supprimer", Message::DeleteLayer(id)));

    container(col)
        .padding(Padding::new(4.0).left(indent).right(4.0))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(colors::SURFACE_CONTAINER_HIGH.into()),
            border: iced::Border {
                radius: layer_item_radius.into(),
                width: 1.0,
                color: colors::BORDER_PANEL,
            },
            shadow: iced::Shadow {
                color: colors::BG_APP,
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            ..Default::default()
        })
        .into()
}

/// Petite ligne du menu contextuel (icône + libellé).
fn menu_row<'a>(icon: &'a str, label: &'a str, msg: Message) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    button(
        row![
            text(icon)
                .font(material)
                .size(13)
                .color(colors::TEXT_SECONDARY),
            text(label).size(11).color(colors::TEXT_PRIMARY),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(4.0).left(8.0).right(8.0))
    .width(Length::Fill)
    .style(|_t, s| ui_kit::style::menu_item(s))
    .on_press(msg)
    .into()
}

/// Trait horizontal fin pour séparer les sections du menu.
fn separator<'a>() -> Element<'a, Message> {
    Space::new()
        .height(Length::Fixed(1.0))
        .width(Length::Fill)
        .into()
}

/// Pile FX unifiée (masques + filtres) — un seul badge par calque, une
/// seule liste commune en dessous. Affinity/Photoshop style.
#[allow(clippy::too_many_arguments)]
fn fx_stack_section<'a>(
    owner_id: Uuid,
    node: &'a LayerNode,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_fx_stack: &'a std::collections::HashSet<Uuid>,
    indent: f32,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;
    let masks = node.masks();
    let filters: &[FilterLayer] = match node {
        LayerNode::Pixel(l) => &l.filter_layers,
        _ => &[],
    };
    let mask_count = masks.len();
    let filter_count = filters.len();
    if mask_count == 0 && filter_count == 0 {
        // Rien à montrer : pas de badge vide (UX).
        return Space::new().height(Length::Fixed(0.0)).into();
    }
    let is_expanded = expanded_fx_stack.contains(&owner_id);
    let mut stack = iced::widget::Column::new().spacing(2);

    // Badge unifié : compteurs masques + filtres, chevron, ToggleFxStack.
    let count_text = if filter_count > 0 && mask_count > 0 {
        format!("{mask_count} masque(s) • {filter_count} filtre(s)")
    } else if filter_count > 0 {
        format!("{filter_count} filtre(s)")
    } else {
        format!("{mask_count} masque(s)")
    };
    let badge = row![
        button(
            row![
                text(ICON_FX).font(material).size(13).color(colors::ACCENT),
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
            if mask_count + filter_count > 0 {
                ui_kit::style::ghost_selected(true, s)
            } else {
                ui_kit::style::ghost(s)
            }
        })
        .on_press(Message::ToggleFxStack(owner_id)),
        text(count_text).size(10).color(colors::TEXT_MUTED),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    stack = stack.push(
        container(badge)
            .width(Length::Fill)
            .padding(Padding::new(2.0).left(indent + 4.0)),
    );

    if is_expanded {
        // Enfants (masques puis filtres) : MÊME interface que les calques,
        // à 0,8× (vignette 38×26, texte 10/8), décalés d'un cran à droite.
        let child_indent = indent + 14.0;
        // Masques d'abord (Affinity) — toujours dans l'ordre affiché.
        for m in masks.iter() {
            let target = crate::message::MaskTarget {
                layer_id: owner_id,
                mask_id: m.id,
            };
            let is_active = active_mask.map(|t| (t.layer_id, t.mask_id)) == Some((owner_id, m.id));
            let thumb = preview_cache.mask_thumb(m.id).cloned().unwrap_or_else(|| {
                iced::widget::image::Handle::from_rgba(38, 26, vec![60, 60, 60, 255])
            });
            stack = stack.push(fx_child_row(
                m.id,
                ICON_MASK,
                Some(thumb),
                &m.name,
                if m.enabled {
                    "Actif".to_string()
                } else {
                    "Désactivé".to_string()
                },
                m.enabled,
                is_active,
                Message::ToggleLayerMaskEnabled(owner_id, m.id),
                Some(Message::SetActiveMask(Some(target))),
                Message::MoveMask {
                    owner_id,
                    mask_id: m.id,
                    up: true,
                },
                Message::MoveMask {
                    owner_id,
                    mask_id: m.id,
                    up: false,
                },
                Message::RemoveLayerMask(owner_id, m.id),
                child_indent,
            ));
        }
        // Filtres (pixels uniquement) — haut de pile en premier.
        for f in filters.iter().rev() {
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
            stack = stack.push(fx_child_row(
                f.id,
                ICON_FX,
                None,
                &f.name,
                subtitle,
                f.enabled,
                Some(f.id) == selected,
                Message::ToggleFilterEnabled {
                    layer_id: owner_id,
                    filter_id: f.id,
                },
                if f.enabled {
                    Some(Message::SelectLayer(f.id))
                } else {
                    None
                },
                Message::MoveLayerUp(f.id),
                Message::MoveLayerDown(f.id),
                Message::RemoveLiveFilter {
                    layer_id: owner_id,
                    filter_id: f.id,
                },
                child_indent,
            ));
            // Masques du sous-calque : mêmes contrôles que les masques de
            // calque (porteur = le filtre) — ajout, activation, peinture,
            // déplacement, suppression passent par les mêmes messages.
            let mask_indent = child_indent + 14.0;
            for m in f.masks.iter() {
                let target = crate::message::MaskTarget {
                    layer_id: f.id,
                    mask_id: m.id,
                };
                let is_active = active_mask.map(|t| (t.layer_id, t.mask_id)) == Some((f.id, m.id));
                let thumb = preview_cache.mask_thumb(m.id).cloned().unwrap_or_else(|| {
                    iced::widget::image::Handle::from_rgba(38, 26, vec![60, 60, 60, 255])
                });
                stack = stack.push(fx_child_row(
                    m.id,
                    ICON_MASK,
                    Some(thumb),
                    &m.name,
                    if m.enabled {
                        "Actif".to_string()
                    } else {
                        "Désactivé".to_string()
                    },
                    m.enabled,
                    is_active,
                    Message::ToggleLayerMaskEnabled(f.id, m.id),
                    Some(Message::SetActiveMask(Some(target))),
                    Message::MoveMask {
                        owner_id: f.id,
                        mask_id: m.id,
                        up: true,
                    },
                    Message::MoveMask {
                        owner_id: f.id,
                        mask_id: m.id,
                        up: false,
                    },
                    Message::RemoveLayerMask(f.id, m.id),
                    mask_indent,
                ));
            }
            stack = stack.push(
                container(
                    button(text("+ Masque").size(10).color(colors::TEXT_MUTED))
                        .padding(2)
                        .style(|_t, s| ui_kit::style::ghost(s))
                        .on_press(Message::AddLayerMask(f.id)),
                )
                .width(Length::Fill)
                .padding(Padding::new(0.0).left(mask_indent)),
            );
        }
    }

    stack.into()
}

/// Ligne enfant (masque ou filtre) — même interface que les calques mais à
/// 0,8× : œil, vignette/glyphe (38×26 = 0,8× du 48×32 parent), nom
/// renommable, sous-titre (10/8 vs 12/10), monter, descendre, supprimer.
/// Toute la ligne est cliquable = sélection (masque actif ou filtre).
#[allow(clippy::too_many_arguments)]
fn fx_child_row<'a>(
    child_id: Uuid,
    glyph: &'static str,
    thumb: Option<iced::widget::image::Handle>,
    name: &'a str,
    subtitle: String,
    enabled: bool,
    highlighted: bool,
    eye_msg: Message,
    select_msg: Option<Message>,
    up_msg: Message,
    down_msg: Message,
    delete_msg: Message,
    indent: f32,
) -> Element<'a, Message> {
    let material = ui_kit::icon_button::MATERIAL_ICONS;

    let eye = button(
        text(if enabled { ICON_VISIBLE } else { ICON_HIDDEN })
            .font(material)
            .size(12)
            .color(if enabled {
                colors::TEXT_SECONDARY
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding(2)
    .style(|_t, s| ui_kit::style::ghost(s))
    .on_press(eye_msg);

    // Vignette 0,8× (38×26) pour les masques, glyphe centré sinon.
    let leading: iced::Element<'a, Message> = match thumb {
        Some(thumb) => container(
            image(thumb)
                .width(Length::Fixed(38.0))
                .height(Length::Fixed(26.0)),
        )
        .style(|_| {
            ui_kit::style::inset_card(
                colors::SURFACE_CONTAINER_LOWEST,
                ui_kit::theme::metrics::RADIUS_SM,
            )
        })
        .into(),
        None => container(text(glyph).font(material).size(16).color(colors::ACCENT))
            .width(Length::Fixed(38.0))
            .center_x(Length::Shrink)
            .into(),
    };

    let name_field = text_input("…", name)
        .size(10)
        .padding(Padding::new(4.0).top(2.0).bottom(2.0))
        .style(|_t, s| ui_kit::style::inline_name_input(s))
        .on_input(move |s| Message::RenameLayer {
            id: child_id,
            name: s,
        });

    let small_btn = |codepoint: &'static str, msg: Message, color: iced::Color| {
        button(text(codepoint).font(material).size(11).color(color))
            .padding(2)
            .style(|_t, s| ui_kit::style::ghost(s))
            .on_press(msg)
    };

    let mut row_btn = button(
        row![
            eye,
            leading,
            column![name_field, text(subtitle).size(8).color(colors::TEXT_MUTED)].spacing(1),
            Space::new().width(Length::Fill),
            small_btn(ICON_UP, up_msg, colors::TEXT_MUTED),
            small_btn(ICON_DOWN, down_msg, colors::TEXT_MUTED),
            small_btn(ICON_DELETE, delete_msg, colors::ERROR),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::new(2.0).left(4.0).right(4.0))
    .width(Length::Fill)
    .style(move |_t, s| {
        if !enabled {
            ui_kit::style::ghost(s)
        } else {
            ui_kit::style::ghost_selected(highlighted, s)
        }
    });
    if let Some(msg) = select_msg {
        row_btn = row_btn.on_press(msg);
    }

    container(row_btn)
        .width(Length::Fill)
        .padding(Padding::new(0.0).left(indent))
        .into()
}
