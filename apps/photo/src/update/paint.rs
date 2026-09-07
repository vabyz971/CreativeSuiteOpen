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

//! Paint / brush / mask message handlers — extracted from update/mod.rs.

use crate::message::Message;
use crate::message::Tool;
use crate::state::PhotoApp;
use iced::Task;
use std::sync::Arc;
use uuid::Uuid;

pub fn handle_brush_start(app: &mut PhotoApp, x: f32, y: f32, erase: bool) -> Task<Message> {
    let _ = (x, y, erase);
    if app.tools.pending_paint.is_none()
        && let Some(id) = app.document.selected_layer
        && app.document.doc.pixel_layer(id).is_some()
        && app
            .document
            .doc
            .find(id)
            .map(|n| n.visible())
            .unwrap_or(false)
    {
        app.tools.stroke_layer = Some(id);
    }
    Task::none()
}

pub fn handle_brush_end(
    app: &mut PhotoApp,
    points: Vec<(f32, f32)>,
    tex: Option<ui_kit::image_canvas::StrokeTex>,
    erase: bool,
) -> Task<Message> {
    let stroke_target = app.tools.stroke_layer.take();
    if let Some(id) = stroke_target
        && app.tools.pending_paint.is_none()
        && points.len() > 1
        && let Some(tex) = tex
    {
        // Masque actif : ciblage direct OU via le calque porteur (un masque
        // de sous-calque se peint depuis le parent, façon Affinity).
        let active = app.tools.active_mask.and_then(|t| {
            let owner_ok =
                t.layer_id == id || app.document.doc.find_filter_parent(t.layer_id) == Some(id);
            owner_ok.then_some(t)
        });
        let stroke_mask_id = active
            .filter(|t| app.document.doc.mask_of(t.layer_id, t.mask_id).is_some())
            .map(|t| t.mask_id);
        // Porteur effectif du masque (filtre ou calque) — utilisé pour la
        // lecture, le write-back et l'espace de transform.
        let mask_owner = active.map(|t| t.layer_id);
        let is_mask = stroke_mask_id.is_some();
        // Ne capturer QUE des Arc (zéro copie) : sur un masque, la copie du
        // buffer se fait dans le worker (`commit_stroke`), pas sur le thread UI.
        let source = if is_mask {
            let owner = match mask_owner {
                Some(id) => id,
                None => return Task::none(),
            };
            let mask_id = match stroke_mask_id {
                Some(id) => id,
                None => return Task::none(),
            };
            let m = match app.document.doc.mask_of(owner, mask_id) {
                Some(mask) => mask,
                None => return Task::none(),
            };
            let mask = Arc::clone(&m.image);
            // Espace du porteur : sous-calque → transform du calque parent.
            let carrier = app.document.doc.find_filter_parent(owner).unwrap_or(owner);
            let transform = match app.document.doc.find(carrier) {
                Some(photo_engine::LayerNode::Pixel(l)) => l.transform,
                _ => crate::layers::Transform2D::default(),
            };
            PaintSource::Mask(mask, transform)
        } else if let Some(layer) = app.document.doc.pixel_layer(id) {
            PaintSource::Dyn(Arc::clone(&layer.source_image), layer.transform)
        } else {
            return Task::none();
        };
        let pts = points;
        // Le write-back suit le PORTEUR du masque (peut être un filtre).
        let commit_owner = mask_owner.unwrap_or(id);
        app.tools.pending_paint = Some(crate::message::PendingPaint {
            layer_id: commit_owner,
            mask_id: stroke_mask_id,
            tex: tex.clone(),
        });
        app.document.history.push_snapshot(app.snapshot());
        // Sur un masque (façon Affinity) : le pinceau peint la couleur du
        // toggle — noir = masque (cache), blanc = révèle ; la gomme révèle
        // toujours (blanc). Mode Paint pour écrire la couverture (canal R).
        let (stroke_color, stroke_mode) = if is_mask {
            let reveal = erase || !app.tools.mask_brush_black;
            (
                if reveal { [255, 255, 255] } else { [0, 0, 0] },
                photo_engine::paint::StrokeMode::Paint,
            )
        } else {
            let c = app.tools.brush_color;
            (
                [
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                ],
                if erase {
                    photo_engine::paint::StrokeMode::Erase
                } else {
                    photo_engine::paint::StrokeMode::Paint
                },
            )
        };
        let brush = photo_engine::paint::BrushParams {
            radius: app.tools.brush_size / 2.0,
            color: stroke_color,
            opacity: app.tools.brush_opacity,
            mode: stroke_mode,
        };
        let task_id = app.rendering.background_tasks.start("Trait de pinceau...");
        return Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let (dyn_img, transform) = match source {
                        PaintSource::Dyn(img, t) => (img, t),
                        PaintSource::Mask(mask, t) => (
                            Arc::new(image::DynamicImage::ImageRgba8((*mask).clone())),
                            t,
                        ),
                    };
                    photo_engine::paint::commit_stroke(&dyn_img, &pts, &transform, &brush)
                })
                .await
            },
            move |result| match result {
                Ok(buf) => Message::PaintApplied {
                    task_id,
                    layer_id: commit_owner,
                    mask_id: stroke_mask_id,
                    buf,
                },
                Err(_) => Message::PaintFailed {
                    task_id,
                    layer_id: commit_owner,
                    mask_id: stroke_mask_id,
                },
            },
        );
    }
    Task::none()
}

/// Buffer source du trait : Arc partagé, dé-référencé dans le worker.
enum PaintSource {
    Dyn(Arc<image::DynamicImage>, crate::layers::Transform2D),
    Mask(Arc<image::RgbaImage>, crate::layers::Transform2D),
}

pub fn handle_paint_failed(
    app: &mut PhotoApp,
    task_id: u64,
    layer_id: Uuid,
    mask_id: Option<Uuid>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    if app
        .tools
        .pending_paint
        .as_ref()
        .is_some_and(|p| p.layer_id == layer_id && p.mask_id == mask_id)
    {
        app.tools.pending_paint = None;
    }
    app.canvas.image_error = Some("Échec interne lors de l'application du trait".into());
    Task::none()
}

pub fn handle_paint_applied(
    app: &mut PhotoApp,
    task_id: u64,
    layer_id: Uuid,
    mask_id: Option<Uuid>,
    buf: photo_engine::paint::StrokeCommit,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    if let Some(img) = image::RgbaImage::from_raw(buf.width, buf.height, buf.rgba) {
        if let Some(mask_id) = mask_id {
            if let Some(mask) = app.document.doc.mask_of_mut(layer_id, mask_id) {
                mask.image = Arc::new(img);
                mask.touch();
            }
        } else {
            app.document
                .doc
                .set_source_image(layer_id, image::DynamicImage::ImageRgba8(img));
        }
    }
    app.tools.pending_paint = None;
    app.invalidate_fallback();
    Task::none()
}

pub fn handle_set_brush_color(app: &mut PhotoApp, c: iced::Color) -> Task<Message> {
    app.tools.brush_color = c;
    app.tools.color_picker_open = false;
    Task::none()
}
pub fn handle_set_brush_size(app: &mut PhotoApp, s: f32) -> Task<Message> {
    app.tools.brush_size = s;
    Task::none()
}
pub fn handle_set_brush_opacity(app: &mut PhotoApp, o: f32) -> Task<Message> {
    app.tools.brush_opacity = o;
    Task::none()
}
pub fn handle_toggle_picker(app: &mut PhotoApp) -> Task<Message> {
    app.tools.color_picker_open = !app.tools.color_picker_open;
    Task::none()
}
pub fn handle_select_tool(app: &mut PhotoApp, tool: crate::message::Tool) -> Task<Message> {
    // Réactiver la pipette : on mémorise l'outil courant pour y revenir
    // automatiquement après l'échantillonnage. Choisir un autre outil
    // explicitement efface la mémorisation.
    if tool == crate::message::Tool::Eyedropper {
        if app.tools.selected_tool != Tool::Eyedropper {
            app.tools.previous_tool = Some(app.tools.selected_tool);
        }
    } else {
        app.tools.previous_tool = None;
    }
    app.tools.selected_tool = tool;
    app.canvas.canvas_selection = None;
    app.tools.move_anchor = None;
    app.tools.transform_anchor = None;
    Task::none()
}
pub fn handle_toggle_tools(app: &mut PhotoApp) -> Task<Message> {
    app.canvas.tools_visible = !app.canvas.tools_visible;
    Task::none()
}
pub fn handle_set_active_mask(
    app: &mut PhotoApp,
    target: Option<crate::message::MaskTarget>,
) -> Task<Message> {
    app.tools.active_mask = target;
    Task::none()
}
pub fn handle_add_mask(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    // Porteur = calque pixels/groupe OU sous-calque de filtre.
    let can = app
        .document
        .doc
        .find(id)
        .map(|n| {
            matches!(
                n,
                photo_engine::LayerNode::Pixel(_) | photo_engine::LayerNode::Group(_)
            )
        })
        .unwrap_or(false)
        || app.document.doc.find_filter_layer(id).is_some();
    if can {
        // Dimensions source : le sous-calque vit dans l'espace du parent.
        let carrier = app.document.doc.find_filter_parent(id).unwrap_or(id);
        let (w, h) = match app.document.doc.find(carrier) {
            Some(photo_engine::LayerNode::Pixel(l)) => l.dimensions(),
            _ => (
                app.document.doc.width.max(1),
                app.document.doc.height.max(1),
            ),
        };
        let task_id = app.rendering.background_tasks.start("Ajout d'un masque...");
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mask = photo_engine::LayerMask::full(w, h);
                    (id, mask)
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))
            },
            move |res| match res {
                Ok((id, mask)) => Message::AddLayerMaskComputed {
                    task_id,
                    layer_id: id,
                    mask,
                },
                Err(e) => Message::AddLayerMaskFailed { task_id, error: e },
            },
        )
    } else {
        Task::none()
    }
}
pub fn handle_add_mask_computed(
    app: &mut PhotoApp,
    task_id: u64,
    id: Uuid,
    mask: photo_engine::LayerMask,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    let pre = app.snapshot();
    let mask_id = mask.id;
    if let Some(masks) = app.document.doc.masks_of_mut(id) {
        masks.push(mask);
    }
    app.tools.expanded_masks.insert(id);
    app.tools.active_mask = Some(crate::message::MaskTarget {
        layer_id: id,
        mask_id,
    });
    app.document.history.push_snapshot(pre);
    app.invalidate_fallback();
    Task::none()
}

pub fn handle_add_mask_failed(app: &mut PhotoApp, task_id: u64, error: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(error);
    Task::none()
}

pub fn handle_remove_mask(app: &mut PhotoApp, layer_id: Uuid, mask_id: Uuid) -> Task<Message> {
    let existed = app.document.doc.mask_of(layer_id, mask_id).is_some();
    if existed {
        let pre = app.snapshot();
        if let Some(masks) = app.document.doc.masks_of_mut(layer_id) {
            masks.retain(|m| m.id != mask_id);
        }
        if app.tools.active_mask.map(|t| (t.layer_id, t.mask_id)) == Some((layer_id, mask_id)) {
            app.tools.active_mask = None;
        }
        app.document.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_mask_enabled(
    app: &mut PhotoApp,
    layer_id: Uuid,
    mask_id: Uuid,
) -> Task<Message> {
    if let Some(m) = app.document.doc.mask_of(layer_id, mask_id) {
        let cmd = photo_engine::Command::SetMaskEnabled {
            node_id: layer_id,
            mask_id,
            old: m.enabled,
            new: !m.enabled,
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_invert_mask(app: &mut PhotoApp, layer_id: Uuid, mask_id: Uuid) -> Task<Message> {
    if let Some(m) = app.document.doc.mask_of(layer_id, mask_id) {
        let cmd = photo_engine::Command::SetMaskInverted {
            node_id: layer_id,
            mask_id,
            old: m.inverted,
            new: !m.inverted,
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_mask_list(app: &mut PhotoApp, layer_id: Uuid) -> Task<Message> {
    if app.tools.expanded_masks.contains(&layer_id) {
        app.tools.expanded_masks.remove(&layer_id);
    } else {
        app.tools.expanded_masks.insert(layer_id);
    }
    Task::none()
}
pub fn handle_toggle_mask_color(app: &mut PhotoApp) -> Task<Message> {
    app.tools.mask_brush_black = !app.tools.mask_brush_black;
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::BrushStart { x, y, erase } => Some(handle_brush_start(app, x, y, erase)),
        Message::BrushEnd { points, tex, erase } => Some(handle_brush_end(app, points, tex, erase)),
        Message::PaintFailed {
            task_id,
            layer_id,
            mask_id,
        } => Some(handle_paint_failed(app, task_id, layer_id, mask_id)),
        Message::PaintApplied {
            task_id,
            layer_id,
            mask_id,
            buf,
        } => Some(handle_paint_applied(app, task_id, layer_id, mask_id, buf)),
        Message::SetBrushColor(c) => Some(handle_set_brush_color(app, c)),
        Message::SetBrushSize(s) => Some(handle_set_brush_size(app, s)),
        Message::SetBrushOpacity(o) => Some(handle_set_brush_opacity(app, o)),
        Message::ToggleColorPicker => Some(handle_toggle_picker(app)),
        Message::SelectTool(t) => Some(handle_select_tool(app, t)),
        Message::ToggleToolsPanel => Some(handle_toggle_tools(app)),
        Message::SetActiveMask(target) => Some(handle_set_active_mask(app, target)),
        Message::AddLayerMask(id) => Some(handle_add_mask(app, id)),
        Message::AddLayerMaskComputed {
            task_id,
            layer_id,
            mask,
        } => Some(handle_add_mask_computed(app, task_id, layer_id, mask)),
        Message::AddLayerMaskFailed { task_id, error } => {
            Some(handle_add_mask_failed(app, task_id, error))
        }
        Message::RemoveLayerMask(layer_id, mask_id) => {
            Some(handle_remove_mask(app, layer_id, mask_id))
        }
        Message::ToggleLayerMaskEnabled(layer_id, mask_id) => {
            Some(handle_toggle_mask_enabled(app, layer_id, mask_id))
        }
        Message::InvertLayerMask(layer_id, mask_id) => {
            Some(handle_invert_mask(app, layer_id, mask_id))
        }
        Message::ToggleMaskList(layer_id) => Some(handle_toggle_mask_list(app, layer_id)),
        Message::ToggleMaskColor => Some(handle_toggle_mask_color(app)),
        _ => None,
    }
}

/// Pré-dispatch sans clonage — voir `mod.rs`.
pub fn handles(msg: &Message) -> bool {
    matches!(
        msg,
        Message::BrushStart { .. }
            | Message::BrushEnd { .. }
            | Message::PaintFailed { .. }
            | Message::PaintApplied { .. }
            | Message::SetBrushColor(_)
            | Message::SetBrushSize(_)
            | Message::SetBrushOpacity(_)
            | Message::ToggleColorPicker
            | Message::SelectTool(_)
            | Message::ToggleToolsPanel
            | Message::SetActiveMask(_)
            | Message::AddLayerMask(_)
            | Message::AddLayerMaskComputed { .. }
            | Message::AddLayerMaskFailed { .. }
            | Message::RemoveLayerMask(..)
            | Message::ToggleLayerMaskEnabled(..)
            | Message::InvertLayerMask(..)
            | Message::ToggleMaskList(_)
            | Message::ToggleMaskColor
    )
}
