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

use crate::components::{layers_panel, properties, toolpanel};
use crate::layers::Layer;
use crate::{Message, PanelType, Tool};
use suite_core::Graph;
use datatypes::NodeId;
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{button, column, container, image, text, Space};
use iced::{Alignment, Element, Length, Size, Vector};
use ui::base_panel;
use ui::dropdown::{dropdown_box, menu_item, menu_separator};
use ui::theme::colors;

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    panes: &'a pane_grid::State<PanelType>,
    focus: Option<pane_grid::Pane>,
    layers: &'a [Layer],
    selected_layer: Option<u64>,
    doc_size: Option<Size>,
    fallback_handle: Option<image::Handle>,
    fallback_size: Option<Size>,
    // Calque en cours de déplacement (mode fallback)
    drag_layer: Option<u64>,
    // Fond composite pré-calculé sans le calque déplacé
    drag_background: Option<image::Handle>,
    drag_background_size: Option<Size>,
    image_path: Option<String>,
    image_error: Option<String>,
    selected_tool: Tool,
    tools_visible: bool,
    canvas_pan: Vector,
    zoom_level: u32,
    canvas_selection: Option<iced::Rectangle>,
    color_profile: String,
    canvas_viewport: Size,
    // Générateur de textures (graphe nodal, futur usage filtres/génération)
    gen_graph: &'a Graph,
    gen_selected: Option<NodeId>,
    gen_previews: &std::collections::HashMap<NodeId, image::Handle>,
    node_context_menu: Option<iced::Point>,
    node_context_world: Option<datatypes::Vec2>,
    // Trait de pinceau en cours (aperçu canvas, style inclus)
    stroke_overlay: Option<ui::image_canvas::StrokeOverlay>,
    // Écran d'accueil (aucun document ouvert)
    new_doc_w: &'a str,
    new_doc_h: &'a str,
    welcome_error: Option<&'a str>,
) -> Element<'a, Message> {
    let total_panes = panes.len();

    let stroke_src = &stroke_overlay;
    let pane_grid = PaneGrid::new(panes, |id, panel_type, _is_maximized| {
        let is_focused = focus == Some(id);

        let (title_text, base_content): (String, Element<'_, Message>) = match panel_type {
            PanelType::Canvas => {
                // Chemin rapide : chaque calque = une texture canvas, offsets
                // appliqués au draw → drag/zoom sans AUCUN recomposite.
                // Fallback : modes de fusion non-Normal → composite CPU unique.
                let needs_fallback = layers
                    .iter()
                    .any(|l| l.visible && l.opacity > 0.01 && l.blend_mode != "Normal");
                let preview: Element<'_, Message> = render_canvas_preview(
                    if needs_fallback { fallback_handle.clone() } else { None },
                    if needs_fallback { fallback_size } else { None },
                    drag_layer,
                    drag_background.clone(),
                    drag_background_size,
                    layers,
                    doc_size,
                    image_error.clone(),
                    selected_tool,
                    selected_layer,
                    tools_visible,
                    canvas_pan,
                    zoom_level,
                    canvas_selection,
                    canvas_viewport,
                    stroke_src.clone(),
                    new_doc_w,
                    new_doc_h,
                    welcome_error,
                );
                // Titre dynamique : nom du fichier + dimensions + profil
                let title = match (image_path.as_deref(), doc_size) {
                    (Some(name), Some(sz)) => format!(
                        "{} — {} × {} px • {}",
                        name, sz.width as u32, sz.height as u32, color_profile
                    ),
                    (Some(name), None) => name.to_string(),
                    _ => "Canvas".to_string(),
                };
                (title, preview)
            }
            PanelType::Properties => {
                let sel = selected_layer.and_then(|id| layers.iter().find(|l| l.id == id));
                ("Propriétés".to_string(), properties::render(sel))
            }
            PanelType::Layers => {
                ("Calques".to_string(), layers_panel::render(layers, selected_layer))
            }
            PanelType::Generator => {
                let g_clone = gen_graph.clone();
                let previews = gen_previews.clone();
                let busy_empty: std::collections::HashSet<NodeId> = Default::default();
                let canvas = ui::node_graph::view(g_clone, gen_selected, Vector::new(0.0, 0.0), 1.0, previews, &busy_empty)
                    .map(Message::NodeGraphEvent);

                // Menu contextuel ancré dans les coordonnées LOCALES du canvas
                let content: Element<'_, Message> = if let Some(local) = node_context_menu {
                    let world = node_context_world.unwrap_or(datatypes::Vec2::new(0.0, 0.0));
                    let node_menu = build_node_context_menu(local, world);
                    let outside = iced::widget::mouse_area(
                        iced::widget::Space::new().width(Length::Fill).height(Length::Fill),
                    )
                    .on_press(Message::CloseNodeContextMenu);
                    let menu_pos = container(node_menu)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(iced::Padding::default().top(local.y).left(local.x))
                        .align_x(iced::alignment::Horizontal::Left)
                        .align_y(iced::alignment::Vertical::Top);
                    iced::widget::stack![container(canvas).width(Length::Fill).height(Length::Fill), outside, menu_pos].into()
                } else {
                    container(canvas).padding(0).clip(true).into()
                };
                ("Générateur de textures".to_string(), content)
            }
        };

        // ContextMenu native sur le titre (clic droit → fermer le panneau)
        let close_menu = if *panel_type != PanelType::Canvas && total_panes > 1 {
            Some(Message::ClosePane(id))
        } else {
            None
        };

        base_panel::render(title_text, base_content, is_focused, close_menu)
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(4)
    .on_click(Message::PaneClicked)
    .on_drag(Message::PaneDragged)
    .on_resize(10, Message::PaneResized);

    let grid_container = container(pane_grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(4)
        .style(|_theme| container::Style {
            background: Some(colors::BG_APP.into()),
            ..Default::default()
        });

    grid_container.into()
}

fn build_node_context_menu<'a>(click_pos: iced::Point, world: datatypes::Vec2) -> Element<'a, Message> {
    let categories: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("Couleur", vec![("Luminosité / Contraste", "brightness_contrast"), ("Correction Couleur", "color_correct")]),
        ("Filtre", vec![("Flou", "blur")]),
        ("Compositing", vec![("Mélange", "mix")]),
        ("Sortie", vec![("Sortie", "output")]),
    ];

    let mut col = column![container(text("Ajouter un nœud générateur").size(13).color(colors::TEXT_PRIMARY)).padding(iced::Padding::new(4.0).left(8.0))].spacing(2);
    for (cat_label, nodes) in categories {
        col = col.push(container(text(cat_label).size(11).color(colors::TEXT_MUTED)).padding(iced::Padding::new(2.0).left(8.0)));
        for (label, type_id) in nodes {
            col = col.push(menu_item(
                label,
                "",
                Message::AddNodeAt { type_id: type_id.to_string(), world_pos: world },
            ));
        }
        col = col.push(menu_separator());
    }
    let _ = click_pos;
    dropdown_box(col, 220.0)
}

#[allow(clippy::too_many_arguments)]
fn render_canvas_preview<'a>(
    fallback_handle: Option<image::Handle>,
    fallback_size: Option<Size>,
    drag_layer: Option<u64>,
    drag_background: Option<image::Handle>,
    drag_background_size: Option<Size>,
    layers: &'a [Layer],
    doc_size: Option<Size>,
    image_error: Option<String>,
    selected_tool: Tool,
    selected_layer: Option<u64>,
    tools_visible: bool,
    canvas_pan: Vector,
    zoom_level: u32,
    canvas_selection: Option<iced::Rectangle>,
    _viewport: Size,
    stroke_overlay: Option<ui::image_canvas::StrokeOverlay>,
    new_doc_w: &'a str,
    new_doc_h: &'a str,
    welcome_error: Option<&'a str>,
) -> Element<'a, Message> {
    let _ = selected_layer; // conservé pour futurs réglages contextuels
    let zoom = zoom_level as f32 / 100.0;
    let canvas_tool = match selected_tool {
        Tool::Hand => ui::image_canvas::CanvasTool::Hand,
        Tool::Move => ui::image_canvas::CanvasTool::Move,
        Tool::Zoom => ui::image_canvas::CanvasTool::Zoom,
        Tool::Select => ui::image_canvas::CanvasTool::Select,
        Tool::Eyedropper => ui::image_canvas::CanvasTool::Select,
        Tool::Brush => ui::image_canvas::CanvasTool::Brush,
    };
    // Calques canvas : texture + offset + opacité appliqués AU DRAW (GPU)
    // → slider d'opacité = zéro régénération de pixels, zéro clignotement
    let dragging = drag_layer.is_some();
    let mut canvas_layers: Vec<ui::image_canvas::CanvasLayer> = layers
        .iter()
        .filter(|l| l.visible && l.opacity > 0.01)
        // En drag fallback : le calque déplacé est exclu du fond (il est
        // dessiné par-dessus le fond pré-calculé, voir plus bas)
        .filter(|l| !(dragging && drag_background.is_some() && Some(l.id) == drag_layer))
        .map(|l| {
            let (w, h) = l.dimensions();
            ui::image_canvas::CanvasLayer {
                handle: l.handle.clone(),
                width: w as f32,
                height: h as f32,
                offset_x: l.offset_x,
                offset_y: l.offset_y,
                opacity: (l.opacity / 100.0).clamp(0.0, 1.0),
                rotation_deg: l.rotation,
                scale: l.scale,
            }
        })
        .collect();

    // Drag en fallback : fond pré-calculé (sans le calque déplacé) inséré
    // en bas de pile, puis le calque déplacé dessiné par-dessus à sa
    // position live. ZÉRO recomposite pendant le geste — le blend réel
    // (Multiply/Screen/…) est recalculé une seule fois au relâchement.
    let has_drag_bg = drag_background.is_some() && drag_background_size.is_some();
    if dragging
        && let Some(bg) = drag_background
        && let Some(bgsz) = drag_background_size
    {
        let (bg_off_x, bg_off_y) = doc_size
            .map(|d| ((d.width - bgsz.width) / 2.0, (d.height - bgsz.height) / 2.0))
            .unwrap_or((0.0, 0.0));
        canvas_layers.insert(
            0,
            ui::image_canvas::CanvasLayer {
                handle: bg,
                width: bgsz.width,
                height: bgsz.height,
                offset_x: bg_off_x,
                offset_y: bg_off_y,
                opacity: 1.0,
                rotation_deg: 0.0,
                scale: 1.0,
            },
        );
    }
    if dragging && has_drag_bg
        && let Some(l) = drag_layer.and_then(|id| layers.iter().find(|l| l.id == id))
        && l.visible
    {
        // Uniquement en fallback : le fond pré-calculé exclut ce calque,
        // on le dessine par-dessus. En chemin rapide il est DÉJÀ dans
        // canvas_layers — le push ici le dessinerait deux fois.
        let (w, h) = l.dimensions();
        canvas_layers.push(ui::image_canvas::CanvasLayer {
            handle: l.handle.clone(),
            width: w as f32,
            height: h as f32,
            offset_x: l.offset_x,
            offset_y: l.offset_y,
            opacity: (l.opacity / 100.0).clamp(0.0, 1.0),
            rotation_deg: l.rotation,
            scale: l.scale,
        });
    }

    // Fallback fusion non-Normal (HORS drag) : une seule image composite.
    // Pendant un drag, on utilise le chemin par calque ci-dessus
    // (fond pré-calculé + calque déplacé).
    // Le buffer composite est symétrique autour du centre document →
    // offset = (doc - buffer)/2 pour respecter la convention
    // « offset (0,0) = coin haut-gauche du document ».
    let content: Element<'_, Message> = if !dragging
        && let (Some(handle), Some(sz)) = (fallback_handle, fallback_size)
    {
        let (fb_off_x, fb_off_y) = doc_size
            .map(|d| ((d.width - sz.width) / 2.0, (d.height - sz.height) / 2.0))
            .unwrap_or((0.0, 0.0));
        let ls = vec![ui::image_canvas::CanvasLayer {
            handle,
            width: sz.width,
            height: sz.height,
            offset_x: fb_off_x,
            offset_y: fb_off_y,
            opacity: 1.0, // opacité déjà appliquée dans le composite
            rotation_deg: 0.0,
            scale: 1.0,
        }];
        let canvas = ui::image_canvas::view_with_tool(
            doc_size, canvas_pan, zoom, canvas_tool, canvas_selection, ls, None,
        )
        .map(Message::ImageCanvasEvent);
        container(canvas).width(Length::Fill).height(Length::Fill).clip(true).into()
    } else {
        let canvas = ui::image_canvas::view_with_tool(
            doc_size, canvas_pan, zoom, canvas_tool, canvas_selection, canvas_layers,
            stroke_overlay,
        )
        .map(Message::ImageCanvasEvent);
        if layers.is_empty() && doc_size.is_none() {
            // Écran d'accueil : créer/ouvrir un document
            let welcome = crate::components::welcome::render(
                new_doc_w,
                new_doc_h,
                welcome_error,
            );
            iced::widget::stack![
                container(canvas).width(Length::Fill).height(Length::Fill),
                container(welcome)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            ]
            .into()
        } else {
            container(canvas).width(Length::Fill).height(Length::Fill).clip(true).into()
        }
    };

    // Barre d'outils FLOTTANTE verticale en HAUT à gauche du canvas
    let floating_tools: Element<'_, Message> = if tools_visible {
        let tools_pill = container(toolpanel::render(selected_tool))
            .padding(iced::Padding::new(4.0).top(4.0).bottom(4.0))
            .style(|_| container::Style {
                background: Some(colors::SURFACE_CONTAINER.into()),
                border: iced::Border {
                    width: 1.0,
                    color: colors::BORDER_PANEL,
                    radius: 10.0.into(),
                },
                shadow: ui::theme::shadows::panel(),
                ..Default::default()
            });
        container(tools_pill.width(Length::Shrink))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .align_x(iced::alignment::Horizontal::Left)
            .padding(iced::Padding::default().top(14.0).left(14.0))
            .into()
    } else {
        Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(0.0)).into()
    };

    container(
        iced::widget::stack![
            container(content).width(Length::Fill).height(Length::Fill),
            floating_tools,
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
}
