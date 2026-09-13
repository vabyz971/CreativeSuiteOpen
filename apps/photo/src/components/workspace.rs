// Cygnus — Suite créative professionnelle open source
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

use std::sync::Arc;

use crate::components::{layers::panel as layers_panel, properties, toolpanel};
use crate::{Message, PanelType, Tool};
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{Space, container};
use iced::{Element, Length, Size, Vector};
use photo_engine::{Document, FilterNode, LayerNode};
use ui_kit::base_panel;
use ui_kit::layer_canvas::{
    AdjustmentOp, DisplayContent, DisplayLayer, DisplayMask, HitLayer, LayerCanvas, TransformTarget,
};
use ui_kit::theme::colors;
use uuid::Uuid;

/// Identité de contenu d'un tampon partagé (adresse de l'Arc, conservé
/// vivant par le cache — même motif que `PreviewCache`).
fn cle_contenu(data: &Arc<[u8]>) -> u64 {
    Arc::as_ptr(data).cast::<u8>() as usize as u64
}

/// Clé stable d'un nœud (FNV sur l'Uuid) pour le cache de groupes.
fn cle_noeud(id: Uuid) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in id.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Un filtre actif du moteur vers une opération GPU (`type_id` du registre
/// d'effets). Les types inconnus sont traversants, comme côté moteur
/// (`filters::render_nodes` ignore les effets introuvables).
fn operation_ajustement(filtre: &FilterNode) -> Option<AdjustmentOp> {
    if !filtre.enabled {
        return None;
    }
    let param = |nom: &str, defaut: f32| {
        filtre
            .params
            .get(nom)
            .and_then(|p| p.as_float())
            .unwrap_or(defaut)
    };
    match filtre.type_id.as_str() {
        "brightness_contrast" => Some(AdjustmentOp::BrightnessContrast {
            brightness: param("brightness", 0.0),
            contrast: param("contrast", 0.0),
        }),
        "color_correct" => Some(AdjustmentOp::Saturation {
            value: param("saturation", 1.0),
        }),
        "blur" => Some(AdjustmentOp::Blur {
            radius: param("radius", 0.0),
        }),
        _ => None,
    }
}

/// Calques cliquables plats (tous niveaux, haut de pile en dernier) pour
/// le pick de l'outil Select — même ensemble que l'ancien chemin rapide.
fn cibles_pick(noeuds: &[LayerNode]) -> Vec<HitLayer> {
    let mut cibles = Vec::new();
    for noeud in noeuds {
        match noeud {
            LayerNode::Pixel(l) => {
                if l.visible && l.opacity > 0.01 {
                    let (larg, haut) = l.dimensions();
                    cibles.push(HitLayer {
                        id: Some(l.id),
                        transform: l.transform,
                        width: larg as f32,
                        height: haut as f32,
                    });
                }
            }
            LayerNode::Group(g) => {
                cibles.extend(cibles_pick(&g.children));
            }
            LayerNode::Adjustment(_) => {}
        }
    }
    cibles
}

/// Quad du calque sélectionné (coins doc) pour le contour — pixels
/// visibles uniquement (même règle que l'ancien visualiseur).
fn cadre_selection(doc: &Document, selectionne: Option<Uuid>) -> Option<[(f32, f32); 4]> {
    let id = selectionne?;
    let l = doc.pixel_layer(id)?;
    if !l.visible {
        return None;
    }
    let (larg, haut) = l.dimensions();
    Some(l.transform.doc_corners(larg as f32, haut as f32))
}

/// Empile les nœuds en couches affichables (récursif : les groupes portent
/// leurs enfants). Miroir des règles du compositing CPU : invisibles et
/// opacités nulles sautés, groupes vides et ajustements sans opération
/// active ignorés. Les pixels viennent de l'apparence bakée (masques
/// inclus — même sourcing que l'ancien chemin rapide).
fn empiler_noeuds(
    noeuds: &[LayerNode],
    cache: &crate::ui_handles::PreviewCache,
) -> Vec<DisplayLayer> {
    let mut couches = Vec::new();
    for noeud in noeuds {
        match noeud {
            LayerNode::Pixel(l) => {
                if !l.visible || l.opacity <= 0.01 {
                    continue;
                }
                let Some(buf) = cache.apparence(l.id) else {
                    continue;
                };
                // Dimensions LOGIQUES plein format (l'aperçu est réduit
                // au-delà de 2048 px) : le shader étire comme iced, et le
                // placement reste en coordonnées document plein format.
                // `tex_*` = dims RÉELLES du tampon (upload exact).
                let (plein_l, plein_h) = l.dimensions();
                couches.push(DisplayLayer {
                    key: cle_contenu(&buf.data),
                    rgba: Some(Arc::clone(&buf.data)),
                    width: plein_l,
                    height: plein_h,
                    tex_width: buf.width,
                    tex_height: buf.height,
                    opacity: (l.opacity / 100.0).clamp(0.0, 1.0),
                    blend: l.blend_mode.id(),
                    transform: l.transform,
                    mask: None,
                    content: DisplayContent::Pixel,
                });
            }
            LayerNode::Group(g) => {
                if !g.visible || g.opacity <= 0.01 {
                    continue;
                }
                let enfants = empiler_noeuds(&g.children, cache);
                if enfants.is_empty() {
                    continue;
                }
                // Masques du groupe : couverture taille document (espace
                // document côté shader), en cache par signature.
                let masque = cache.couverture_groupe(g.id).map(|couv| DisplayMask {
                    key: cle_contenu(&couv.data),
                    rgba: couv.data,
                    width: couv.width,
                    height: couv.height,
                });
                couches.push(DisplayLayer {
                    key: cle_noeud(g.id),
                    rgba: None,
                    width: 0,
                    height: 0,
                    tex_width: 0,
                    tex_height: 0,
                    opacity: (g.opacity / 100.0).clamp(0.0, 1.0),
                    blend: g.blend_mode.id(),
                    transform: photo_engine::Transform2D::default(),
                    mask: masque,
                    content: DisplayContent::Group(enfants),
                });
            }
            LayerNode::Adjustment(a) => {
                if !a.visible || a.opacity <= 0.01 {
                    continue;
                }
                let ops: Vec<AdjustmentOp> =
                    a.filters.iter().filter_map(operation_ajustement).collect();
                if ops.is_empty() {
                    continue;
                }
                couches.push(DisplayLayer {
                    key: cle_noeud(a.id),
                    rgba: None,
                    width: 0,
                    height: 0,
                    tex_width: 0,
                    tex_height: 0,
                    opacity: (a.opacity / 100.0).clamp(0.0, 1.0),
                    blend: 0,
                    transform: photo_engine::Transform2D::default(),
                    mask: None,
                    content: DisplayContent::Adjustment(ops),
                });
            }
        }
    }
    couches
}

#[allow(clippy::too_many_arguments)]
pub fn render<'a>(
    panes: &'a pane_grid::State<PanelType>,
    focus: Option<pane_grid::Pane>,
    doc: &'a Document,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    selected_layer: Option<Uuid>,
    layer_drag: &'a crate::components::layers::LayerDragState,
    hovered_layer_row: Option<Uuid>,
    active_mask: Option<crate::message::MaskTarget>,
    expanded_fx_stack: &'a std::collections::HashSet<Uuid>,
    filter_menu_open: bool,
    context_menu_open: Option<Uuid>,
    layer_item_radius: f32,
    mask_brush_black: bool,
    doc_size: Option<Size>,
    image_path: Option<String>,
    selected_tool: Tool,
    brush_color: iced::Color,
    color_picker_open: bool,
    tools_visible: bool,
    canvas_pan: Vector,
    zoom_level: u32,
    canvas_selection: Option<iced::Rectangle>,
    color_profile: String,
    _canvas_viewport: Size,
    // Style du pinceau + aperçu figé du commit en cours (texture)
    brush: ui_kit::image_canvas::BrushStyle,
    pending_preview: Option<ui_kit::image_canvas::StrokeTex>,
    // Loupe pipette : patch RGBA + côté (pixels fournis par l'app)
    loupe: Option<(Arc<[u8]>, u32)>,
    // Un déplacement de calque est en cours (pill d'outils adaptée).
    deplacement: bool,
    // Écran d'accueil (aucun document ouvert)
    new_doc_w: &'a str,
    new_doc_h: &'a str,
    welcome_error: Option<&'a str>,
    // Réglage slider en vol (pouce du slider, voir filter_card)
    pending_param: Option<&'a crate::message::PendingParam>,
) -> Element<'a, Message> {
    let total_panes = panes.len();

    let pane_grid = PaneGrid::new(panes, |id, panel_type, _is_maximized| {
        let is_focused = focus == Some(id);

        let (title_text, base_content): (String, Element<'_, Message>) = match panel_type {
            PanelType::Canvas => {
                // Chemin de rendu UNIQUE : chaque entrée = une texture GPU
                // (pixels bakés), offsets/transforms/fusions/masques appliqués
                // au draw → drag/zoom/peinture sans AUCUN recomposite CPU.
                // (L'export exact reste CPU côté moteur.)
                let preview: Element<'_, Message> = render_canvas_preview(
                    doc,
                    preview_cache,
                    doc_size,
                    selected_tool,
                    selected_layer,
                    canvas_pan,
                    zoom_level,
                    canvas_selection,
                    brush,
                    pending_preview.clone(),
                    loupe.clone(),
                    deplacement,
                    tools_visible,
                    brush_color,
                    color_picker_open,
                    mask_brush_black,
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
            PanelType::Properties => (
                "Propriétés".to_string(),
                properties::render(doc, selected_layer, active_mask, pending_param),
            ),
            PanelType::Layers => (
                "Calques".to_string(),
                layers_panel::render(
                    doc,
                    preview_cache,
                    selected_layer,
                    layer_drag,
                    hovered_layer_row,
                    active_mask,
                    expanded_fx_stack,
                    filter_menu_open,
                    context_menu_open,
                    layer_item_radius,
                ),
            ),
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

#[allow(clippy::too_many_arguments)]
fn render_canvas_preview<'a>(
    doc: &'a Document,
    preview_cache: &'a crate::ui_handles::PreviewCache,
    doc_size: Option<Size>,
    selected_tool: Tool,
    selected_layer: Option<Uuid>,
    canvas_pan: Vector,
    zoom_level: u32,
    canvas_selection: Option<iced::Rectangle>,
    brush: ui_kit::image_canvas::BrushStyle,
    pending_preview: Option<ui_kit::image_canvas::StrokeTex>,
    loupe: Option<(Arc<[u8]>, u32)>,
    // Un déplacement de calque est en cours (pill d'outils adaptée).
    deplacement: bool,
    tools_visible: bool,
    brush_color: iced::Color,
    color_picker_open: bool,
    mask_brush_black: bool,
    new_doc_w: &'a str,
    new_doc_h: &'a str,
    welcome_error: Option<&'a str>,
) -> Element<'a, Message> {
    let zoom = zoom_level as f32 / 100.0;
    let canvas_tool = match selected_tool {
        Tool::Hand => ui_kit::image_canvas::CanvasTool::Hand,
        Tool::Zoom => ui_kit::image_canvas::CanvasTool::Zoom,
        Tool::Select => ui_kit::image_canvas::CanvasTool::Select,
        Tool::Eyedropper => ui_kit::image_canvas::CanvasTool::Eyedropper,
        Tool::Brush => ui_kit::image_canvas::CanvasTool::Brush,
        Tool::Eraser => ui_kit::image_canvas::CanvasTool::Eraser,
    };
    // Pile d'affichage : pixels bakés + groupes + ajustements, mapping
    // pur du document (aucune logique de rendu ici — le compositing vit
    // dans le shader `layer_canvas`).
    let couches = empiler_noeuds(&doc.root, preview_cache);
    // Peinture autorisée si la cible (sous-calque → porteur) est visible.
    let canvas_target = selected_layer.and_then(|id| doc.find_filter_parent(id).or(Some(id)));
    let can_paint = canvas_target
        .and_then(|id| doc.find(id))
        .map(|n| n.visible())
        .unwrap_or(false);
    // Cible du visualiseur de transformation : calque pixels visible.
    let transform_target = canvas_target
        .and_then(|id| doc.pixel_layer(id))
        .filter(|l| l.visible)
        .map(|l| {
            let (larg, haut) = l.dimensions();
            TransformTarget {
                id: Some(l.id),
                transform: l.transform,
                width: larg as f32,
                height: haut as f32,
            }
        });
    let on_event = std::rc::Rc::new(|evt: ui_kit::image_canvas::ImageCanvasEvent| {
        Message::ImageCanvasEvent(evt)
    });
    let canvas = ui_kit::layer_canvas::view(
        LayerCanvas::new(doc_size.map(|d| (d.width, d.height)), on_event)
            .with_layers(couches)
            .with_hit_layers(cibles_pick(&doc.root))
            .with_cadre(cadre_selection(doc, selected_layer))
            .with_transform_target(transform_target)
            .with_view(canvas_pan, zoom)
            .with_tool(canvas_tool)
            .with_selection(canvas_selection)
            .with_brush(brush)
            .with_can_paint(can_paint)
            .with_pending_preview(pending_preview)
            .with_loupe_patch(loupe),
    );
    let content: Element<'_, Message> = if doc.root.is_empty() && doc_size.is_none() {
        // Écran d'accueil : créer/ouvrir un document
        let welcome = crate::components::welcome::render(new_doc_w, new_doc_h, welcome_error);
        iced::widget::stack![
            container(canvas).width(Length::Fill).height(Length::Fill),
            iced::widget::center(welcome),
        ]
        .into()
    } else {
        container(canvas)
            .width(Length::Fill)
            .height(Length::Fill)
            .clip(true)
            .into()
    };
    // Barre d'outils FLOTTANTE verticale en HAUT à gauche du canvas
    let floating_tools: Element<'_, Message> = if tools_visible {
        let tools_pill = container(toolpanel::render(
            selected_tool,
            brush_color,
            color_picker_open,
            mask_brush_black,
            deplacement,
        ))
        .padding(iced::Padding::new(3.0).top(3.0).bottom(3.0))
        .style(|_| {
            // Palette flottante façon macOS : fond discret, ombre portée
            ui_kit::style::floating_card(
                colors::SURFACE_CONTAINER_LOW,
                ui_kit::theme::metrics::RADIUS_NODE,
                ui_kit::theme::shadows::panel(),
            )
        });
        container(tools_pill.width(Length::Shrink))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Top)
            .align_x(iced::alignment::Horizontal::Left)
            .padding(iced::Padding::default().top(14.0).left(14.0))
            .into()
    } else {
        Space::new()
            .width(Length::Fixed(0.0))
            .height(Length::Fixed(0.0))
            .into()
    };

    container(iced::widget::stack![
        container(content).width(Length::Fill).height(Length::Fill),
        floating_tools,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn image_pleine(w: u32, h: u32) -> Arc<image::DynamicImage> {
        Arc::new(image::DynamicImage::ImageRgba8(
            image::ImageBuffer::from_pixel(w, h, image::Rgba([10, 20, 30, 255])),
        ))
    }

    fn calque_pixels(nom: &str) -> photo_engine::PixelLayer {
        photo_engine::PixelLayer::new(nom, image_pleine(4, 4))
    }

    fn doc_synchronise(doc: &photo_engine::Document) -> crate::ui_handles::PreviewCache {
        let mut cache = crate::ui_handles::PreviewCache::default();
        cache.sync(doc);
        cache
    }

    #[test]
    fn operation_ajustement_mapping() {
        // brightness_contrast avec paramètres explicites.
        let mut bc = FilterNode::new("brightness_contrast");
        bc.params
            .insert("brightness".to_string(), datatypes::ParamValue::Float(10.0));
        bc.params
            .insert("contrast".to_string(), datatypes::ParamValue::Float(-20.0));
        let Some(AdjustmentOp::BrightnessContrast {
            brightness,
            contrast,
        }) = operation_ajustement(&bc)
        else {
            panic!("BC attendu");
        };
        assert!((brightness - 10.0).abs() < 1e-6);
        assert!((contrast + 20.0).abs() < 1e-6);
        // Défauts moteur quand le paramètre est absent.
        let sat = operation_ajustement(&FilterNode::new("color_correct"));
        assert!(matches!(
            sat,
            Some(AdjustmentOp::Saturation { value }) if (value - 1.0).abs() < 1e-6
        ));
        let flou = operation_ajustement(&FilterNode::new("blur"));
        assert!(matches!(
            flou,
            Some(AdjustmentOp::Blur { radius }) if radius.abs() < 1e-6
        ));
        // Inconnu → traversant (comme le moteur) ; désactivé → ignoré.
        assert!(operation_ajustement(&FilterNode::new("effet_futur")).is_none());
        let mut eteint = FilterNode::new("brightness_contrast");
        eteint.enabled = false;
        assert!(operation_ajustement(&eteint).is_none());
    }

    #[test]
    fn empilement_groupe_multiply() {
        // Scénario 1 (ancien repli CPU) : calque glissé dans un groupe en
        // Multiply → UNE entrée groupe (fusion 1) avec l'enfant dedans.
        let mut doc = Document::new(8, 8);
        let mut groupe =
            photo_engine::GroupLayer::new("G", vec![LayerNode::Pixel(calque_pixels("P"))]);
        groupe.blend_mode = photo_engine::BlendMode::Multiply;
        doc.push_layer(LayerNode::Group(groupe));
        let mut cache = doc_synchronise(&doc);
        let couches = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches.len(), 1);
        let DisplayContent::Group(enfants) = &couches[0].content else {
            panic!("groupe attendu");
        };
        assert_eq!(enfants.len(), 1);
        assert_eq!(couches[0].blend, 1, "Multiply = 1 (BlendMode::id)");
        assert!(matches!(enfants[0].content, DisplayContent::Pixel));
        // Stabilité : second passage, même clé (pas de re-téléversement).
        cache.sync(&doc);
        let couches2 = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches[0].key, couches2[0].key);
    }

    #[test]
    fn empilement_masque_bake() {
        // Scénario 2 : peinture sur calque à masque actif → pixels bakés
        // (masque inclus), AUCUNE couverture séparée (pas de double
        // application), clé stable.
        let mut doc = Document::new(8, 8);
        let mut l = calque_pixels("M");
        l.masks.push(photo_engine::LayerMask {
            id: Uuid::new_v4(),
            name: String::from("Masque"),
            image: Arc::new(image::ImageBuffer::from_pixel(
                4,
                4,
                image::Rgba([255, 255, 255, 255]),
            )),
            enabled: true,
            inverted: false,
            version: 0,
        });
        doc.push_layer(LayerNode::Pixel(l));
        let mut cache = doc_synchronise(&doc);
        let couches = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches.len(), 1);
        assert!(couches[0].rgba.is_some());
        assert!(couches[0].mask.is_none(), "masque déjà baké");
        cache.sync(&doc);
        let couches2 = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches[0].key, couches2[0].key);
    }

    #[test]
    fn empilement_grande_image_tex_reduit() {
        // Photo > 2048 px : l'aperçu est réduit, les dims logiques restent
        // plein format et `tex_*` suit le tampon (upload exact, pas de
        // lecture hors limites — régression crash du chemin unique).
        let mut doc = Document::new(2100, 100);
        let gros = photo_engine::PixelLayer::new("G", image_pleine(2100, 100));
        doc.push_layer(LayerNode::Pixel(gros));
        let cache = doc_synchronise(&doc);
        let couches = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches.len(), 1);
        let couche = &couches[0];
        assert_eq!((couche.width, couche.height), (2100, 100));
        assert!(
            couche.tex_width <= 2100 && couche.tex_height <= 100,
            "tampon réduit ou égal, jamais agrandi",
        );
        let rgba = couche.rgba.as_ref().expect("pixels");
        assert_eq!(
            rgba.len(),
            couche.tex_width as usize * couche.tex_height as usize * 4
        );
    }

    #[test]
    fn cadre_selection_quad() {
        // Calque sélectionné visible → ses 4 coins doc (identité ici) ;
        // rien si aucune sélection, calque invisible ou filtre.
        let mut doc = Document::new(8, 8);
        let l = calque_pixels("C");
        let id = l.id;
        doc.push_layer(LayerNode::Pixel(l));
        let quad = cadre_selection(&doc, Some(id)).expect("cadre");
        assert_eq!(quad[0], (0.0, 0.0));
        assert_eq!(quad[2], (4.0, 4.0));
        assert!(cadre_selection(&doc, None).is_none());
        doc.pixel_layer_mut(id).unwrap().visible = false;
        assert!(cadre_selection(&doc, Some(id)).is_none());
    }

    #[test]
    fn empilement_ajustement_et_skew() {
        // Scénario 3 : skew conservé dans le placement ; ajustement actif
        // → entrée dédiée avec sa chaîne.
        let mut doc = Document::new(8, 8);
        let mut l = calque_pixels("S");
        l.transform.skew_x = 15.0;
        doc.push_layer(LayerNode::Pixel(l));
        let bc = FilterNode::new("brightness_contrast");
        doc.push_layer(LayerNode::Adjustment(photo_engine::AdjustmentLayer::new(
            "A",
            vec![bc],
        )));
        let cache = doc_synchronise(&doc);
        let couches = empiler_noeuds(&doc.root, &cache);
        assert_eq!(couches.len(), 2);
        assert!((couches[0].transform.skew_x - 15.0).abs() < 1e-6);
        let DisplayContent::Adjustment(ops) = &couches[1].content else {
            panic!("ajustement attendu");
        };
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], AdjustmentOp::BrightnessContrast { .. }));
        // Invisibles et ajustements vides : ignorés (comme le CPU).
        let mut doc2 = Document::new(8, 8);
        let mut invisible = calque_pixels("I");
        invisible.visible = false;
        doc2.push_layer(LayerNode::Pixel(invisible));
        doc2.push_layer(LayerNode::Adjustment(photo_engine::AdjustmentLayer::new(
            "V",
            vec![],
        )));
        let cache2 = doc_synchronise(&doc2);
        assert!(empiler_noeuds(&doc2.root, &cache2).is_empty());
    }
}
