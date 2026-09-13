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

//! Layer tree message handlers — extracted from update/mod.rs.

use iced::Task;
use std::sync::Arc;
use uuid::Uuid;

use crate::components::layers::{
    DropPosition, LayerDragRelease, LayerDragState, LayerDropTarget, resolve_drop_target,
};
use crate::layers::{LayerNode, PixelLayer, Transform2D};
use crate::message::{AppearanceToggle, Message, OffsetAxis, PendingParam};
use crate::state::PhotoApp;
use photo_engine::Command;

fn coalesce_key(node_id: Uuid, param: u64) -> u64 {
    (node_id.as_u128() as u64)
        .wrapping_mul(16)
        .wrapping_add(param)
}

fn resolve_target(app: &PhotoApp, id: Uuid) -> Option<Uuid> {
    let target = if id == Uuid::nil() {
        app.document.selected_layer
    } else {
        Some(id)
    };
    target.and_then(|tid| {
        // Un sous-calque de filtre redirige vers son calque porteur pour
        // les opérations canvas (rotation, flip, crop…).
        if app.document.doc.find(tid).is_some() {
            Some(tid)
        } else {
            app.document.doc.find_filter_parent(tid)
        }
    })
}

fn create_default_mask(
    doc: &photo_engine::Document,
    target: Uuid,
) -> Option<photo_engine::LayerMask> {
    // Utilise la taille du calque porteur.
    let (w, h) = match doc.find(target) {
        Some(photo_engine::LayerNode::Pixel(l)) => l.dimensions(),
        _ => (doc.width.max(1), doc.height.max(1)),
    };
    Some(photo_engine::LayerMask::full(w, h))
}

fn resolve_context_target(app: &PhotoApp, id: Uuid) -> Uuid {
    if id == Uuid::nil() {
        app.document.selected_layer.unwrap_or(id)
    } else {
        id
    }
}

fn rename_duplicate_suffix(doc: &mut photo_engine::Document, new_id: Uuid) {
    if let Some(node) = doc.find_mut(new_id) {
        let base = node.name().to_string();
        node.set_name(format!("{base} copie"));
    }
}

fn rename_duplicate_suffix_filter(doc: &mut photo_engine::Document, new_id: Uuid) {
    if let Some(f) = doc.find_filter_layer_mut(new_id) {
        let base = f.name.clone();
        f.name = format!("{base} copie");
    }
}

fn handle_add_empty(app: &mut PhotoApp) -> Task<Message> {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let task_id = app
        .rendering
        .background_tasks
        .start("Création d'un calque vide...");

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let img = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
                    w,
                    h,
                    image::Rgba([0, 0, 0, 0]),
                ));
                let layer = PixelLayer::new("Calque vide", Arc::new(img));
                crate::message::DecodedLayer(layer)
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))
        },
        move |res| Message::ImageDecoded {
            task_id,
            result: res,
        },
    )
}

fn handle_add_solid(app: &mut PhotoApp, color: iced::Color) -> Task<Message> {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let rgba = image::Rgba([
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    ]);
    let task_id = app
        .rendering
        .background_tasks
        .start("Création d'un calque de couleur...");

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let img =
                    image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(w, h, rgba));
                let layer = PixelLayer::new("Couleur uni", Arc::new(img));
                crate::message::DecodedLayer(layer)
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))
        },
        move |res| Message::ImageDecoded {
            task_id,
            result: res,
        },
    )
}

pub fn handle_select_layer(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    // Nœud ou sous-calque de filtre — les invisibles/désactivés refusent.
    let selectable = match app.document.doc.find(id) {
        Some(n) => n.visible(),
        None => app
            .document
            .doc
            .find_filter_layer(id)
            .map(|f| f.enabled)
            .unwrap_or(false),
    };
    if !selectable {
        return Task::none();
    }
    app.document.selected_layer = Some(id);
    // Contexte actif unique : sélectionner un calque quitte l'édition de masque.
    app.tools.active_mask = None;
    app.tools.move_anchor = None;
    app.tools.transform_anchor = None;
    Task::none()
}

pub fn handle_toggle_visible(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(node) = app.document.doc.find(id) {
        let new_visible = !node.visible();
        let cmd = Command::SetVisibility {
            node_id: id,
            old: node.visible(),
            new: new_visible,
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
        if !new_visible && app.document.selected_layer == Some(id) {
            app.document.selected_layer = None;
            app.tools.move_anchor = None;
            app.tools.transform_anchor = None;
            app.tools.stroke_layer = None;
        }
    }
    Task::none()
}

pub fn handle_set_opacity(app: &mut PhotoApp, id: Uuid, opacity: f32) -> Task<Message> {
    // Polymorphe : nœud ou sous-calque de filtre (même Command, appli
    // routée côté moteur + touch du porteur).
    if let Some(old) = app.document.doc.opacity_of(id) {
        let cmd = Command::SetOpacity {
            layer_id: id,
            old,
            new: opacity,
        };
        app.document
            .history
            .push_command(coalesce_key(id, 1), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_blend(
    app: &mut PhotoApp,
    id: Uuid,
    mode: crate::layers::BlendMode,
) -> Task<Message> {
    if let Some(old) = app.document.doc.blend_of(id) {
        let cmd = Command::SetBlendMode {
            node_id: id,
            old,
            new: mode,
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_rename(app: &mut PhotoApp, id: Uuid, name: String) -> Task<Message> {
    let old = app
        .document
        .doc
        .find(id)
        .map(|n| n.name().to_string())
        .or_else(|| {
            app.document
                .doc
                .find_filter_layer(id)
                .map(|f| f.name.clone())
        })
        .or_else(|| app.document.doc.mask_name(id));
    if let Some(old) = old {
        let cmd = Command::RenameLayer {
            node_id: id,
            old,
            new: name,
        };
        app.document
            .history
            .push_command(coalesce_key(id, 0), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_offset(
    app: &mut PhotoApp,
    id: Uuid,
    axis: OffsetAxis,
    value: f32,
) -> Task<Message> {
    // Polymorphe : pixels et sous-calques de filtres portent un Transform2D.
    if let Some(t) = app.document.doc.transform_of(id) {
        let mut new_t = t;
        match axis {
            OffsetAxis::X => new_t.offset_x = value,
            OffsetAxis::Y => new_t.offset_y = value,
        }
        let cmd = Command::SetTransform {
            layer_id: id,
            old: t,
            new: new_t,
        };
        app.document
            .history
            .push_command(coalesce_key(id, 2), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_rotation(app: &mut PhotoApp, id: Uuid, degrees: f32) -> Task<Message> {
    if let Some(t) = app.document.doc.transform_of(id) {
        let cmd = Command::SetTransform {
            layer_id: id,
            old: t,
            new: Transform2D {
                rotation_deg: degrees.clamp(-360.0, 360.0),
                ..t
            },
        };
        app.document
            .history
            .push_command(coalesce_key(id, 3), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_rotate90(app: &mut PhotoApp, id: Uuid, clockwise: bool) -> Task<Message> {
    let target = resolve_target(app, id);
    let delta = if clockwise { 90.0 } else { -90.0 };
    if let Some(tid) = target
        && let Some(LayerNode::Pixel(l)) = app.document.doc.find(tid)
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
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_flip(app: &mut PhotoApp, id: Uuid, horizontal: bool) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target {
        let op = if horizontal {
            crate::message::DestructiveOp::FlipHorizontal
        } else {
            crate::message::DestructiveOp::FlipVertical
        };
        // On ne clone QUE les Arc (bon marché, zéro copie pixels) : le
        // déréférencement + fliph/flipv du buffer complet se font dans le
        // worker. Le Document n'est pas Sync, on ne peut pas l'expédier.
        let (source, masks, filter_masks) = match app.document.doc.pixel_layer(tid) {
            Some(l) => {
                let source = Arc::clone(&l.source_image);
                let masks: Vec<Arc<image::RgbaImage>> =
                    l.masks.iter().map(|m| Arc::clone(&m.image)).collect();
                let filter_masks: Vec<(Uuid, Vec<Arc<image::RgbaImage>>)> = l
                    .filter_layers
                    .iter()
                    .map(|f| (f.id, f.masks.iter().map(|m| Arc::clone(&m.image)).collect()))
                    .collect();
                (source, masks, filter_masks)
            }
            None => return Task::none(),
        };
        let task_id = app.rendering.background_tasks.start("Miroir du calque...");
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut flipped = source.to_rgba8();
                    if horizontal {
                        image::imageops::flip_horizontal_in_place(&mut flipped);
                    } else {
                        image::imageops::flip_vertical_in_place(&mut flipped);
                    }
                    let mut masks_rgba = Vec::with_capacity(masks.len());
                    for m in &masks {
                        let mut copy = (**m).clone();
                        if horizontal {
                            image::imageops::flip_horizontal_in_place(&mut copy);
                        } else {
                            image::imageops::flip_vertical_in_place(&mut copy);
                        }
                        masks_rgba.push(copy);
                    }
                    let mut filter_masks_rgba = Vec::with_capacity(filter_masks.len());
                    for (fid, fms) in &filter_masks {
                        let mut out = Vec::with_capacity(fms.len());
                        for m in fms {
                            let mut copy = (**m).clone();
                            if horizontal {
                                image::imageops::flip_horizontal_in_place(&mut copy);
                            } else {
                                image::imageops::flip_vertical_in_place(&mut copy);
                            }
                            out.push(copy);
                        }
                        filter_masks_rgba.push((*fid, out));
                    }
                    Ok(crate::message::DestructiveResult {
                        source: flipped,
                        masks: masks_rgba,
                        filter_masks: filter_masks_rgba,
                        offset_delta: (0.0, 0.0),
                    })
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))?
            },
            move |res| Message::DestructiveOpComputed {
                task_id,
                layer_id: tid,
                op,
                result: res,
            },
        )
    } else {
        Task::none()
    }
}

pub fn handle_rotate(app: &mut PhotoApp, id: Uuid, delta: f32) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target
        && let Some(t) = app.document.doc.transform_of(tid)
    {
        let r = (t.rotation_deg + delta + 180.0).rem_euclid(360.0) - 180.0;
        let new_rot = if r == -180.0 { 180.0 } else { r };
        let cmd = Command::SetTransform {
            layer_id: tid,
            old: t,
            new: Transform2D {
                rotation_deg: new_rot,
                ..t
            },
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_scale_axis(
    app: &mut PhotoApp,
    id: Uuid,
    axis: crate::OffsetAxis,
    scale: f32,
) -> Task<Message> {
    if let Some(t) = app.document.doc.transform_of(id) {
        let scale = scale.clamp(0.05, 8.0);
        let new = match axis {
            crate::OffsetAxis::X => Transform2D {
                scale_x: scale,
                ..t
            },
            crate::OffsetAxis::Y => Transform2D {
                scale_y: scale,
                ..t
            },
        };
        let cmd = Command::SetTransform {
            layer_id: id,
            old: t,
            new,
        };
        app.document
            .history
            .push_command(coalesce_key(id, 4), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_set_skew(
    app: &mut PhotoApp,
    id: Uuid,
    axis: crate::OffsetAxis,
    deg: f32,
) -> Task<Message> {
    if let Some(t) = app.document.doc.transform_of(id) {
        let deg = deg.clamp(-80.0, 80.0);
        let new = match axis {
            crate::OffsetAxis::X => Transform2D { skew_x: deg, ..t },
            crate::OffsetAxis::Y => Transform2D { skew_y: deg, ..t },
        };
        let cmd = Command::SetTransform {
            layer_id: id,
            old: t,
            new,
        };
        app.document
            .history
            .push_command(coalesce_key(id, 4), cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_reset_transform(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let target = resolve_target(app, id);
    if let Some(tid) = target
        && let Some(t) = app.document.doc.transform_of(tid)
    {
        let cmd = Command::SetTransform {
            layer_id: tid,
            old: t,
            new: Transform2D {
                rotation_deg: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                skew_x: 0.0,
                skew_y: 0.0,
                ..t
            },
        };
        app.document.history.push_command_immediate(cmd.clone());
        let _ = app.document.doc.apply_command(cmd);
    }
    Task::none()
}

pub fn handle_crop(app: &mut PhotoApp) -> Task<Message> {
    let target = resolve_target(app, app.document.selected_layer.unwrap_or(Uuid::nil()));
    if let Some(tid) = target {
        let Some(sel) = app.canvas.canvas_selection else {
            return Task::none();
        };
        let Some(layer) = app.document.doc.pixel_layer(tid) else {
            return Task::none();
        };
        let t = layer.transform;
        let x0 = (sel.x - t.offset_x).min(sel.x + sel.width - t.offset_x);
        let y0 = (sel.y - t.offset_y).min(sel.y + sel.height - t.offset_y);
        let x1 = (sel.x - t.offset_x).max(sel.x + sel.width - t.offset_x);
        let y1 = (sel.y - t.offset_y).max(sel.y + sel.height - t.offset_y);
        let (iw, ih) = (
            layer.source_image.width() as i32,
            layer.source_image.height() as i32,
        );
        let mut cx = x0.round().max(0.0) as i32;
        let mut cy = y0.round().max(0.0) as i32;
        let mut cw = ((x1 - x0).abs().round() as i32).max(1);
        let mut ch = ((y1 - y0).abs().round() as i32).max(1);
        // Clamp aux bornes du calque (la version sync du moteur le faisait
        // aussi) — évite le panic de `image::imageops::crop_imm`.
        if cx < 0 {
            cw += cx;
            cx = 0;
        }
        if cy < 0 {
            ch += cy;
            cy = 0;
        }
        if cx + cw > iw {
            cw = iw - cx;
        }
        if cy + ch > ih {
            ch = ih - cy;
        }
        if cw <= 0 || ch <= 0 {
            app.canvas.image_error = Some("Rognage : sélection hors calque".into());
            return Task::none();
        }
        let (cx_u, cy_u, cw_u, ch_u) = (cx as u32, cy as u32, cw as u32, ch as u32);
        let dx = cx as f32;
        let dy = cy as f32;

        // Ne capturer que les Arc (rendus) : to_rgba8 et crop se font dans
        // le worker pour ne pas geler le thread UI.
        let source = Arc::clone(&layer.source_image);
        let masks: Vec<Arc<image::RgbaImage>> =
            layer.masks.iter().map(|m| Arc::clone(&m.image)).collect();
        let filter_masks: Vec<(Uuid, Vec<Arc<image::RgbaImage>>)> = layer
            .filter_layers
            .iter()
            .map(|f| (f.id, f.masks.iter().map(|m| Arc::clone(&m.image)).collect()))
            .collect();
        let task_id = app.rendering.background_tasks.start("Rognage du calque...");
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let buf = source.to_rgba8();
                    let cropped =
                        image::imageops::crop_imm(&buf, cx_u, cy_u, cw_u, ch_u).to_image();
                    let mut masks_rgba = Vec::with_capacity(masks.len());
                    for m in &masks {
                        let copy = (**m).clone();
                        let cm =
                            image::imageops::crop_imm(&copy, cx_u, cy_u, cw_u, ch_u).to_image();
                        masks_rgba.push(cm);
                    }
                    let mut filter_masks_rgba = Vec::with_capacity(filter_masks.len());
                    for (fid, fms) in &filter_masks {
                        let mut out = Vec::with_capacity(fms.len());
                        for m in fms {
                            let copy = (**m).clone();
                            let cm =
                                image::imageops::crop_imm(&copy, cx_u, cy_u, cw_u, ch_u).to_image();
                            out.push(cm);
                        }
                        filter_masks_rgba.push((*fid, out));
                    }
                    Ok(crate::message::DestructiveResult {
                        source: cropped,
                        masks: masks_rgba,
                        filter_masks: filter_masks_rgba,
                        offset_delta: (dx, dy),
                    })
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))?
            },
            move |res| Message::DestructiveOpComputed {
                task_id,
                layer_id: tid,
                op: crate::message::DestructiveOp::Crop,
                result: res,
            },
        )
    } else {
        Task::none()
    }
}

pub fn handle_layer_drag_pressed(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    app.tools.layer_drag = LayerDragState::press(id);
    Task::none()
}

pub fn handle_layer_drag_moved(app: &mut PhotoApp, position: (f32, f32)) -> Task<Message> {
    app.tools
        .layer_drag
        .moved(iced::Point::new(position.0, position.1));
    Task::none()
}

pub fn handle_layer_drag_hover(
    app: &mut PhotoApp,
    hovered: Option<Uuid>,
    position: DropPosition,
) -> Task<Message> {
    let target = match (app.tools.layer_drag.dragged_id(), hovered) {
        (Some(dragged), Some(hovered)) => {
            resolve_drop_target(&app.document.doc, dragged, hovered, position)
        }
        _ => None,
    };
    app.tools.layer_drag.hover(target);
    Task::none()
}

pub fn handle_layer_drag_released(app: &mut PhotoApp) -> Task<Message> {
    let (release, _) = app.tools.layer_drag.release();
    app.tools.layer_drag = LayerDragState::Idle;
    match release {
        LayerDragRelease::None => Task::none(),
        LayerDragRelease::Select(id) => handle_select_layer(app, id),
        LayerDragRelease::Commit { layer_id, target } => {
            handle_layer_drop_commit(app, layer_id, target)
        }
    }
}

pub fn handle_layer_drag_cancelled(app: &mut PhotoApp) -> Task<Message> {
    app.tools.layer_drag = LayerDragState::Idle;
    Task::none()
}

pub fn handle_layer_row_hovered(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    app.tools.hovered_layer_row = Some(id);
    // Pendant un drag, le survol d'un corps de ligne vaut proposition Inside
    // pour les groupes, ou effacement pour les autres lignes.
    if matches!(app.tools.layer_drag, LayerDragState::Dragging { .. }) {
        let target = match app.tools.layer_drag.dragged_id() {
            Some(dragged) => resolve_drop_target(
                &app.document.doc,
                dragged,
                id,
                crate::components::layers::DropPosition::Inside,
            ),
            None => None,
        };
        app.tools.layer_drag.hover(target);
    }
    Task::none()
}

pub fn handle_layer_row_unhovered(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.tools.hovered_layer_row == Some(id) {
        app.tools.hovered_layer_row = None;
    }
    // Quitter le corps d'un groupe abandonne sa cible Inside éventuelle.
    if app.tools.layer_drag.target() == Some(crate::components::layers::LayerDropTarget::Inside(id))
    {
        app.tools.layer_drag.hover(None);
    }
    Task::none()
}

fn handle_layer_drop_commit(
    app: &mut PhotoApp,
    layer_id: Uuid,
    target: LayerDropTarget,
) -> Task<Message> {
    let pre = app.snapshot();
    let moved = match target {
        LayerDropTarget::Before(target) => app.document.doc.reorder_before(layer_id, target, true),
        LayerDropTarget::After(target) => app.document.doc.reorder_before(layer_id, target, false),
        LayerDropTarget::Inside(group) => app.document.doc.move_into(layer_id, group),
    };
    if moved {
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}

pub fn handle_destructive_op_computed(
    app: &mut PhotoApp,
    task_id: u64,
    layer_id: Uuid,
    _op: crate::message::DestructiveOp,
    result: Result<crate::message::DestructiveResult, String>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    match result {
        Ok(r) => {
            let pre = app.snapshot();
            // Source : remplace via l'API moteur (cache-friendly). Le buffer
            // est déjà un RgbaImage PROPRE — wrap en DynamicImage (zéro copie).
            if app
                .document
                .doc
                .set_source_image(layer_id, image::DynamicImage::ImageRgba8(r.source))
            {
                // Masques : remplace un par un (le moteur n'a pas d'API
                // batch, mais l'opération est O(N_masks) avec N petit).
                if let Some(LayerNode::Pixel(layer)) = app.document.doc.find_mut(layer_id) {
                    for (i, new_mask) in r.masks.into_iter().enumerate() {
                        if let Some(m) = layer.masks.get_mut(i) {
                            m.image = Arc::new(new_mask);
                            m.touch();
                        }
                    }
                    // Masques des sous-calques (même espace source)
                    for (fid, fms) in r.filter_masks.into_iter() {
                        if let Some(f) = layer.filter_layers.iter_mut().find(|f| f.id == fid) {
                            for (i, new_mask) in fms.into_iter().enumerate() {
                                if let Some(m) = f.masks.get_mut(i) {
                                    m.image = Arc::new(new_mask);
                                    m.touch();
                                }
                            }
                        }
                    }
                    // Crop : compense l'origine monde (le pixel (x,y)
                    // d'origine reste à sa place).
                    if r.offset_delta != (0.0, 0.0) {
                        layer.transform.offset_x += r.offset_delta.0;
                        layer.transform.offset_y += r.offset_delta.1;
                    }
                }
                app.document.history.push_snapshot(pre);
            }
        }
        Err(e) => app.canvas.image_error = Some(e),
    }
    Task::none()
}
pub fn handle_duplicate(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    // Sous-calque de filtre : clone juste au-dessus dans le même parent.
    if let Some(parent) = app.document.doc.find_filter_parent(id) {
        let pre = app.snapshot();
        if let Some(new_id) = app.document.doc.duplicate_filter(parent, id) {
            rename_duplicate_suffix_filter(&mut app.document.doc, new_id);
            app.document.selected_layer = Some(new_id);
            app.document.history.push_snapshot(pre);
        }
        return Task::none();
    }
    let target = resolve_target(app, id);
    if let Some(src) = target {
        let pre = app.snapshot();
        if let Some(new_id) = app.document.doc.duplicate(src) {
            rename_duplicate_suffix(&mut app.document.doc, new_id);
            app.document.selected_layer = Some(new_id);
            app.document.history.push_snapshot(pre);
        }
    }
    Task::none()
}
pub fn handle_delete(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    // Sous-calque de filtre : suppression dans le parent (pas de garde
    // pixel_count — un calque peut perdre tous ses filtres).
    if let Some(parent) = app.document.doc.find_filter_parent(id) {
        let pre = app.snapshot();
        if app.document.doc.remove_filter(parent, id).is_some() {
            app.document.selected_layer = Some(parent);
            if app
                .rendering
                .pending_param
                .as_ref()
                .is_some_and(|p| p.layer_id == parent && p.filter_id == id)
            {
                app.rendering.pending_param = None;
            }
            app.document.history.push_snapshot(pre);
        }
        return Task::none();
    }
    let target = resolve_target(app, id);
    if let Some(t) = target
        && app.document.doc.pixel_count() > 1
    {
        let pre = app.snapshot();
        if app.document.doc.remove(t).is_some() {
            app.document.selected_layer = app.document.doc.iter_pixels().last().map(|l| l.id);
            if app
                .rendering
                .pending_param
                .as_ref()
                .is_some_and(|p| p.layer_id == t)
            {
                app.rendering.pending_param = None;
            }
            app.document.history.push_snapshot(pre);
        }
    }
    Task::none()
}
pub fn handle_move_up(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    // Sous-calque : remonte dans la chaîne du parent (= appliqué plus tard).
    if let Some(parent) = app.document.doc.find_filter_parent(id) {
        let pre = app.snapshot();
        if app.document.doc.move_filter(parent, id, true) {
            app.document.history.push_snapshot(pre);
        }
        return Task::none();
    }
    if app.document.doc.move_up(id) {
        let pre = app.snapshot();
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}
pub fn handle_move_down(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(parent) = app.document.doc.find_filter_parent(id) {
        let pre = app.snapshot();
        if app.document.doc.move_filter(parent, id, false) {
            app.document.history.push_snapshot(pre);
        }
        return Task::none();
    }
    if app.document.doc.move_down(id) {
        let pre = app.snapshot();
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}
/// Déplace un masque dans la liste de son porteur (up = vers le haut).
pub fn handle_move_mask(
    app: &mut PhotoApp,
    owner_id: Uuid,
    mask_id: Uuid,
    up: bool,
) -> Task<Message> {
    let pre = app.snapshot();
    if app.document.doc.move_mask(owner_id, mask_id, up) {
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}
pub fn handle_group(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let pre = app.snapshot();
    if let Some(gid) = app.document.doc.group(&[id]) {
        app.document.selected_layer = Some(gid);
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}
pub fn handle_ungroup(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let pre = app.snapshot();
    if let Some(freed) = app.document.doc.ungroup(id) {
        app.document.selected_layer = freed.first().copied();
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}
pub fn handle_toggle_collapsed(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(LayerNode::Group(g)) = app.document.doc.find_mut(id) {
        g.collapsed = !g.collapsed;
    }
    Task::none()
}

pub fn handle_toggle_fx_stack(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.tools.expanded_fx_stack.contains(&id) {
        app.tools.expanded_fx_stack.remove(&id);
    } else {
        app.tools.expanded_fx_stack.insert(id);
    }
    Task::none()
}

pub fn handle_toggle_filter_menu(app: &mut PhotoApp) -> Task<Message> {
    app.tools.filter_menu_open = !app.tools.filter_menu_open;
    Task::none()
}

pub fn handle_add_live_filter(app: &mut PhotoApp, id: Uuid, type_id: String) -> Task<Message> {
    if let Some(filter) = photo_engine::new_filter_layer(&type_id) {
        let pre = app.snapshot();
        if let Some(fid) = app.document.doc.add_filter(id, filter) {
            // Pixels : le parent se déplie et le nouveau sous-calque est
            // sélectionné (édition directe dans Propriétés). Ajustements :
            // la sélection reste (liste classique dans Propriétés).
            if app.document.doc.pixel_layer(id).is_some() {
                app.tools.expanded_fx_stack.insert(id);
                app.document.selected_layer = Some(fid);
                app.tools.active_mask = None;
            }
            app.tools.filter_menu_open = false;
            app.document.history.push_snapshot(pre);
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
    if app
        .document
        .doc
        .remove_filter(layer_id, filter_id)
        .is_some()
    {
        if app.document.selected_layer == Some(filter_id) {
            app.document.selected_layer = Some(layer_id);
        }
        // Le filtre n'existe plus : aucun réglage en attente ne peut
        // aboutir — le pouce retombe (la réception réconcilie aussi).
        if app
            .rendering
            .pending_param
            .as_ref()
            .is_some_and(|p| p.layer_id == layer_id && p.filter_id == filter_id)
        {
            app.rendering.pending_param = None;
        }
        app.document.history.push_snapshot(pre);
    }
    Task::none()
}

/// Lecture d'un paramètre de filtre (pixels : sous-calques, ajustements :
/// chaîne simple).
fn old_filter_param(
    app: &PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
    key: &str,
) -> Option<datatypes::ParamValue> {
    match app.document.doc.find(layer_id) {
        Some(LayerNode::Pixel(l)) => l
            .filter_layers
            .iter()
            .find(|f| f.id == filter_id)?
            .params
            .get(key)
            .cloned(),
        _ => app
            .document
            .doc
            .find(layer_id)?
            .filters()?
            .iter()
            .find(|f| f.id == filter_id)?
            .params
            .get(key)
            .cloned(),
    }
}

fn current_filter_enabled(app: &PhotoApp, layer_id: Uuid, filter_id: Uuid) -> Option<bool> {
    match app.document.doc.find(layer_id) {
        Some(LayerNode::Pixel(l)) => l
            .filter_layers
            .iter()
            .find(|f| f.id == filter_id)
            .map(|f| f.enabled),
        _ => app
            .document
            .doc
            .find(layer_id)?
            .filters()?
            .iter()
            .find(|f| f.id == filter_id)
            .map(|f| f.enabled),
    }
}

/// Pré-chauffe un réglage de filtre HORS thread UI (front montant) :
/// le clone reçoit la valeur en attente, l'entrée chaude est insérée à
/// la réception. Le vivant garde l'ancienne valeur entre-temps (zéro
/// freeze des ticks), le pouce affiche `pending_param`.
fn spawn_param_warm(app: &mut PhotoApp, pending: PendingParam) -> Task<Message> {
    let Some(carrier) = warm_carrier(app, pending.layer_id) else {
        return Task::none();
    };
    if !app.rendering.warm_inflight.insert(carrier) {
        return Task::none();
    }
    let epoch = app.rendering.param_epoch;
    let task_id = app.rendering.background_tasks.start("Réglage du filtre...");
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    doc_copy.warm_cache_from(&app.document.doc);
    // Clones dédiés au worker : `pending` reste propriété de la closure
    // de réception (pas de double move).
    let worker_key = pending.key.clone();
    let worker_value = pending.value.clone();
    let worker_layer = pending.layer_id;
    let worker_filter = pending.filter_id;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                if !doc_copy.set_filter_param(worker_layer, worker_filter, worker_key, worker_value)
                {
                    return Err("Filtre introuvable".to_string());
                }
                doc_copy
                    .appearance(carrier)
                    .ok_or_else(|| "Aucune apparence à pré-chauffer".to_string())?;
                doc_copy
                    .export_warmed(carrier)
                    .ok_or_else(|| "Cache d'apparence vide".to_string())
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))?
        },
        move |result| Message::ParamWarmed {
            task_id,
            epoch,
            layer_id: pending.layer_id,
            filter_id: pending.filter_id,
            key: pending.key,
            value: pending.value,
            result,
        },
    )
}
/// Enchaîne le front descendant : une valeur plus fraîche attend-elle ?
fn chain_param(app: &mut PhotoApp, next: Option<PendingParam>) -> Task<Message> {
    match next {
        Some(p) => spawn_param_warm(app, p),
        None => Task::none(),
    }
}

/// Réception d'un réglage pré-chauffé : applique la valeur sur le vivant
/// (logique historique inchangée : commande coalescée ou init directe),
/// insère l'entrée chaude, puis enchaîne une valeur plus fraîche si le
/// slider a bougé pendant le calcul. `epoch` périmé (undo/redo) : tout
/// est jeté, le vivant garde l'état annulé.
fn handle_param_warmed(
    app: &mut PhotoApp,
    task_id: u64,
    epoch: u64,
    target: PendingParam,
    result: Result<photo_engine::WarmedAppearance, String>,
) -> Task<Message> {
    let PendingParam {
        layer_id,
        filter_id,
        key,
        value,
    } = target;
    app.rendering.background_tasks.finish(task_id);
    finish_warm(app, layer_id);
    // Réconciliation du front descendant EN PREMIER (toujours, même en
    // cas d'échec ou de porteur disparu) : sinon le pouce resterait
    // coincé sur une valeur fantôme.
    let target = PendingParam {
        layer_id,
        filter_id,
        key: key.clone(),
        value: value.clone(),
    };
    let next = match &app.rendering.pending_param {
        Some(p) if p != &target => Some(p.clone()),
        _ => {
            app.rendering.pending_param = None;
            None
        }
    };
    if epoch != app.rendering.param_epoch {
        return Task::none();
    }
    let Some(carrier) = warm_carrier(app, layer_id) else {
        return chain_param(app, next);
    };
    // Application live : logique historique inchangée.
    let old_value = old_filter_param(app, layer_id, filter_id, &key);
    match old_value {
        Some(old) => {
            let cmd = Command::SetFilterParam {
                layer_id,
                filter_id,
                param_name: key.clone(),
                old,
                new: value.clone(),
            };
            app.document
                .history
                .push_command(coalesce_key(filter_id, 5), cmd.clone());
            let _ = app.document.doc.apply_command(cmd);
        }
        None => {
            // Missing parameter (initialization): outside history
            app.document
                .doc
                .set_filter_param(layer_id, filter_id, key, value);
        }
    }
    // Entrée périmée (édition concurrente) : reste inutilisée — affichage
    // juste, un recalcul ponctuel (comportement antérieur).
    if let Ok(warmed) = result {
        app.document.doc.insert_warmed(carrier, warmed);
    }
    chain_param(app, next)
}

pub fn handle_set_filter_param(
    app: &mut PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
    key: String,
    value: datatypes::ParamValue,
) -> Task<Message> {
    // Calque pixels : appliquer live MISSrait le cache et rejouerait la
    // chaîne en pleine résolution à CHAQUE tick (freeze du slider sur
    // grandes images). Le vivant garde l'ancienne valeur (sync HIT), le
    // pouce affiche `pending_param`, le worker pré-chauffe la nouvelle.
    let Some(carrier) = warm_carrier(app, layer_id) else {
        let old_value = old_filter_param(app, layer_id, filter_id, &key);
        match old_value {
            Some(old) => {
                let cmd = Command::SetFilterParam {
                    layer_id,
                    filter_id,
                    param_name: key.clone(),
                    old,
                    new: value.clone(),
                };
                app.document
                    .history
                    .push_command(coalesce_key(filter_id, 5), cmd.clone());
                let _ = app.document.doc.apply_command(cmd);
            }
            None => {
                // Missing parameter (initialization): outside history
                app.document
                    .doc
                    .set_filter_param(layer_id, filter_id, key, value);
            }
        }
        return Task::none();
    };
    let pending = PendingParam {
        layer_id,
        filter_id,
        key,
        value,
    };
    app.rendering.pending_param = Some(pending.clone());
    // Rafale : un vol est déjà en cours, le front descendant enchaînera.
    if app.rendering.warm_inflight.contains(&carrier) {
        return Task::none();
    }
    spawn_param_warm(app, pending)
}

/// Calque pixels dont l'apparence doit être pré-chauffée pour un toggle.
/// Les masques peuvent vivre sur un sous-calque de filtre : le calcul
/// vise alors le porteur pixels. `None` = ajustement/groupe/supprimé :
/// pas d'entrée d'apparence, application live immédiate (zéro coût).
pub(crate) fn warm_carrier(app: &PhotoApp, owner: Uuid) -> Option<Uuid> {
    match app.document.doc.find(owner) {
        Some(LayerNode::Pixel(_)) => Some(owner),
        _ => app.document.doc.find_filter_parent(owner),
    }
}

/// Applique le flag sur le document (clone worker OU vivant) — sans
/// historique ni invalidation, gérés par l'appelant selon le contexte.
/// Retourne `false` si la cible a disparu.
/// Les deux côtés appliquent EXACTEMENT la même mutation (bit flip sans
/// `touch()` pour les masques) afin de converger vers la même signature.
fn apply_toggle_flag(doc: &mut photo_engine::Document, owner: Uuid, op: AppearanceToggle) -> bool {
    match op {
        AppearanceToggle::FilterEnabled { filter_id, enabled } => {
            doc.set_filter_enabled(owner, filter_id, enabled)
        }
        // PAS de `touch()` ici : la version fait partie de
        // `mask_signature`, worker et vivant doivent converger vers la
        // MÊME version pour que l'entrée pré-chauffée HIT. La version
        // inchangée devient détecteur d'édition concurrente : toute
        // peinture/`touch()` intercalé fait diverger → MISS sain.
        AppearanceToggle::MaskEnabled { mask_id, enabled } => {
            let Some(m) = doc.mask_of_mut(owner, mask_id) else {
                return false;
            };
            m.enabled = enabled;
            true
        }
        AppearanceToggle::MaskInverted { mask_id, inverted } => {
            let Some(m) = doc.mask_of_mut(owner, mask_id) else {
                return false;
            };
            m.inverted = inverted;
            true
        }
    }
}

/// Pré-chauffe l'apparence HORS thread UI après un toggle (filtre/shader
/// ou masque) : le flag est appliqué sur un clone en `spawn_blocking`,
/// l'entrée chaude est insérée à la réception — `PreviewCache::sync()`
/// HIT au lieu d'exécuter `render_chain` + bake + preview/thumb en pleine
/// résolution sur l'UI (freeze sur grandes images).
/// L'appelant a vérifié `warm_carrier(...).is_some()` et calculé `op`
/// (nouvel état déjà inversé). Le libellé alimente le spinner.
pub(crate) fn warm_toggle_task(
    app: &mut PhotoApp,
    owner: Uuid,
    op: AppearanceToggle,
    label: impl Into<String>,
) -> Task<Message> {
    let Some(carrier) = warm_carrier(app, owner) else {
        return Task::none();
    };
    if !app.rendering.warm_inflight.insert(carrier) {
        return Task::none();
    }
    let task_id = app.rendering.background_tasks.start(label);
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    doc_copy.warm_cache_from(&app.document.doc);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                if !apply_toggle_flag(&mut doc_copy, owner, op) {
                    return Err("Calque introuvable".to_string());
                }
                doc_copy
                    .appearance(carrier)
                    .ok_or_else(|| "Aucune apparence à pré-chauffer".to_string())?;
                doc_copy
                    .export_warmed(carrier)
                    .ok_or_else(|| "Cache d'apparence vide".to_string())
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))?
        },
        move |result| Message::AppearanceWarmed {
            task_id,
            layer_id: owner,
            op,
            result,
        },
    )
}

/// État courant du flag visé par un toggle (`None` = cible disparue).
fn toggle_current(app: &PhotoApp, owner: Uuid, op: AppearanceToggle) -> Option<bool> {
    match op {
        AppearanceToggle::FilterEnabled { filter_id, .. } => {
            current_filter_enabled(app, owner, filter_id)
        }
        AppearanceToggle::MaskEnabled { mask_id, .. } => {
            app.document.doc.mask_of(owner, mask_id).map(|m| m.enabled)
        }
        AppearanceToggle::MaskInverted { mask_id, .. } => {
            app.document.doc.mask_of(owner, mask_id).map(|m| m.inverted)
        }
    }
}

/// Nouvel état visé par un toggle.
fn toggle_desired(op: AppearanceToggle) -> bool {
    match op {
        AppearanceToggle::FilterEnabled { enabled, .. } => enabled,
        AppearanceToggle::MaskEnabled { enabled, .. } => enabled,
        AppearanceToggle::MaskInverted { inverted, .. } => inverted,
    }
}

/// Libère la garde anti-empilement d'un porteur : entrée existante →
/// retrait ciblé, porteur disparu → purge des orphelines (les UUID ne
/// sont jamais réutilisés, simple hygiène du set).
fn finish_warm(app: &mut PhotoApp, owner: Uuid) {
    if let Some(carrier) = warm_carrier(app, owner) {
        app.rendering.warm_inflight.remove(&carrier);
    } else {
        let doc = &app.document.doc;
        app.rendering
            .warm_inflight
            .retain(|id| doc.find(*id).is_some());
    }
}

/// Réception d'une apparence pré-chauffée : rejoue le flag sur le vivant
/// (bon marché), insère l'entrée chaude, enregistre l'historique comme
/// l'ancien chemin synchrone. Si l'état a divergé pendant le calcul
/// (édition concurrente), l'entrée reste inutilisée — affichage juste,
/// un recalcul ponctuel (comportement antérieur).
fn handle_appearance_warmed(
    app: &mut PhotoApp,
    task_id: u64,
    owner: Uuid,
    op: AppearanceToggle,
    result: Result<photo_engine::WarmedAppearance, String>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    finish_warm(app, owner);
    let warmed = match result {
        Ok(w) => w,
        Err(e) => {
            app.canvas.image_error = Some(e);
            return Task::none();
        }
    };
    let Some(carrier) = warm_carrier(app, owner) else {
        return Task::none();
    };
    // Cible disparue ou déjà dans l'état visé : rien à rejouer.
    if toggle_current(app, owner, op) != Some(!toggle_desired(op)) {
        return Task::none();
    }
    if !apply_toggle_flag(&mut app.document.doc, owner, op) {
        return Task::none();
    }
    match op {
        AppearanceToggle::FilterEnabled { filter_id, .. } => {
            if app.document.selected_layer == Some(filter_id)
                && current_filter_enabled(app, owner, filter_id) == Some(false)
            {
                app.document.selected_layer = Some(owner);
            }
            app.document.history.push_snapshot(app.snapshot());
        }
        AppearanceToggle::MaskEnabled { .. } | AppearanceToggle::MaskInverted { .. } => {
            // L'historique des masques passe par commandes immédiates :
            // le flag vient de basculer !new → new, la commande miroir
            // porte old=!new (réversible par undo/redo).
            push_mask_toggle_history(app, owner, op);
        }
    }
    app.document.doc.insert_warmed(carrier, warmed);
    Task::none()
}

/// Enregistre le toggle de masque dans l'historique immédiat (miroir de
/// l'ancien chemin synchrone : la commande porte old→new).
fn push_mask_toggle_history(app: &mut PhotoApp, owner: Uuid, op: AppearanceToggle) {
    let cmd = match op {
        AppearanceToggle::MaskEnabled {
            mask_id,
            enabled: new,
        } => Command::SetMaskEnabled {
            node_id: owner,
            mask_id,
            old: !new,
            new,
        },
        AppearanceToggle::MaskInverted {
            mask_id,
            inverted: new,
        } => Command::SetMaskInverted {
            node_id: owner,
            mask_id,
            old: !new,
            new,
        },
        AppearanceToggle::FilterEnabled { .. } => return,
    };
    app.document.history.push_command_immediate(cmd);
}

pub fn handle_toggle_filter_enabled(
    app: &mut PhotoApp,
    layer_id: Uuid,
    filter_id: Uuid,
) -> Task<Message> {
    // invert the current state
    let new = current_filter_enabled(app, layer_id, filter_id)
        .map(|e| !e)
        .unwrap_or(false);
    let op = AppearanceToggle::FilterEnabled {
        filter_id,
        enabled: new,
    };
    // Ajustement (pas d'apparence pixels) : application live immédiate,
    // zéro coût côté affichage (le draw GPU relit l'arbre).
    if warm_carrier(app, layer_id).is_none() {
        let pre = app.snapshot();
        if app
            .document
            .doc
            .set_filter_enabled(layer_id, filter_id, new)
        {
            if app.document.selected_layer == Some(filter_id)
                && current_filter_enabled(app, layer_id, filter_id) == Some(false)
            {
                app.document.selected_layer = Some(layer_id);
            }
            app.document.history.push_snapshot(pre);
        }
        return Task::none();
    }
    // Calque pixels : le flip de signature MISSrait le cache d'apparence
    // et exécuterait render_chain + bake en pleine résolution sur l'UI —
    // pré-chauffage hors thread UI à la place.
    let label = if new {
        "Activation du filtre..."
    } else {
        "Désactivation du filtre..."
    };
    warm_toggle_task(app, layer_id, op, label)
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
        Message::SetLayerScaleAxis { id, axis, scale } => {
            Some(handle_set_scale_axis(app, id, axis, scale))
        }
        Message::SetLayerSkew { id, axis, degrees } => {
            Some(handle_set_skew(app, id, axis, degrees))
        }
        Message::ResetLayerTransform(id) => Some(handle_reset_transform(app, id)),
        Message::CropLayerToSelection => Some(handle_crop(app)),
        Message::AddEmptyLayer => Some(handle_add_empty(app)),
        Message::AddSolidColorLayer => {
            let c = app.tools.brush_color;
            Some(handle_add_solid(app, c))
        }
        Message::DestructiveOpComputed {
            task_id,
            layer_id,
            op,
            result,
        } => Some(handle_destructive_op_computed(
            app, task_id, layer_id, op, result,
        )),
        Message::LayerDragPressed { id } => Some(handle_layer_drag_pressed(app, id)),
        Message::LayerDragMoved { position } => Some(handle_layer_drag_moved(app, position)),
        Message::LayerDragHover { hovered, position } => {
            Some(handle_layer_drag_hover(app, hovered, position))
        }
        Message::LayerDragReleased => Some(handle_layer_drag_released(app)),
        Message::LayerDragCancelled => Some(handle_layer_drag_cancelled(app)),
        Message::LayerRowHovered(id) => Some(handle_layer_row_hovered(app, id)),
        Message::LayerRowUnhovered(id) => Some(handle_layer_row_unhovered(app, id)),
        Message::DuplicateLayer(id) => Some(handle_duplicate(app, id)),
        Message::DeleteLayer(id) => Some(handle_delete(app, id)),
        Message::MoveLayerUp(id) => Some(handle_move_up(app, id)),
        Message::MoveLayerDown(id) => Some(handle_move_down(app, id)),
        Message::GroupLayers(id) => Some(handle_group(app, id)),
        Message::UngroupLayers(id) => Some(handle_ungroup(app, id)),
        Message::ToggleGroupCollapsed(id) => Some(handle_toggle_collapsed(app, id)),
        Message::ToggleFxStack(id) => Some(handle_toggle_fx_stack(app, id)),
        Message::ToggleFilterMenu => Some(handle_toggle_filter_menu(app)),
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
        Message::AppearanceWarmed {
            task_id,
            layer_id,
            op,
            result,
        } => Some(handle_appearance_warmed(app, task_id, layer_id, op, result)),
        Message::ParamWarmed {
            task_id,
            epoch,
            layer_id,
            filter_id,
            key,
            value,
            result,
        } => Some(handle_param_warmed(
            app,
            task_id,
            epoch,
            PendingParam {
                layer_id,
                filter_id,
                key,
                value,
            },
            result,
        )),
        Message::MoveMask {
            owner_id,
            mask_id,
            up,
        } => Some(handle_move_mask(app, owner_id, mask_id, up)),
        Message::OpenContextMenu { layer_id, .. } => {
            app.tools.context_menu_open = Some(layer_id);
            Some(Task::none())
        }
        Message::CloseContextMenu => {
            app.tools.context_menu_open = None;
            Some(Task::none())
        }
        Message::ContextAddMask => {
            if let Some(id) = app.tools.context_menu_open {
                let target = resolve_context_target(app, id);
                let _ = app.snapshot();
                let _ = target;
                // Trigger the async mask creation flow (same as the
                // toolbar mask button) — the mask is created by a paint
                // stroke; the user can confirm/undo later.
                let task_id = app.rendering.background_tasks.start("Ajout du masque...");
                // We delegate to the existing AddLayerMask handler path
                // by posting the computed result manually.
                let pre = app.snapshot();
                if let Some(mask) = create_default_mask(&app.document.doc, target) {
                    if let Some(masks) = app.document.doc.masks_of_mut(target) {
                        masks.push(mask);
                    }
                    app.tools.expanded_fx_stack.insert(target);
                    app.document.history.push_snapshot(pre);
                }
                app.rendering.background_tasks.finish(task_id);
            }
            app.tools.context_menu_open = None;
            Some(Task::none())
        }
        Message::ContextAddFilter => {
            if let Some(id) = app.tools.context_menu_open {
                let target = resolve_context_target(app, id);
                if app.document.doc.pixel_layer(target).is_some() {
                    app.tools.filter_menu_open = true;
                }
            }
            app.tools.context_menu_open = None;
            Some(Task::none())
        }
        Message::ContextMoveUp => {
            if let Some(id) = app.tools.context_menu_open {
                let pre = app.snapshot();
                if app.document.doc.move_up(id) {
                    app.document.history.push_snapshot(pre);
                }
            }
            app.tools.context_menu_open = None;
            Some(Task::none())
        }
        Message::ContextMoveDown => {
            if let Some(id) = app.tools.context_menu_open {
                let pre = app.snapshot();
                if app.document.doc.move_down(id) {
                    app.document.history.push_snapshot(pre);
                }
            }
            app.tools.context_menu_open = None;
            Some(Task::none())
        }
        Message::ContextToggleVisible => {
            let task = if let Some(id) = app.tools.context_menu_open {
                handle_toggle_visible(app, id)
            } else {
                Task::none()
            };
            app.tools.context_menu_open = None;
            Some(task)
        }
        _ => None,
    }
}

pub fn handles(msg: &Message) -> bool {
    matches!(
        msg,
        Message::SelectLayer(_)
            | Message::ToggleLayerVisible(_)
            | Message::SetLayerOpacity { .. }
            | Message::SetLayerBlend { .. }
            | Message::RenameLayer { .. }
            | Message::SetLayerOffset { .. }
            | Message::SetLayerRotation { .. }
            | Message::RotateLayer90 { .. }
            | Message::FlipLayer { .. }
            | Message::RotateLayer { .. }
            | Message::SetLayerScaleAxis { .. }
            | Message::SetLayerSkew { .. }
            | Message::ResetLayerTransform(_)
            | Message::CropLayerToSelection
            | Message::AddEmptyLayer
            | Message::AddSolidColorLayer
            | Message::DestructiveOpComputed { .. }
            | Message::LayerDragPressed { .. }
            | Message::LayerDragMoved { .. }
            | Message::LayerDragHover { .. }
            | Message::LayerDragReleased
            | Message::LayerDragCancelled
            | Message::LayerRowHovered(_)
            | Message::LayerRowUnhovered(_)
            | Message::DuplicateLayer(_)
            | Message::DeleteLayer(_)
            | Message::MoveLayerUp(_)
            | Message::MoveLayerDown(_)
            | Message::GroupLayers(_)
            | Message::UngroupLayers(_)
            | Message::ToggleGroupCollapsed(_)
            | Message::ToggleFxStack(_)
            | Message::ToggleFilterMenu
            | Message::AddLiveFilter { .. }
            | Message::RemoveLiveFilter { .. }
            | Message::SetFilterParam { .. }
            | Message::ToggleFilterEnabled { .. }
            | Message::AppearanceWarmed { .. }
            | Message::ParamWarmed { .. }
            | Message::MoveMask { .. }
            | Message::OpenContextMenu { .. }
            | Message::CloseContextMenu
            | Message::ContextAddMask
            | Message::ContextAddFilter
            | Message::ContextMoveUp
            | Message::ContextMoveDown
            | Message::ContextToggleVisible
    )
}
