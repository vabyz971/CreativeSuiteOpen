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

//! Layer tree message handlers — extracted from update/mod.rs.

use iced::Task;
use uuid::Uuid;

use crate::layers::{LayerNode, PixelLayer, Transform2D};
use crate::message::{Message, OffsetAxis};
use crate::state::PhotoApp;
use photo_engine::Command;

fn coalesce_key(node_id: Uuid, param: u64) -> u64 {
    (node_id.as_u128() as u64)
        .wrapping_mul(16)
        .wrapping_add(param)
}

fn resolve_target(app: &PhotoApp, id: Uuid) -> Option<Uuid> {
    let target = if id == Uuid::nil() {
        app.selected_layer
    } else {
        Some(id)
    };
    target.filter(|tid| app.doc.find(*tid).is_some())
}

fn rename_duplicate_suffix(doc: &mut photo_engine::Document, new_id: Uuid) {
    if let Some(node) = doc.find_mut(new_id) {
        let base = node.name().to_string();
        node.set_name(format!("{base} copie"));
    }
}

fn crop_layer_to_selection(app: &mut PhotoApp) {
    let Some(sel) = app.canvas_selection else {
        return;
    };
    let Some(id) = app.selected_layer else { return };
    let Some(layer) = app.doc.pixel_layer(id) else {
        return;
    };
    let to_layer = |x: f32, y: f32| {
        let t = layer.transform;
        let cx = x - t.offset_x;
        let cy = y - t.offset_y;
        (cx, cy)
    };
    let (x0, y0) = to_layer(sel.x, sel.y);
    let (x1, y1) = to_layer(sel.x + sel.width, sel.y + sel.height);
    let cx0 = x0.min(x1).round() as i32;
    let cy0 = y0.min(y1).round() as i32;
    let cw = ((x1 - x0).abs().round() as u32).max(1);
    let ch = ((y1 - y0).abs().round() as u32).max(1);
    let pre = app.snapshot();
    match app.doc.crop(id, cx0, cy0, cw, ch) {
        Ok(()) => {
            app.history.push_snapshot(pre);
            app.image_error = None;
            app.invalidate_fallback();
        }
        Err(e) => app.image_error = Some(e),
    }
}

fn add_empty_layer(app: &mut PhotoApp) {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let img = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
        w,
        h,
        image::Rgba([0, 0, 0, 0]),
    ));
    let layer = PixelLayer::new("Calque vide", std::sync::Arc::new(img));
    let id = layer.id;
    let pre = app.snapshot();
    app.doc.push_layer(LayerNode::Pixel(layer));
    app.selected_layer = Some(id);
    app.history.push_snapshot(pre);
    app.invalidate_fallback();
}

fn add_solid_color_layer(app: &mut PhotoApp, color: iced::Color) {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let rgba = image::Rgba([
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    ]);
    let img = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(w, h, rgba));
    let layer = PixelLayer::new("Couleur uni", std::sync::Arc::new(img));
    let id = layer.id;
    let pre = app.snapshot();
    app.doc.push_layer(LayerNode::Pixel(layer));
    app.selected_layer = Some(id);
    app.history.push_snapshot(pre);
    app.invalidate_fallback();
}

pub fn handle_select_layer(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.doc.find(id).map(|n| !n.visible()).unwrap_or(true) {
        return Task::none();
    }
    app.selected_layer = Some(id);
    app.move_anchor = None;
    Task::none()
}

pub fn handle_toggle_visible(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(node) = app.doc.find(id) {
        let new_visible = !node.visible();
        let cmd = Command::SetVisibility {
            node_id: id,
            old: node.visible(),
            new: new_visible,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
        if !new_visible && app.selected_layer == Some(id) {
            app.selected_layer = None;
            app.move_anchor = None;
            app.stroke_layer = None;
        }
    }
    Task::none()
}

pub fn handle_set_opacity(app: &mut PhotoApp, id: Uuid, opacity: f32) -> Task<Message> {
    if let Some(node) = app.doc.find(id) {
        let cmd = Command::SetOpacity {
            layer_id: id,
            old: node.opacity(),
            new: opacity,
        };
        app.history.push_command(coalesce_key(id, 1), cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_set_blend(
    app: &mut PhotoApp,
    id: Uuid,
    mode: crate::layers::BlendMode,
) -> Task<Message> {
    if let Some(node) = app.doc.find(id)
        && let Some(old) = node.blend_mode()
    {
        let cmd = Command::SetBlendMode {
            node_id: id,
            old,
            new: mode,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_rename(app: &mut PhotoApp, id: Uuid, name: String) -> Task<Message> {
    if let Some(node) = app.doc.find(id) {
        let cmd = Command::RenameLayer {
            node_id: id,
            old: node.name().to_string(),
            new: name,
        };
        app.history.push_command(coalesce_key(id, 0), cmd.clone());
        let _ = app.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_offset(
    app: &mut PhotoApp,
    id: Uuid,
    axis: OffsetAxis,
    value: f32,
) -> Task<Message> {
    if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
        let mut new_t = l.transform;
        match axis {
            OffsetAxis::X => new_t.offset_x = value,
            OffsetAxis::Y => new_t.offset_y = value,
        }
        let cmd = Command::SetTransform {
            layer_id: id,
            old: l.transform,
            new: new_t,
        };
        app.history.push_command(coalesce_key(id, 2), cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_set_rotation(app: &mut PhotoApp, id: Uuid, degrees: f32) -> Task<Message> {
    if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
        let cmd = Command::SetTransform {
            layer_id: id,
            old: l.transform,
            new: Transform2D {
                rotation_deg: degrees.clamp(-360.0, 360.0),
                ..l.transform
            },
        };
        app.history.push_command(coalesce_key(id, 3), cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_rotate90(app: &mut PhotoApp, id: Uuid, clockwise: bool) -> Task<Message> {
    let target = resolve_target(app, id);
    let delta = if clockwise { 90.0 } else { -90.0 };
    if let Some(tid) = target
        && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
    {
        let r = (l.transform.rotation_deg + delta + 180.0).rem_euclid(360.0) - 180.0;
        let cmd = Command::SetTransform {
            layer_id: tid,
            old: l.transform,
            new: Transform2D {
                rotation_deg: r,
                ..l.transform
            },
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_flip(app: &mut PhotoApp, id: Uuid, horizontal: bool) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target {
        let pre = app.snapshot();
        match app.doc.flip(tid, horizontal) {
            Ok(()) => app.history.push_snapshot(pre),
            Err(e) => app.image_error = Some(e),
        }
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_rotate(app: &mut PhotoApp, id: Uuid, delta: f32) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target
        && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
    {
        let r = (l.transform.rotation_deg + delta + 180.0).rem_euclid(360.0) - 180.0;
        let new_rot = if r == -180.0 { 180.0 } else { r };
        let cmd = Command::SetTransform {
            layer_id: tid,
            old: l.transform,
            new: Transform2D {
                rotation_deg: new_rot,
                ..l.transform
            },
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_set_scale(app: &mut PhotoApp, id: Uuid, scale: f32) -> Task<Message> {
    if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
        let cmd = Command::SetTransform {
            layer_id: id,
            old: l.transform,
            new: Transform2D {
                scale: scale.clamp(0.05, 8.0),
                ..l.transform
            },
        };
        app.history.push_command(coalesce_key(id, 4), cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_reset_transform(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target
        && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
    {
        let cmd = Command::SetTransform {
            layer_id: tid,
            old: l.transform,
            new: Transform2D {
                rotation_deg: 0.0,
                scale: 1.0,
                ..l.transform
            },
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_crop(app: &mut PhotoApp) -> Task<Message> {
    crop_layer_to_selection(app);
    Task::none()
}
pub fn handle_add_empty(app: &mut PhotoApp) -> Task<Message> {
    add_empty_layer(app);
    Task::none()
}
pub fn handle_add_solid(app: &mut PhotoApp, color: iced::Color) -> Task<Message> {
    add_solid_color_layer(app, color);
    Task::none()
}
pub fn handle_set_dragged(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    app.dragged_layer = Some(id);
    Task::none()
}
pub fn handle_drop_on(app: &mut PhotoApp, target: Uuid) -> Task<Message> {
    if let Some(dragged) = app.dragged_layer.take() {
        if dragged != target && app.doc.find(dragged).is_some() && app.doc.find(target).is_some() {
            let pre = app.snapshot();
            if app.doc.reorder_before(dragged, target, true) {
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
    } else {
        app.dragged_layer = None;
    }
    Task::none()
}
pub fn handle_reorder(
    app: &mut PhotoApp,
    dragged: Uuid,
    target: Uuid,
    before: bool,
) -> Task<Message> {
    if app.doc.reorder_before(dragged, target, before) {
        let pre = app.snapshot();
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_duplicate(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(src) = target {
        let pre = app.snapshot();
        if let Some(new_id) = app.doc.duplicate(src) {
            rename_duplicate_suffix(&mut app.doc, new_id);
            app.selected_layer = Some(new_id);
            app.history.push_snapshot(pre);
            app.invalidate_fallback();
        }
    }
    Task::none()
}
pub fn handle_delete(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(t) = target
        && app.doc.pixel_count() > 1
    {
        let pre = app.snapshot();
        if app.doc.remove(t).is_some() {
            app.selected_layer = app.doc.iter_pixels().last().map(|l| l.id);
            app.history.push_snapshot(pre);
            app.invalidate_fallback();
        }
    }
    Task::none()
}
pub fn handle_move_up(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.doc.move_up(id) {
        let pre = app.snapshot();
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_move_down(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.doc.move_down(id) {
        let pre = app.snapshot();
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_group(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let pre = app.snapshot();
    if let Some(gid) = app.doc.group(&[id]) {
        app.selected_layer = Some(gid);
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_ungroup(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let pre = app.snapshot();
    if let Some(freed) = app.doc.ungroup(id) {
        app.selected_layer = freed.first().copied();
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_collapsed(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(LayerNode::Group(g)) = app.doc.find_mut(id) {
        g.collapsed = !g.collapsed;
    }
    Task::none()
}

pub fn handle_add_live_filter(app: &mut PhotoApp, id: Uuid, type_id: String) -> Task<Message> {
    if let Some(filter) = photo_engine::new_filter(&type_id) {
        let pre = app.snapshot();
        if app.doc.add_filter(id, filter).is_some() {
            app.history.push_snapshot(pre);
            app.invalidate_fallback();
        }
    }
    Task::none()
}

pub fn handle_remove_live_filter(
    app: &mut PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
) -> Task<Message> {
    let pre = app.snapshot();
    if app.doc.remove_filter(layer_id, filter_id).is_some() {
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle_set_filter_param(
    app: &mut PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
    key: String,
    value: datatypes::ParamValue,
) -> Task<Message> {
    // Micro-edit par excellence: light coalesced command.
    // Pixel layer: the appearance recomputes itself via the version cache
    // (zero global recomposite). Adjustment: the global blend changes ->
    // recomposite.
    let old_value = app
        .doc
        .find(layer_id)
        .and_then(|n| n.filters())
        .and_then(|fs| fs.iter().find(|f| f.id == filter_id))
        .and_then(|f| f.params.get(&key))
        .cloned();
    match old_value {
        Some(old) => {
            let cmd = Command::SetFilterParam {
                layer_id,
                filter_id,
                param_name: key.clone(),
                old,
                new: value.clone(),
            };
            app.history
                .push_command(coalesce_key(filter_id, 5), cmd.clone());
            let _ = app.doc.apply_command(cmd);
        }
        None => {
            // Missing parameter (initialization): outside history
            app.doc.set_filter_param(layer_id, filter_id, key, value);
        }
    }
    // Cooked in the fallback composite if it is active; on the fast path it
    // is a simple flag with no cost.
    app.invalidate_fallback();
    Task::none()
}

pub fn handle_toggle_filter_enabled(
    app: &mut PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
) -> Task<Message> {
    let pre = app.snapshot();
    if app.doc.set_filter_enabled(layer_id, filter_id, {
        // invert the current state
        app.doc
            .find(layer_id)
            .and_then(|n| n.filters())
            .and_then(|fs| fs.iter().find(|f| f.id == filter_id))
            .map(|f| !f.enabled)
            .unwrap_or(false)
    }) {
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::SelectLayer(id) => Some(handle_select_layer(app, id)),
        Message::ToggleLayerVisible(id) => Some(handle_toggle_visible(app, id)),
        Message::SetLayerOpacity { id, opacity } => Some(handle_set_opacity(app, id, opacity)),
        Message::SetLayerBlend { id, mode } => Some(handle_set_blend(app, id, mode)),
        Message::RenameLayer { id, name } => Some(handle_rename(app, id, name)),
        Message::SetLayerOffset { id, axis, value } => {
            Some(handle_set_offset(app, id, axis, value))
        }
        Message::SetLayerRotation { id, degrees } => Some(handle_set_rotation(app, id, degrees)),
        Message::RotateLayer90 { id, clockwise } => Some(handle_rotate90(app, id, clockwise)),
        Message::FlipLayer { id, horizontal } => Some(handle_flip(app, id, horizontal)),
        Message::RotateLayer { id, delta } => Some(handle_rotate(app, id, delta)),
        Message::SetLayerScale { id, scale } => Some(handle_set_scale(app, id, scale)),
        Message::ResetLayerTransform(id) => Some(handle_reset_transform(app, id)),
        Message::CropLayerToSelection => Some(handle_crop(app)),
        Message::AddEmptyLayer => Some(handle_add_empty(app)),
        Message::AddSolidColorLayer => {
            let c = app.brush_color;
            Some(handle_add_solid(app, c))
        }
        Message::SetDraggedLayer(id) => Some(handle_set_dragged(app, id)),
        Message::DropLayerOn(id) => Some(handle_drop_on(app, id)),
        Message::ReorderLayer {
            dragged,
            target,
            before,
        } => Some(handle_reorder(app, dragged, target, before)),
        Message::DuplicateLayer(id) => Some(handle_duplicate(app, id)),
        Message::DeleteLayer(id) => Some(handle_delete(app, id)),
        Message::MoveLayerUp(id) => Some(handle_move_up(app, id)),
        Message::MoveLayerDown(id) => Some(handle_move_down(app, id)),
        Message::GroupLayers(id) => Some(handle_group(app, id)),
        Message::UngroupLayers(id) => Some(handle_ungroup(app, id)),
        Message::ToggleGroupCollapsed(id) => Some(handle_toggle_collapsed(app, id)),
        Message::AddLiveFilter { id, type_id } => Some(handle_add_live_filter(app, id, type_id)),
        Message::RemoveLiveFilter {
            layer_id,
            filter_id,
        } => Some(handle_remove_live_filter(app, layer_id, filter_id)),
        Message::SetFilterParam {
            layer_id,
            filter_id,
            key,
            value,
        } => Some(handle_set_filter_param(
            app, layer_id, filter_id, key, value,
        )),
        Message::ToggleFilterEnabled {
            layer_id,
            filter_id,
        } => Some(handle_toggle_filter_enabled(app, layer_id, filter_id)),
        _ => None,
    }
}
