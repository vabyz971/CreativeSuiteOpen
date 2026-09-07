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
        if let Some(w) = &mut app.windows.preferences_window
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
        && let Some(action) = app.windows.resolver.resolve(&key, modifiers)
    {
        return super::dispatch(app, Message::ExecuteAction(action));
    }
    Task::none()
}

fn handle_execute_action(app: &mut PhotoApp, action: preferences::PhotoAction) -> Task<Message> {
    // Single typed action -> existing messages bridge (full reuse of the
    // handlers, zero logic duplication).
    let target = || app.document.selected_layer.unwrap_or_else(uuid::Uuid::nil);
    let msg = match action {
        preferences::PhotoAction::ToolBrush => Message::SelectTool(Tool::Brush),
        preferences::PhotoAction::ToolEraser => Message::SelectTool(Tool::Eraser),
        preferences::PhotoAction::ToolEyedropper => Message::SelectTool(Tool::Eyedropper),
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
            app.canvas.zoom_level = 100;
            app.canvas.canvas_pan = Vector::new(0.0, 0.0);
            app.canvas.canvas_selection = None;
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
    if let Some(window) = &mut app.windows.preferences_window {
        window.set_hardware(report);
    }
    Task::none()
}

fn handle_tick_frame(app: &mut PhotoApp) -> Task<Message> {
    // Spinner (de)animation (~30 fps)
    app.rendering.spinner_angle = (app.rendering.spinner_angle + 24.0) % 360.0;
    Task::none()
}

fn handle_canvas_fit(app: &mut PhotoApp) -> Task<Message> {
    // Zoom to see the whole image, centered (null pan)
    if let Some((iw, ih)) = app.doc_dims().map(|(w, h)| (w as f32, h as f32)) {
        let vw = app.canvas.canvas_viewport.width.max(1.0);
        let vh = app.canvas.canvas_viewport.height.max(1.0);
        let fit = (vw / iw).min(vh / ih) * 0.95; // 5% margin
        let zoom = fit.clamp(0.08, 6.0);
        app.canvas.zoom_level = (zoom * 100.0).round() as u32;
        app.canvas.canvas_pan = Vector::new(0.0, 0.0);
        app.canvas.canvas_selection = None;
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
        ui_kit::image_canvas::ImageCanvasEvent::ColorPick { x, y } => handle_pick_color(app, x, y),
        ui_kit::image_canvas::ImageCanvasEvent::Viewport(size) => {
            app.canvas.canvas_viewport = size;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::Pan(pan) => {
            if app.tools.selected_tool == Tool::Hand {
                app.canvas.canvas_pan = pan;
            }
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
            app.canvas.zoom_level = (zoom * 100.0) as u32;
            app.canvas.canvas_pan = pan;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
            app.canvas.zoom_level = (zoom * 100.0) as u32;
            app.canvas.canvas_pan = pan;
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
            if app.tools.selected_tool == Tool::Select || app.tools.selected_tool == Tool::Zoom {
                if app.tools.selected_tool == Tool::Zoom {
                    // Zoom on the selected zone
                    if let Some(r) = rect
                        && r.width > 10.0
                        && r.height > 10.0
                    {
                        let sx = 800.0 / r.width;
                        let sy = 600.0 / r.height;
                        let new_zoom =
                            (sx.min(sy) * app.canvas.zoom_level as f32 / 100.0).clamp(0.08, 6.0);
                        app.canvas.zoom_level = (new_zoom * 100.0) as u32;
                        let cx = r.x + r.width / 2.0 - 400.0;
                        let cy = r.y + r.height / 2.0 - 300.0;
                        app.canvas.canvas_pan = Vector::new(-cx, -cy);
                    }
                } else {
                    app.canvas.canvas_selection = rect;
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
            app.document.selected_layer = None;
            app.canvas.canvas_selection = None;
            app.tools.expanded_masks.clear();
            app.tools.expanded_filters.clear();
            app.tools.transform_anchor = None;
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
            if app.document.selected_layer != Some(uid) {
                app.document.selected_layer = Some(uid);
                app.tools.active_mask = None;
                app.tools.move_anchor = None;
                app.tools.transform_anchor = None;
            }
            uid
        }
        None => app.document.selected_layer.unwrap_or_default(),
    };
    if target == uuid::Uuid::nil() {
        return Task::none();
    }
    // Sous-calque de filtre → geste sur le calque porteur.
    let target = app
        .document
        .doc
        .find_filter_parent(target)
        .unwrap_or(target);
    // Calque masqué → transformation interdite.
    if app
        .document
        .doc
        .find(target)
        .map(|n| !n.visible())
        .unwrap_or(true)
    {
        return Task::none();
    }
    let Some(l) = app.document.doc.pixel_layer(target) else {
        return Task::none();
    };
    let base = l.transform;
    app.tools.move_anchor = Some((target, base));
    app.tools.transform_anchor = Some(TransformAnchor {
        layer_id: target,
        kind,
        base,
        cursor_doc: doc,
    });
    // Les pré-calculs drag (fond sans ce calque + composite masqué) sont
    // DIFFÉRÉS au premier mouvement réel (TransformCursor) : un simple clic
    // qui ne sert qu'à SÉLECTIONNER ne déclenche ainsi AUCUNE composite —
    // sans masque actif nulle part, même pas de fallback. Le blend réel est
    // recalculé UNE seule fois au relâchement, seulement si le calque a bougé.
    Task::none()
}

fn handle_transform_cursor(app: &mut PhotoApp, doc: (f32, f32), uniform: bool) -> Task<Message> {
    let Some(anchor) = app.tools.transform_anchor else {
        return Task::none();
    };
    // Premier mouvement RÉEL du geste : on lance ONE seule fois les
    // pré-calculs drag (fond sans ce calque + composite masqué). Pas de
    // mouvement → un simple clic de sélection ne déclenche AUCUNE composite.
    let drag_task = if !app.rendering.drag_bg_job.is_running() && app.needs_fallback() {
        let has_mask = app
            .document
            .doc
            .find(anchor.layer_id)
            .map(|n| n.masks().iter().any(|m| m.enabled))
            .unwrap_or(false);
        let mut task = app.drag_background_task(anchor.layer_id);
        if has_mask && let Some(t2) = app.drag_layer_composite_task(anchor.layer_id) {
            task = Some(match task {
                Some(t1) => Task::batch([t1, t2]),
                None => t2,
            });
        }
        task.unwrap_or_else(Task::none)
    } else {
        Task::none()
    };
    let Some(LayerNode::Pixel(l)) = app.document.doc.find_mut(anchor.layer_id) else {
        app.tools.transform_anchor = None;
        return drag_task;
    };
    let (w0, h0) = l.dimensions();
    let (w0, h0) = (w0 as f32, h0 as f32);
    let base = anchor.base;
    let new_t = transform_for_cursor(&base, w0, h0, anchor.kind, anchor.cursor_doc, doc, uniform);
    l.transform = new_t;
    // Invalide la fallback stale (contient le calque à l'ancienne position).
    if app.rendering.fallback_handle.is_some() {
        app.rendering.fallback_handle = None;
        app.rendering.fallback_size = None;
    }
    drag_task
}

fn handle_transform_end(app: &mut PhotoApp) -> Task<Message> {
    app.tools.transform_anchor = None;
    app.rendering.drag_bg_job.finish();
    // Purge immédiate des buffers drag — la prochaine frame affiche le
    // fallback complet sans artefacts.
    app.rendering.drag_background = None;
    app.rendering.drag_background_size = None;
    app.rendering.drag_layer_composite = None;
    app.rendering.drag_layer_composite_size = None;
    // Fin de geste : UNE commande ancre→finale (snapshot au début, aucune
    // pendant le geste). Geste immobile = aucune entrée d'historique et
    // AUCUNE recomposite — un simple clic de sélection ne doit rien coûter.
    if let Some((id, anchor_t)) = app.tools.move_anchor.take()
        && let Some(LayerNode::Pixel(l)) = app.document.doc.find(id)
        && l.transform != anchor_t
    {
        let cmd = Command::SetTransform {
            layer_id: id,
            old: anchor_t,
            new: l.transform,
        };
        app.document.history.push_command_immediate(cmd);
        if app.needs_fallback() {
            app.invalidate_fallback();
        }
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

/// Lance l'échantillonnage pipette : composite la scène hors thread UI et
/// échantillonne la couleur au point document donné. Respecte RENDERING.md
/// invariant #1 (composite = spawn_blocking) + #5 (tâche async = background_tasks).
fn handle_pick_color(app: &mut PhotoApp, x: f32, y: f32) -> Task<Message> {
    let task_id = app.rendering.background_tasks.start("Pipette...");
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    doc_copy.warm_cache_from(&app.document.doc);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || doc_copy.sample_color(x, y))
                .await
                .unwrap_or(None)
        },
        move |result| Message::ColorPicked {
            task_id,
            color: result.map(|[r, g, b, a]| iced::Color::from_rgba8(r, g, b, a as f32 / 255.0)),
        },
    )
}

fn handle_color_picked(
    app: &mut PhotoApp,
    task_id: u64,
    color: Option<iced::Color>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    // Hors du plan composite (clic en dehors des calques) : la pipette ne
    // change PAS la couleur courante (sémantique Photoshop).
    if let Some(color) = color {
        app.tools.brush_color = color;
    }
    // Revient à l'outil précédent (comportement pipette standard).
    app.tools.selected_tool = app
        .tools
        .previous_tool
        .unwrap_or(crate::message::Tool::Brush);
    app.tools.previous_tool = None;
    Task::none()
}

fn handle_undo_redo(app: &mut PhotoApp, is_undo: bool) -> Task<Message> {
    // Hybrid history: the history applies the inverse itself (undo) or the
    // command (redo) to the document, then describes what to invalidate —
    // full recomposite or nothing (the UI texture cache sync already targets
    // the actually-modified layers).
    let action = if is_undo {
        app.document.history.undo(&mut app.document.doc)
    } else {
        app.document.history.redo(&mut app.document.doc)
    };
    match action {
        Some(UndoAction::FullRestore) => {
            // Restored structure: the selection may point to a vanished node,
            // we bound it (nœuds ET sous-calques de filtres).
            if let Some(sel) = app.document.selected_layer
                && app.document.doc.find(sel).is_none()
                && app.document.doc.find_filter_layer(sel).is_none()
            {
                app.document.selected_layer = app.document.doc.iter_pixels().last().map(|l| l.id);
            }
            app.tools.move_anchor = None;
            app.tools.transform_anchor = None;
            app.rendering.drag_background = None;
            app.rendering.drag_background_size = None;
            app.tools.pending_paint = None;
            app.tools.stroke_layer = None;
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
    task_id: u64,
    generation: u64,
    result: Result<Option<(Vec<u8>, u32, u32)>, String>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    use crate::state::Finish;
    // Si la génération ne correspond plus ou qu'une édition a eu lieu pendant
    // le vol, take_fallback_task doit relancer (l'état du job le sait déjà).
    if !matches!(
        app.rendering.fallback_job.finish(generation),
        Finish::Applied
    ) {
        return app.take_fallback_task().unwrap_or_else(Task::none);
    }
    match result {
        Ok(Some((rgba, w, h))) => {
            app.rendering.fallback_size = Some(Size::new(w as f32, h as f32));
            app.rendering.fallback_handle =
                Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        }
        Ok(None) => {
            app.rendering.fallback_handle = None;
            app.rendering.fallback_size = None;
        }
        Err(e) => app.canvas.image_error = Some(e),
    }
    Task::none()
}

fn handle_drag_background_computed(
    app: &mut PhotoApp,
    task_id: u64,
    layer_id: uuid::Uuid,
    result: Option<(Vec<u8>, u32, u32)>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    // Le calcul est terminé : on libère l'état job AVANT de vérifier si le
    // résultat est encore pertinent (le drag peut avoir changé de cible).
    let still_running_for = app.rendering.drag_bg_job.is_running_for(layer_id);
    app.rendering.drag_bg_job.finish();
    // Only applies if we are STILL dragging the same subtree
    if app.tools.move_anchor.map(|(id, _)| id) == Some(layer_id)
        && still_running_for
        && let Some((rgba, w, h)) = result
    {
        app.rendering.drag_background = Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        app.rendering.drag_background_size = Some(Size::new(w as f32, h as f32));
    }
    Task::none()
}

fn handle_drag_layer_composite_computed(
    app: &mut PhotoApp,
    task_id: u64,
    layer_id: uuid::Uuid,
    result: Option<(Vec<u8>, u32, u32)>,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.rendering.drag_layer_job.finish();
    // Valide seulement si on DRAG toujours CE calque — sinon le buffer est
    // orphelin et écrasé au prochain MoveLayerStart.
    if app.tools.move_anchor.map(|(id, _)| id) == Some(layer_id)
        && let Some((rgba, w, h)) = result
    {
        app.rendering.drag_layer_composite =
            Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
        app.rendering.drag_layer_composite_size = Some(Size::new(w as f32, h as f32));
    }
    Task::none()
}

fn handle_zoom_in(app: &mut PhotoApp) -> Task<Message> {
    app.canvas.zoom_level = (app.canvas.zoom_level + 10).clamp(5, 1600);
    Task::none()
}

fn handle_zoom_out(app: &mut PhotoApp) -> Task<Message> {
    app.canvas.zoom_level = app.canvas.zoom_level.saturating_sub(10).max(5);
    Task::none()
}

fn handle_detect_gpu(app: &mut PhotoApp) -> Task<Message> {
    let task_id = app.rendering.background_tasks.start("Détection du GPU...");
    Task::perform(
        async move { crate::components::gpu::detect_gpu_info().await },
        move |info| Message::GpuDetected { task_id, info },
    )
}

fn handle_gpu_detected(app: &mut PhotoApp, task_id: u64, info: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.rendering.gpu_info = Some(info);
    app.rendering.gpu_available = true;
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
        Message::PickColor { x, y } => Some(handle_pick_color(app, x, y)),
        Message::ColorPicked { task_id, color } => Some(handle_color_picked(app, task_id, color)),
        Message::FallbackComputed {
            task_id,
            generation,
            result,
        } => Some(handle_fallback_computed(app, task_id, generation, result)),
        Message::DragBackgroundComputed {
            task_id,
            layer_id,
            result,
        } => Some(handle_drag_background_computed(
            app, task_id, layer_id, result,
        )),
        Message::DragLayerCompositeComputed {
            task_id,
            layer_id,
            result,
        } => Some(handle_drag_layer_composite_computed(
            app, task_id, layer_id, result,
        )),
        Message::ZoomInPressed => Some(handle_zoom_in(app)),
        Message::ZoomOutPressed => Some(handle_zoom_out(app)),
        Message::MockAction => Some(Task::none()),
        Message::DetectGpu => Some(handle_detect_gpu(app)),
        Message::GpuDetected { task_id, info } => Some(handle_gpu_detected(app, task_id, info)),
        _ => None,
    }
}

/// Pré-dispatch sans clonage — voir `mod.rs`.
pub fn handles(msg: &Message) -> bool {
    matches!(
        msg,
        Message::Event { .. }
            | Message::ExecuteAction(_)
            | Message::HardwareDetected(_)
            | Message::TickFrame
            | Message::CanvasFit
            | Message::ImageCanvasEvent(_)
            | Message::Quit
            | Message::Undo
            | Message::Redo
            | Message::PickColor { .. }
            | Message::ColorPicked { .. }
            | Message::FallbackComputed { .. }
            | Message::DragBackgroundComputed { .. }
            | Message::DragLayerCompositeComputed { .. }
            | Message::ZoomInPressed
            | Message::ZoomOutPressed
            | Message::MockAction
            | Message::DetectGpu
            | Message::GpuDetected { .. }
    )
}
