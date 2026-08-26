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

//! Panneau Propriétés : réglages du nœud sélectionné.
//! Calque pixels : nom, opacité, fusion, transform, infos source ET chaîne
//! de live filters (ajout depuis le registre, réglages, activation).
//! Ajustement : opacité + sa chaîne de filtres. Groupe : opacité/fusion.

use crate::Message;
use crate::layers::{BlendMode, LayerNode};
use datatypes::ParamValue;
use iced::widget::{Space, column, container, row, scrollable, slider, text, text_input};
use iced::{Alignment, Element, Length, Padding};
use photo_engine::{Document, FilterNode};
use ui_kit::theme::{colors, fonts};

pub fn render<'a>(doc: &'a Document, selected: Option<uuid::Uuid>) -> Element<'a, Message> {
    let node = selected.and_then(|id| doc.find(id));
    let Some(node) = node else {
        return container(
            column![
                text("Propriétés")
                    .size(14)
                    .font(fonts::SANS_SEMIBOLD)
                    .color(colors::TEXT_PRIMARY),
                Space::new().height(Length::Fixed(12.0)),
                text("Aucun calque sélectionné")
                    .size(13)
                    .color(colors::TEXT_SECONDARY),
                Space::new().height(Length::Fixed(8.0)),
                text("Sélectionnez un calque dans le panneau Calques")
                    .size(11)
                    .color(colors::TEXT_MUTED),
            ]
            .padding(12),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    };

    let id = node.id();

    let header = {
        let name_field: Element<'_, Message> = text_input("Nom du calque", node.name())
            .size(14)
            .padding(4)
            .on_input(move |s| Message::RenameLayer { id, name: s })
            .into();
        let kind = match node {
            LayerNode::Pixel(_) => "Calque pixels",
            LayerNode::Group(_) => "Groupe",
            LayerNode::Adjustment(_) => "Calque d'ajustement",
        };
        let short_id = id.simple().to_string();
        let short = if short_id.len() > 8 {
            &short_id[..8]
        } else {
            &short_id
        };
        container(
            column![
                name_field,
                text(format!("{kind} · {short}"))
                    .size(11)
                    .color(colors::TEXT_MUTED),
            ]
            .spacing(2),
        )
    }
    .padding(12)
    .style(|_t| container::Style {
        background: Some(colors::BG_NODE_HEADER.into()),
        ..Default::default()
    });

    // --- Paramètres communs ---
    let mut common = column![
        param_slider("Opacité", node.opacity(), 0.0..=100.0, 1.0, move |v| {
            Message::SetLayerOpacity { id, opacity: v }
        }),
        blend_mode_buttons(node.blend_mode(), id),
    ]
    .spacing(10);

    // --- Spécifique au type de nœud ---
    match node {
        LayerNode::Pixel(l) => {
            common = common
                .push(offset_row("Décalage X", l.transform.offset_x, move |v| {
                    Message::SetLayerOffset {
                        id,
                        axis: crate::OffsetAxis::X,
                        value: v,
                    }
                }))
                .push(offset_row("Décalage Y", l.transform.offset_y, move |v| {
                    Message::SetLayerOffset {
                        id,
                        axis: crate::OffsetAxis::Y,
                        value: v,
                    }
                }))
                .push(param_slider(
                    "Échelle (%)",
                    l.transform.scale * 100.0,
                    5.0..=800.0,
                    1.0,
                    move |v| Message::SetLayerScale {
                        id,
                        scale: v / 100.0,
                    },
                ))
                .push(Space::new().height(Length::Fixed(10.0)))
                .push(source_info(l.dimensions(), &l.source_image));
        }
        LayerNode::Group(g) => {
            let _ = g;
        }
        LayerNode::Adjustment(a) => {
            let _ = a;
        }
    }

    let mut content = column![header, container(column![common].padding(12))];

    // --- Chaîne de filtres (pixels et ajustements) ---
    if let Some(filters) = node.filters() {
        let mut section = column![
            text(if matches!(node, LayerNode::Adjustment(_)) {
                "Filtres d'ajustement"
            } else {
                "Live filters (non destructif)"
            })
            .size(12)
            .color(colors::ON_SURFACE),
            Space::new().height(Length::Fixed(6.0)),
            add_filter_pick(id),
        ]
        .spacing(6);
        for f in filters.iter().rev() {
            section = section.push(filter_card(id, f));
        }
        content = content.push(container(section.padding(12)));
    }

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn source_info(dims: (u32, u32), img: &image::DynamicImage) -> Element<'static, Message> {
    let is_rgba8 = img.as_rgba8().is_some();
    let info = column![
        row![
            text("Source").size(11).color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            text(format!("{} × {} px", dims.0, dims.1))
                .size(11)
                .color(colors::TEXT_PRIMARY),
        ],
        row![
            text("Mode").size(11).color(colors::TEXT_MUTED),
            Space::new().width(Length::Fill),
            text(if is_rgba8 {
                "8-bit / canal · RGBA"
            } else {
                "16/32-bit"
            })
            .size(11)
            .color(colors::TEXT_PRIMARY),
        ],
    ]
    .spacing(4);
    container(info)
        .padding(8)
        .style(|_| {
            ui_kit::style::inset_card(
                colors::SURFACE_CONTAINER_LOWEST,
                ui_kit::theme::metrics::RADIUS_SM,
            )
        })
        .into()
}

/// Liste déroulante d'ajout de filtre (effets image→image du registre).
fn add_filter_pick(layer_id: uuid::Uuid) -> Element<'static, Message> {
    let defs = photo_engine::filterable_types();
    #[derive(Clone, PartialEq)]
    struct Choice {
        type_id: String,
        label: String,
    }
    impl std::fmt::Display for Choice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.label)
        }
    }
    let choices: Vec<Choice> = defs
        .into_iter()
        .map(|d| Choice {
            type_id: d.type_id.clone(),
            label: format!("+ {}", d.name),
        })
        .collect();
    iced::widget::pick_list(choices, None::<Choice>, move |c: Choice| {
        Message::AddLiveFilter {
            id: layer_id,
            type_id: c.type_id.clone(),
        }
    })
    .width(Length::Fill)
    .placeholder("Ajouter un filtre…")
    .into()
}

/// Carte d'un filtre : activation, réglages floats, suppression.
fn filter_card<'a>(layer_id: uuid::Uuid, f: &'a FilterNode) -> Element<'a, Message> {
    let fid = f.id;
    let material = ui_kit::icon_button::MATERIAL_ICONS;

    let toggle = iced::widget::button(
        text(if f.enabled { "\u{e8f4}" } else { "\u{e8f5}" })
            .font(material)
            .size(14)
            .color(if f.enabled {
                colors::ACCENT
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding(2)
    .on_press(Message::ToggleFilterEnabled {
        layer_id,
        filter_id: fid,
    });

    let remove = iced::widget::button(
        text("\u{e872}") // delete
            .font(material)
            .size(14)
            .color(colors::TEXT_MUTED),
    )
    .padding(2)
    .on_press(Message::RemoveLiveFilter {
        layer_id,
        filter_id: fid,
    });

    let mut card = column![
        row![
            toggle.style(|_t, s| ui_kit::style::ghost(s)),
            text(&f.type_id).size(11).color(colors::TEXT_SECONDARY),
            Space::new().width(Length::Fill),
            remove.style(|_t, s| ui_kit::style::ghost(s)),
        ]
        .align_y(Alignment::Center)
        .spacing(6)
    ]
    .spacing(4);

    if f.enabled {
        for (key, value) in &f.params {
            if let ParamValue::Float(v) = value {
                let k = key.clone();
                let (lo, hi) = float_param_range(key);
                card = card.push(slider(lo..=hi, *v, move |nv| Message::SetFilterParam {
                    layer_id,
                    filter_id: fid,
                    key: k.clone(),
                    value: ParamValue::Float(nv),
                }));
            }
        }
    }

    container(card)
        .padding(Padding::new(8.0).top(6.0).bottom(6.0))
        .width(Length::Fill)
        .style(|_| {
            ui_kit::style::inset_card(
                colors::SURFACE_CONTAINER_LOWEST,
                ui_kit::theme::metrics::RADIUS_SM,
            )
        })
        .into()
}

/// Bornes de slider par clé de paramètre (heuristique volontairement simple).
fn float_param_range(key: &str) -> (f32, f32) {
    match key {
        "brightness" => (-100.0, 100.0),
        "contrast" => (-100.0, 100.0),
        "saturation" => (0.0, 3.0),
        "radius" => (0.0, 50.0),
        "hue" => (-180.0, 180.0),
        _ => (0.0, 10.0),
    }
}

fn blend_mode_buttons(cur: Option<BlendMode>, id: uuid::Uuid) -> Element<'static, Message> {
    let Some(cur) = cur else {
        return Space::new().height(Length::Fixed(0.0)).into();
    };
    let parse_blend = |label: &str| -> BlendMode {
        BlendMode::ALL
            .iter()
            .find(|m| m.label() == label)
            .copied()
            .unwrap_or(BlendMode::Normal)
    };
    let mode_btn = |label: &'static str| -> Element<'static, Message> {
        let mode = parse_blend(label);
        let is_sel = mode == cur;
        iced::widget::button(text(label).size(11))
            .padding(Padding::new(4.0).left(8.0).right(8.0))
            .style(move |_theme: &iced::Theme, status| {
                let mut st = iced::widget::button::Style::default();
                if is_sel {
                    st.background = Some(colors::ACCENT.into());
                    st.text_color = colors::TEXT_ON_ACCENT;
                } else if status == iced::widget::button::Status::Hovered {
                    st.background = Some(colors::HOVER_OVERLAY.into());
                    st.text_color = colors::TEXT_PRIMARY;
                } else {
                    st.background = Some(colors::SURFACE_CONTAINER_HIGH.into());
                    st.text_color = colors::TEXT_SECONDARY;
                }
                st.border.radius = ui_kit::theme::metrics::RADIUS_BUTTON.into();
                st
            })
            .on_press(Message::SetLayerBlend { id, mode })
            .into()
    };
    column![
        text("Mode de fusion").size(12).color(colors::ON_SURFACE),
        row![mode_btn("Normal"), mode_btn("Multiply"), mode_btn("Screen")].spacing(6),
        row![mode_btn("Overlay"), mode_btn("Darken"), mode_btn("Lighten")].spacing(6),
    ]
    .spacing(6)
    .into()
}

fn offset_row<'a>(
    label: &'a str,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).size(11).color(colors::TEXT_MUTED),
        Space::new().width(Length::Fill),
        text_input("0", &format!("{:.0}", value))
            .size(11)
            .width(Length::Fixed(90.0))
            .on_input(move |s| {
                if let Ok(v) = s.parse::<f32>() {
                    on_change(v)
                } else {
                    Message::MockAction
                }
            }),
    ]
    .align_y(Alignment::Center)
    .spacing(6)
    .into()
}

fn param_slider<'a>(
    label: &'a str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        row![
            text(label).size(12).color(colors::ON_SURFACE),
            Space::new().width(Length::Fill),
            container(
                text(format!("{:.2}", value))
                    .size(11)
                    .color(colors::TEXT_PRIMARY)
            )
            .padding(4)
            .style(|_t| container::Style {
                background: Some(colors::SURFACE_CONTAINER_HIGH.into()),
                border: iced::Border {
                    radius: ui_kit::theme::metrics::RADIUS_BUTTON.into(),
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                },
                ..Default::default()
            })
        ]
        .align_y(Alignment::Center),
        slider(*range.start()..=*range.end(), value, on_change).step(step),
    ]
    .spacing(6)
    .into()
}
