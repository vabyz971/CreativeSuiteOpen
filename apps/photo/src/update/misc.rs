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

//! Misc handlers (preferences, canvas, hardware, fallback) — extracted from update/mod.rs

use iced::{Size, Task, Vector};

use crate::layers::LayerNode;
use crate::message::{Message, Tool};
use crate::state::{PhotoApp, TransformAnchor};
use photo_engine::{Command, UndoAction};
use ui_kit::image_canvas::{Corner, TransformHandle};

fn handle_event(app: &mut PhotoApp, event: iced::Event, window: iced::window::Id) -> Task<Message> {
    // Keys pressed in the preferences window must NEVER reach the document shortcuts.
    if app.is_preferences_window(window) {
        if let Some(w) = &mut app.preferences_window
            && let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) = event
        {
            w.key_event(key, modifiers);
        }
        return Task::none();
    }
    // Global resolution: the subscription only delivers keys NOT consumed by
    // a widget (text fields are therefore safe).
    if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event
        && let Some(action) = app.resolver.resolve(&key, modifiers)
    {
        return super::dispatch(app, Message::ExecuteAction(action));
    }
    Task::none()
}

fn handle_execute_action(app: &mut PhotoApp, action: preferences::PhotoAction) -> Task<Message> {
    // Single typed action -> existing messages bridge (full reuse of the
    // handlers, zero logic duplication).
    let target = || app.selected_layer.unwrap_or_else(uuid::Uuid::nil);
    let msg = match action {
        preferences::PhotoAction::ToolBrush => Message::SelectTool(Tool::Brush),
        preferences::PhotoAction::ToolEraser => Message::SelectTool(Tool::Eraser),
        preferences::PhotoAction::ToolEyedropper => Message::SelectTool(Tool::Select),
        preferences::PhotoAction::ToolMove => Message::SelectTool(Tool::Move),
        preferences::PhotoAction::ToolHand => Message::SelectTool(Tool::Hand),
        preferences::PhotoAction::ToolZoom => Message::SelectTool(Tool::Zoom),
        preferences::PhotoAction::Undo => Message::Undo,
        preferences::PhotoAction::Redo => Message::Redo,
        preferences::PhotoAction::DeleteLayer => Message::DeleteLayer(target()),
        preferences::PhotoAction::NewProject => Message::NewProject,
        preferences::PhotoAction::Open => Message::OpenProject,
        preferences::PhotoAction::Save => Message::SaveProject,
        preferences::PhotoAction::SaveAs => Message::SaveProjectAs,
        preferences::PhotoAction::Export => Message::ExportImage,
        preferences::PhotoAction::ZoomIn => Message::ZoomInPressed,
        preferences::PhotoAction::ZoomOut => Message::ZoomOutPressed,
        preferences::PhotoAction::ZoomFit => Message::CanvasFit,
        preferences::PhotoAction::Zoom100 => {
            app.zoom_level = 100;
            app.canvas_pan = Vector::new(0.0, 0.0);
            app.canvas_selection = None;
            return Task::none();
        }
        preferences::PhotoAction::ToggleLayersPanel => {
            Message::TogglePanel(crate::message::PanelType::Layers)
        }
        preferences::PhotoAction::ToggleToolsPanel => Message::ToggleToolsPanel,
        preferences::PhotoAction::NewLayer => Message::AddEmptyLayer,
        preferences::PhotoAction::DuplicateLayer => Message::DuplicateLayer(target()),
        preferences::PhotoAction::OpenPreferences => Message::OpenPreferences,
    };
    super::dispatch(app, msg)
}

fn handle_hardware_detected(
    app: &mut PhotoApp,
    report: preferences::HardwareReport,
) -> Task<Message> {
    if let Some(window) = &mut app.preferences_window {
        window.set_hardware(report);
    }
    Task::none()
}

fn handle_tick_frame(app: &mut PhotoApp) -> Task<Message> {
    // Spinner (de)animation (~30 fps)
    app.spinner_angle = (app.spinner_angle + 24.0) % 360.0;
    Task::none()
}

fn handle_canvas_fit(app: &mut PhotoApp) -> Task<Message> {
    // Zoom to see the whole image, centered (null pan)
    if let Some((iw, ih)) = app.doc_dims().map(|(w, h)| (w as f32, h as f32)) {
        let vw = app.canvas_viewport.width.max(1.0);
        let vh = app.canvas_viewport.height.max(1.0);
        let fit = (vw / iw).min(vh / ih) * 0.95; // 5% margin
        let zoom = fit.clamp(0.08, 6.0);
        app.zoom_level = (zoom * 100.0).round() as u32;
        app.canvas_pan = Vector::new(0.0, 0.0);
        app.canvas_selection = None;
    }
    Task::none()
}

fn handle_image_canvas_event(
    app: &mut PhotoApp,
    evt: ui_kit::image_canvas::ImageCanvasEvent,
) -> Task<Message> {
    match evt {
        ui_kit::image_canvas::ImageCanvasEvent::BrushStart { x, y, erase } => {
            super::dispatch(app, Message::BrushStart { x, y, erase })
        }
        ui_kit::image_canvas::ImageCanvasEvent::BrushEnd { points, tex, erase } => {
            super::dispatch(app, Message::BrushEnd { points, tex, erase })
        }
        ui_kit::image_canvas::ImageCanvasEvent::Viewport(size) => {
            app.canvas_viewport = size;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::Pan(pan) => {
            if app.selected_tool == Tool::Hand {
                app.canvas_pan = pan;
            }
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
            app.zoom_level = (zoom * 100.0) as u32;
            app.canvas_pan = pan;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
            app.zoom_level = (zoom * 100.0) as u32;
            app.canvas_pan = pan;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
            if app.selected_tool == Tool::Select || app.selected_tool == Tool::Zoom {
                if app.selected_tool == Tool::Zoom {
                    // Zoom on the selected zone
                    if let Some(r) = rect
                        && r.width > 10.0
                        && r.height > 10.0
                    {
                        let sx = 800.0 / r.width;
                        let sy = 600.0 / r.height;
                        let new_zoom =
                            (sx.min(sy) * app.zoom_level as f32 / 100.0).clamp(0.08, 6.0);
                        app.zoom_level = (new_zoom * 100.0) as u32;
                        let cx = r.x + r.width / 2.0 - 400.0;
                        let cy = r.y + r.height / 2.0 - 300.0;
                        app.canvas_pan = Vector::new(-cx, -cy);
                    }
                } else {
                    app.canvas_selection = rect;
                }
            }
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::TransformStart { id, kind, doc } => {
            handle_transform_start(app, id, kind, doc)
        }
        ui_kit::image_canvas::ImageCanvasEvent::TransformCursor { doc, uniform } => {
            handle_transform_cursor(app, doc, uniform)
        }
        ui_kit::image_canvas::ImageCanvasEvent::TransformEnd => handle_transform_end(app),
        ui_kit::image_canvas::ImageCanvasEvent::ClearSelection => {
            app.selected_layer = None;
            app.canvas_selection = None;
            app.expanded_masks.clear();
            app.transform_anchor = None;
            Task::none()
        }
    }
}

fn handle_transform_start(
    app: &mut PhotoApp,
    id: Option<uuid::Uuid>,
    kind: TransformHandle,
    doc: (f32, f32),
) -> Task<Message> {
    // Clic sur un autre calque → le sélectionner AVANT de démarrer le geste.
    // (effets identiques à `Message::SelectLayer`, sans aller-retour Task.)
    let target = match id {
        Some(uid) => {
            if app.selected_layer != Some(uid) {
                app.selected_layer = Some(uid);
                app.active_mask = None;
                app.move_anchor = None;
                app.transform_anchor = None;
            }
            uid
        }
        None => app.selected_layer.unwrap_or_default(),
    };
    if target == uuid::Uuid::nil() {
        return Task::none();
    }
    // Calque masqué → transformation interdite.
    if app.doc.find(target).map(|n| !n.visible()).unwrap_or(true) {
        return Task::none();
    }
    let Some(l) = app.doc.pixel_layer(target) else {
        return Task::none();
    };
    let base = l.transform;
    app.move_anchor = Some((target, base));
    app.transform_anchor = Some(TransformAnchor {
        layer_id: target,
        kind,
        base,
        cursor_doc: doc,
    });
    // Fallback (blending inter-calques) : fond sans ce sous-arbre pré-calculé
    // hors thread pendant que le geste affiche le calque seul — le vrai blend
    // est recalculé UNE fois au relâchement.
    if app.needs_fallback() {
        let has_mask = app
            .doc
            .find(target)
            .map(|n| n.masks().iter().any(|m| m.enabled))
            .unwrap_or(false);
        let mut task = app.drag_background_task(target);
        if has_mask && let Some(t2) = app.drag_layer_composite_task(target) {
            task = Some(match task {
                Some(t1) => Task::batch([t1, t2]),
                None => t2,
            });
        }
        return task.unwrap_or_else(Task::none);
    }
    Task::none()
}

fn handle_transform_cursor(app: &mut PhotoApp, doc: (f32, f32), uniform: bool) -> Task<Message> {
    let Some(anchor) = app.transform_anchor else {
        return Task::none();
    };
    let Some(LayerNode::Pixel(l)) = app.doc.find_mut(anchor.layer_id) else {
        app.transform_anchor = None;
        return Task::none();
    };
    let (w0, h0) = l.dimensions();
    let (w0, h0) = (w0 as f32, h0 as f32);
    let base = anchor.base;
    let new_t = transform_for_cursor(&base, w0, h0, anchor.kind, anchor.cursor_doc, doc, uniform);
    l.transform = new_t;
    // Invalide la fallback stale (contient le calque à l'ancienne position).
    if app.fallback_handle.is_some() {
        app.fallback_handle = None;
        app.fallback_size = None;
    }
    Task::none()
}

fn handle_transform_end(app: &mut PhotoApp) -> Task<Message> {
    app.transform_anchor = None;
    app.drag_bg_in_flight = None;
    // Purge immédiate des buffers drag — la prochaine frame affiche le
    // fallback complet sans artefacts.
    app.drag_background = None;
    app.drag_background_size = None;
    app.drag_layer_composite = None;
    app.drag_layer_composite_size = None;
    // Fin de geste : UNE commande ancre→finale (snapshot au début, aucune
    // pendant le geste). Geste immobile = aucune entrée d'historique.
    if let Some((id, anchor_t)) = app.move_anchor.take()
        && let Some(LayerNode::Pixel(l)) = app.doc.find(id)
        && l.transform != anchor_t
    {
        let cmd = Command::SetTransform {
            layer_id: id,
            old: anchor_t,
            new: l.transform,
        };
        app.history.push_command_immediate(cmd);
    }
    if app.needs_fallback() {
        app.invalidate_fallback();
    }
    Task::none()
}

/// Calcule la transformation d'une position curseur document pour un geste.
fn transform_for_cursor(
    base: &crate::layers::Transform2D,
    w0: f32,
    h0: f32,
    kind: TransformHandle,
    start: (f32, f32),
    cur: (f32, f32),
    uniform: bool,
) -> crate::layers::Transform2D {
    let cx = w0 / 2.0;
    let cy = h0 / 2.0;
    match kind {
        // Déplacement : l'offset suit le delta document 1:1 (déjà en px image).
        TransformHandle::Move => {
            let mut t = *base;
            t.offset_x = base.offset_x + (cur.0 - start.0);
            t.offset_y = base.offset_y + (cur.1 - start.1);
            t
        }
        // Rotation : autour du centre du rectangle scalé, angle Δ depuis le début.
        TransformHandle::Rotate => {
            let center_doc = base.local_to_doc(w0, h0, cx, cy);
            let a0 = (start.1 - center_doc.1).atan2(start.0 - center_doc.0);
            let a1 = (cur.1 - center_doc.1).atan2(cur.0 - center_doc.0);
            let mut t = *base;
            t.rotation_deg = base.rotation_deg + (a1 - a0).to_degrees();
            t
        }
        // Redimensionnement : le coin opposé reste PIVOTÉ (fixe), le coin
        // saisi suit le curseur. L'angle de rotation et les skew sont
        // conservés tels quels (les axes locaux du boîtier ne changent pas).
        TransformHandle::Corner(corner) => {
            let (dc, oc) = corner_pair(corner, w0, h0);
            let o_doc = base.local_to_doc(w0, h0, oc.0, oc.1);
            let kx = base.skew_x.to_radians().tan();
            let ky = base.skew_y.to_radians().tan();
            let rad = base.rotation_deg.to_radians();
            let (cos, sin) = (rad.cos(), rad.sin());
            // b = K^-1 * (R^-1 * (cur - o_doc))
            let wx = cur.0 - o_doc.0;
            let wy = cur.1 - o_doc.1;
            let rx = wx * cos + wy * sin;
            let ry = -wx * sin + wy * cos;
            let det = 1.0 - kx * ky;
            let (b1, b2) = if det.abs() > 1e-4 {
                ((rx - kx * ry) / det, (-ky * rx + ry) / det)
            } else {
                (rx, ry)
            };
            let hx = dc.0 - cx;
            let hy = dc.1 - cy;
            let raw_x = b1 / (2.0 * hx);
            let raw_y = b2 / (2.0 * hy);
            // Ctrl enfoncé → échelle PROPORTIONNELLE : un seul facteur dérivé de
            // l'axe dominant et appliqué aux 2 échelles (aspect conservé).
            // Les clamps durs ne jouent qu'aux bornes extrêmes.
            let (sx, sy) = if uniform {
                let f = if raw_x.abs() >= raw_y.abs() {
                    raw_x
                } else {
                    raw_y
                };
                let dom = if base.scale_x.abs() >= base.scale_y.abs() {
                    base.scale_x.abs()
                } else {
                    base.scale_y.abs()
                };
                // « raw » est une échelle ABSOLUE (b = K·S'·(dc−oc)), pas un
                // facteur : q = f/dom cale l'axe dominant sur le curseur et
                // préserve l'aspect dessiné des 2 axes.
                let q = f / dom.max(1.0e-3);
                (base.scale_x * q, base.scale_y * q)
            } else {
                (raw_x, raw_y)
            };
            let sx = sx.clamp(0.05, 8.0);
            let sy = sy.clamp(0.05, 8.0);
            // C' = o_doc - R*K*(S'*(oc - c0)) puis offset = C' - c0*S'
            let vx = (oc.0 - cx) * sx;
            let vy = (oc.1 - cy) * sy;
            let tx = vx + kx * vy;
            let ty = ky * vx + vy;
            let cxp = o_doc.0 - (tx * cos - ty * sin);
            let cyp = o_doc.1 - (tx * sin + ty * cos);
            let mut t = *base;
            t.offset_x = cxp - cx * sx;
            t.offset_y = cyp - cy * sy;
            t.scale_x = sx;
            t.scale_y = sy;
            t
        }
        // Inclinaison des poignées milieux : angle = atan(delta / hauteur résiduelle).
        TransformHandle::SkewX => {
            let mut t = *base;
            let height = h0 * base.scale_y;
            let delta = (cur.0 - start.0).clamp(-height * 8.0, height * 8.0);
            t.skew_x = (base.skew_x + delta.atan2(height).to_degrees()).clamp(-80.0, 80.0);
            t
        }
        TransformHandle::SkewY => {
            let mut t = *base;
            let width = w0 * base.scale_x;
            let delta = (cur.1 - start.1).clamp(-width * 8.0, width * 8.0);
            t.skew_y = (base.skew_y + delta.atan2(width).to_degrees()).clamp(-80.0, 80.0);
            t
        }
    }
}

/// Coin saisi + coin opposé (local, ordre tl/tr/br/bl comme le canvas).
fn corner_pair(corner: Corner, w0: f32, h0: f32) -> ((f32, f32), (f32, f32)) {
    let corners = [(0.0, 0.0), (w0, 0.0), (w0, h0), (0.0, h0)];
    let idx = match corner {
        Corner::TopLeft => 0,
        Corner::TopRight => 1,
        Corner::BottomRight => 2,
        Corner::BottomLeft => 3,
    };
    let opp = (idx + 2) % 4;
    (corners[idx], corners[opp])
}

fn handle_quit(_app: &mut PhotoApp) -> Task<Message> {
    std::process::exit(0);
}

fn handle_undo_redo(app: &mut PhotoApp, is_undo: bool) -> Task<Message> {
    // Hybrid history: the history applies the inverse itself (undo) or the
    // command (redo) to the document, then describes what to invalidate —
    // full recomposite or nothing (the UI texture cache sync already targets
    // the actually-modified layers).
    let action = if is_undo {
        app.history.undo(&mut app.doc)
    } else {
        app.history.redo(&mut app.doc)
    };
    match action {
        Some(UndoAction::FullRestore) => {
            // Restored structure: the selection may point to a vanished node,
            // we bound it.
            if let Some(sel) = app.selected_layer
                && app.doc.find(sel).is_none()
            {
                app.selected_layer = app.doc.iter_pixels().last().map(|l| l.id);
            }
            app.move_anchor = None;
            app.transform_anchor = None;
            app.drag_background = None;
            app.drag_background_size = None;
            app.pending_paint = None;
            app.stroke_layer = None;
            app.invalidate_fallback();
        }
        Some(UndoAction::Applied(cmd)) if cmd.affects_composite() => {
            // Targeted invalidation: recomposite ONLY if the global blending
            // depends on the touched node
            app.invalidate_fallback();
        }
        Some(UndoAction::Applied(_)) | None => {}
    }
    Task::none()
}

fn handle_fallback_computed(
    app: &mut PhotoApp,
    generation: u64,
    result: Result<Option<(Vec<u8>, u32, u32)>, String>,
) -> Task<Message> {
    app.fallback_in_flight = false;
    if generation != app.fallback_generation {
        // Stale result: the document changed during computation.
        // take_fallback_task will re-emit because dirty is still true.
        return Task::batch([
            Task::none(),
            app.take_fallback_task().unwrap_or_else(Task::none),
        ]);
    }
    match result {
        Ok(Some((rgba, w, h))) => {
            app.fallback_size = Some(Size::new(w as f32, h as f32));
            app.fallback_handle = Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        }
        Ok(None) => {
            app.fallback_handle = None;
            app.fallback_size = None;
        }
        Err(e) => app.image_error = Some(e),
    }
    Task::none()
}

fn handle_drag_background_computed(
    app: &mut PhotoApp,
    layer_id: uuid::Uuid,
    result: Option<(Vec<u8>, u32, u32)>,
) -> Task<Message> {
    app.drag_bg_in_flight = None;
    // Only applies if we are STILL dragging the same subtree
    if app.move_anchor.map(|(id, _)| id) == Some(layer_id)
        && let Some((rgba, w, h)) = result
    {
        app.drag_background = Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        app.drag_background_size = Some(Size::new(w as f32, h as f32));
    }
    Task::none()
}

fn handle_drag_layer_composite_computed(
    app: &mut PhotoApp,
    layer_id: uuid::Uuid,
    result: Option<(Vec<u8>, u32, u32)>,
) -> Task<Message> {
    app.drag_layer_composite_in_flight = false;
    // Valide seulement si on DRAG toujours CE calque — sinon le buffer est
    // orphelin et écrasé au prochain MoveLayerStart.
    if app.move_anchor.map(|(id, _)| id) == Some(layer_id)
        && let Some((rgba, w, h)) = result
    {
        app.drag_layer_composite = Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        app.drag_layer_composite_size = Some(Size::new(w as f32, h as f32));
    }
    Task::none()
}

fn handle_zoom_in(app: &mut PhotoApp) -> Task<Message> {
    app.zoom_level = (app.zoom_level + 10).clamp(5, 1600);
    Task::none()
}

fn handle_zoom_out(app: &mut PhotoApp) -> Task<Message> {
    app.zoom_level = app.zoom_level.saturating_sub(10).max(5);
    Task::none()
}

fn handle_detect_gpu(_app: &mut PhotoApp) -> Task<Message> {
    Task::perform(
        async { crate::components::gpu::detect_gpu_info().await },
        Message::GpuDetected,
    )
}

fn handle_gpu_detected(app: &mut PhotoApp, info: String) -> Task<Message> {
    app.gpu_info = Some(info);
    app.gpu_available = true;
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::Event { event, window } => Some(handle_event(app, event, window)),
        Message::ExecuteAction(action) => Some(handle_execute_action(app, action)),
        Message::HardwareDetected(report) => Some(handle_hardware_detected(app, report)),
        Message::TickFrame => Some(handle_tick_frame(app)),
        Message::CanvasFit => Some(handle_canvas_fit(app)),
        Message::ImageCanvasEvent(evt) => Some(handle_image_canvas_event(app, evt)),
        Message::Quit => Some(handle_quit(app)),
        Message::Undo => Some(handle_undo_redo(app, true)),
        Message::Redo => Some(handle_undo_redo(app, false)),
        Message::FallbackComputed { generation, result } => {
            Some(handle_fallback_computed(app, generation, result))
        }
        Message::DragBackgroundComputed { layer_id, result } => {
            Some(handle_drag_background_computed(app, layer_id, result))
        }
        Message::DragLayerCompositeComputed { layer_id, result } => {
            Some(handle_drag_layer_composite_computed(app, layer_id, result))
        }
        Message::ZoomInPressed => Some(handle_zoom_in(app)),
        Message::ZoomOutPressed => Some(handle_zoom_out(app)),
        Message::MockAction => Some(Task::none()),
        Message::DetectGpu => Some(handle_detect_gpu(app)),
        Message::GpuDetected(info) => Some(handle_gpu_detected(app, info)),
        _ => None,
    }
}
