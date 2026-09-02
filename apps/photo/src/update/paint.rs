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
use crate::state::PhotoApp;
use iced::Task;
use std::sync::Arc;
use uuid::Uuid;

pub fn handle_brush_start(app: &mut PhotoApp, x: f32, y: f32, erase: bool) -> Task<Message> {
    let _ = (x, y, erase);
    if app.pending_paint.is_none()
        && let Some(id) = app.selected_layer
        && app.doc.pixel_layer(id).is_some()
        && app.doc.find(id).map(|n| n.visible()).unwrap_or(false)
    {
        app.stroke_layer = Some(id);
    }
    Task::none()
}

pub fn handle_brush_end(
    app: &mut PhotoApp,
    points: Vec<(f32, f32)>,
    tex: Option<ui_kit::image_canvas::StrokeTex>,
    erase: bool,
) -> Task<Message> {
    let stroke_target = app.stroke_layer.take();
    if let Some(id) = stroke_target
        && app.pending_paint.is_none()
        && points.len() > 1
        && let Some(tex) = tex
    {
        let active_mask = app.active_mask.filter(|t| t.layer_id == id);
        let stroke_mask_id = active_mask
            .filter(|t| app.doc.find(id).and_then(|n| n.mask(t.mask_id)).is_some())
            .map(|t| t.mask_id);
        let is_mask = stroke_mask_id.is_some();
        let (source, transform) = if is_mask {
            let m = app
                .doc
                .find(id)
                .and_then(|n| n.mask(stroke_mask_id.unwrap()))
                .unwrap();
            let dyn_img = image::DynamicImage::ImageRgba8((*m.image).clone());
            (
                Arc::new(dyn_img),
                match app.doc.find(id).unwrap() {
                    photo_engine::LayerNode::Pixel(l) => l.transform,
                    _ => crate::layers::Transform2D::default(),
                },
            )
        } else if let Some(layer) = app.doc.pixel_layer(id) {
            (Arc::clone(&layer.source_image), layer.transform)
        } else {
            return Task::none();
        };
        let pts = points;
        app.pending_paint = Some(crate::message::PendingPaint {
            layer_id: id,
            mask_id: stroke_mask_id,
            tex: tex.clone(),
        });
        app.history.push_snapshot(app.snapshot());
        // Sur un masque (façon Affinity) : le pinceau peint la couleur du
        // toggle — noir = masque (cache), blanc = révèle ; la gomme révèle
        // toujours (blanc). Mode Paint pour écrire la couverture (canal R).
        let (stroke_color, stroke_mode) = if is_mask {
            let reveal = erase || !app.mask_brush_black;
            (
                if reveal { [255, 255, 255] } else { [0, 0, 0] },
                photo_engine::paint::StrokeMode::Paint,
            )
        } else {
            let c = app.brush_color;
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
            radius: app.brush_size / 2.0,
            color: stroke_color,
            opacity: app.brush_opacity,
            mode: stroke_mode,
        };
        return Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    photo_engine::paint::commit_stroke(&source, &pts, &transform, &brush)
                })
                .await
            },
            move |result| match result {
                Ok(buf) => Message::PaintApplied {
                    layer_id: id,
                    mask_id: stroke_mask_id,
                    buf,
                },
                Err(_) => Message::PaintFailed {
                    layer_id: id,
                    mask_id: stroke_mask_id,
                },
            },
        );
    }
    Task::none()
}

pub fn handle_paint_failed(
    app: &mut PhotoApp,
    layer_id: Uuid,
    mask_id: Option<Uuid>,
) -> Task<Message> {
    if app
        .pending_paint
        .as_ref()
        .is_some_and(|p| p.layer_id == layer_id && p.mask_id == mask_id)
    {
        app.pending_paint = None;
    }
    app.image_error = Some("Échec interne lors de l'application du trait".into());
    Task::none()
}

pub fn handle_paint_applied(
    app: &mut PhotoApp,
    layer_id: Uuid,
    mask_id: Option<Uuid>,
    buf: photo_engine::paint::StrokeCommit,
) -> Task<Message> {
    if let Some(img) = image::RgbaImage::from_raw(buf.width, buf.height, buf.rgba) {
        if let Some(mask_id) = mask_id {
            if let Some(mask) = app.doc.find_mut(layer_id).and_then(|n| n.mask_mut(mask_id)) {
                mask.image = Arc::new(img);
                mask.touch();
            }
        } else {
            app.doc
                .set_source_image(layer_id, image::DynamicImage::ImageRgba8(img));
        }
    }
    app.pending_paint = None;
    app.invalidate_fallback();
    Task::none()
}

pub fn handle_set_brush_color(app: &mut PhotoApp, c: iced::Color) -> Task<Message> {
    app.brush_color = c;
    app.color_picker_open = false;
    Task::none()
}
pub fn handle_set_brush_size(app: &mut PhotoApp, s: f32) -> Task<Message> {
    app.brush_size = s;
    Task::none()
}
pub fn handle_set_brush_opacity(app: &mut PhotoApp, o: f32) -> Task<Message> {
    app.brush_opacity = o;
    Task::none()
}
pub fn handle_toggle_picker(app: &mut PhotoApp) -> Task<Message> {
    app.color_picker_open = !app.color_picker_open;
    Task::none()
}
pub fn handle_select_tool(app: &mut PhotoApp, tool: crate::message::Tool) -> Task<Message> {
    app.selected_tool = tool;
    app.canvas_selection = None;
    app.move_anchor = None;
    app.transform_anchor = None;
    Task::none()
}
pub fn handle_toggle_tools(app: &mut PhotoApp) -> Task<Message> {
    app.tools_visible = !app.tools_visible;
    Task::none()
}
pub fn handle_set_active_mask(
    app: &mut PhotoApp,
    target: Option<crate::message::MaskTarget>,
) -> Task<Message> {
    app.active_mask = target;
    Task::none()
}
pub fn handle_add_mask(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let can = app
        .doc
        .find(id)
        .map(|n| {
            matches!(
                n,
                photo_engine::LayerNode::Pixel(_) | photo_engine::LayerNode::Group(_)
            )
        })
        .unwrap_or(false);
    if can {
        let (w, h) = match app.doc.find(id) {
            Some(photo_engine::LayerNode::Pixel(l)) => l.dimensions(),
            _ => (app.doc.width.max(1), app.doc.height.max(1)),
        };
        app.background_tasks
            .push("Ajout d'un masque...".to_string());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mask = photo_engine::LayerMask::full(w, h);
                    (id, mask)
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))
            },
            |res| match res {
                Ok((id, mask)) => Message::AddLayerMaskComputed { layer_id: id, mask },
                Err(e) => Message::ImageDecoded(Err(e)),
            },
        )
    } else {
        Task::none()
    }
}
pub fn handle_add_mask_computed(
    app: &mut PhotoApp,
    id: Uuid,
    mask: photo_engine::LayerMask,
) -> Task<Message> {
    app.background_tasks
        .retain(|t| !t.starts_with("Ajout d'un masque"));
    let pre = app.snapshot();
    let mask_id = mask.id;
    if let Some(masks) = app.doc.find_mut(id).and_then(|n| n.masks_mut()) {
        masks.push(mask);
    }
    app.expanded_masks.insert(id);
    app.active_mask = Some(crate::message::MaskTarget {
        layer_id: id,
        mask_id,
    });
    app.history.push_snapshot(pre);
    app.invalidate_fallback();
    Task::none()
}

pub fn handle_remove_mask(app: &mut PhotoApp, layer_id: Uuid, mask_id: Uuid) -> Task<Message> {
    let existed = app
        .doc
        .find(layer_id)
        .and_then(|n| n.mask(mask_id))
        .is_some();
    if existed {
        let pre = app.snapshot();
        if let Some(masks) = app.doc.find_mut(layer_id).and_then(|n| n.masks_mut()) {
            masks.retain(|m| m.id != mask_id);
        }
        if app.active_mask.map(|t| (t.layer_id, t.mask_id)) == Some((layer_id, mask_id)) {
            app.active_mask = None;
        }
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_mask_enabled(
    app: &mut PhotoApp,
    layer_id: Uuid,
    mask_id: Uuid,
) -> Task<Message> {
    if let Some(m) = app.doc.find(layer_id).and_then(|n| n.mask(mask_id)) {
        let cmd = photo_engine::Command::SetMaskEnabled {
            node_id: layer_id,
            mask_id,
            old: m.enabled,
            new: !m.enabled,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_invert_mask(app: &mut PhotoApp, layer_id: Uuid, mask_id: Uuid) -> Task<Message> {
    if let Some(m) = app.doc.find(layer_id).and_then(|n| n.mask(mask_id)) {
        let cmd = photo_engine::Command::SetMaskInverted {
            node_id: layer_id,
            mask_id,
            old: m.inverted,
            new: !m.inverted,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_mask_list(app: &mut PhotoApp, layer_id: Uuid) -> Task<Message> {
    if app.expanded_masks.contains(&layer_id) {
        app.expanded_masks.remove(&layer_id);
    } else {
        app.expanded_masks.insert(layer_id);
    }
    Task::none()
}
pub fn handle_toggle_mask_color(app: &mut PhotoApp) -> Task<Message> {
    app.mask_brush_black = !app.mask_brush_black;
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::BrushStart { x, y, erase } => Some(handle_brush_start(app, x, y, erase)),
        Message::BrushEnd { points, tex, erase } => Some(handle_brush_end(app, points, tex, erase)),
        Message::PaintFailed { layer_id, mask_id } => {
            Some(handle_paint_failed(app, layer_id, mask_id))
        }
        Message::PaintApplied {
            layer_id,
            mask_id,
            buf,
        } => Some(handle_paint_applied(app, layer_id, mask_id, buf)),
        Message::SetBrushColor(c) => Some(handle_set_brush_color(app, c)),
        Message::SetBrushSize(s) => Some(handle_set_brush_size(app, s)),
        Message::SetBrushOpacity(o) => Some(handle_set_brush_opacity(app, o)),
        Message::ToggleColorPicker => Some(handle_toggle_picker(app)),
        Message::SelectTool(t) => Some(handle_select_tool(app, t)),
        Message::ToggleToolsPanel => Some(handle_toggle_tools(app)),
        Message::SetActiveMask(target) => Some(handle_set_active_mask(app, target)),
        Message::AddLayerMask(id) => Some(handle_add_mask(app, id)),
        Message::AddLayerMaskComputed { layer_id, mask } => {
            Some(handle_add_mask_computed(app, layer_id, mask))
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
