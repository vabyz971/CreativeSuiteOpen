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

//! Misc handlers (preferences, canvas, hardware) — extracted from update/mod.rs

use std::sync::Arc;

use iced::{Task, Vector};

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
    // Spacebar hold → outil Main temporaire (Photoshop/Affinity style).
    // On l'intercepte AVANT le resolver pour ne pas être écrasé par un binding
    // éventuel de Space (Space seul = pan ; toute combinaison modifieur laisse
    // passer le resolver). Le drag souris sur le canvas fait le pan via
    // `CanvasTool::Hand` côté image_canvas.
    let space_key = iced::keyboard::Key::Named(iced::keyboard::key::Named::Space);
    match &event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
            if *key == space_key =>
        {
            return super::dispatch(app, Message::SpaceHeldDown);
        }
        iced::Event::Keyboard(iced::keyboard::Event::KeyReleased { key, .. })
            if *key == space_key =>
        {
            return super::dispatch(app, Message::SpaceHeldUp);
        }
        _ => {}
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
        preferences::PhotoAction::ToolMove => Message::SelectTool(Tool::Select),
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
        ui_kit::image_canvas::ImageCanvasEvent::PickHover { x, y } => handle_pick_hover(app, x, y),
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
        ui_kit::image_canvas::ImageCanvasEvent::TransformCursor { doc, uniform, snap } => {
            handle_transform_cursor(app, doc, uniform, snap)
        }
        ui_kit::image_canvas::ImageCanvasEvent::TransformEnd => handle_transform_end(app),
        ui_kit::image_canvas::ImageCanvasEvent::ClearSelection => {
            app.document.selected_layer = None;
            app.canvas.canvas_selection = None;
            app.tools.expanded_fx_stack.clear();
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
    // Chemin GPU unique : aucun pré-calcul — un simple clic qui ne sert
    // qu'à SÉLECTIONNER ne déclenche AUCUNE tâche.
    Task::none()
}

fn handle_transform_cursor(
    app: &mut PhotoApp,
    doc: (f32, f32),
    uniform: bool,
    snap: bool,
) -> Task<Message> {
    let Some(anchor) = app.tools.transform_anchor else {
        return Task::none();
    };
    // Chemin GPU unique : le déplacement met à jour le transform EN DIRECT
    // (redessiné au draw, zéro recomposite) — aucun pré-calcul drag.
    let Some(LayerNode::Pixel(l)) = app.document.doc.find_mut(anchor.layer_id) else {
        app.tools.transform_anchor = None;
        return Task::none();
    };
    let (w0, h0) = l.dimensions();
    let (w0, h0) = (w0 as f32, h0 as f32);
    let base = anchor.base;
    let move_grid = if app.tools.move_grid_enabled {
        Some(app.tools.move_grid_size)
    } else {
        None
    };
    let new_t = transform_for_cursor(
        &base,
        w0,
        h0,
        anchor.kind,
        anchor.cursor_doc,
        doc,
        uniform,
        snap,
        app.tools.rotation_step,
        move_grid,
    );
    l.transform = new_t;
    Task::none()
}

fn handle_transform_end(app: &mut PhotoApp) -> Task<Message> {
    app.tools.transform_anchor = None;
    // Fin de geste : UNE commande ancre→finale (snapshot au début, aucune
    // pendant le geste). Geste immobile = aucune entrée d'historique —
    // un simple clic de sélection ne doit rien coûter.
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
    }
    Task::none()
}

/// Calcule la transformation d'une position curseur document pour un geste.
#[allow(clippy::too_many_arguments)]
fn transform_for_cursor(
    base: &crate::layers::Transform2D,
    w0: f32,
    h0: f32,
    kind: TransformHandle,
    start: (f32, f32),
    cur: (f32, f32),
    uniform: bool,
    snap: bool,
    rot_step: f32,
    move_grid: Option<f32>,
) -> crate::layers::Transform2D {
    let cx = w0 / 2.0;
    let cy = h0 / 2.0;
    match kind {
        // Déplacement : l'offset suit le delta document 1:1 (déjà en px image).
        // Grille active (outil Sélection) → delta arrondi au pas de grille.
        TransformHandle::Move => {
            let mut t = *base;
            let dx = cur.0 - start.0;
            let dy = cur.1 - start.1;
            if let Some(step) = move_grid {
                t.offset_x = base.offset_x + (dx / step).round() * step;
                t.offset_y = base.offset_y + (dy / step).round() * step;
            } else {
                t.offset_x = base.offset_x + dx;
                t.offset_y = base.offset_y + dy;
            }
            t
        }
        // Rotation : autour du centre du rectangle scalé, angle Δ depuis le début.
        TransformHandle::Rotate => {
            let center_doc = base.local_to_doc(w0, h0, cx, cy);
            let a0 = (start.1 - center_doc.1).atan2(start.0 - center_doc.0);
            let a1 = (cur.1 - center_doc.1).atan2(cur.0 - center_doc.0);
            let mut t = *base;
            let mut deg = base.rotation_deg + (a1 - a0).to_degrees();
            // Aimantation : Ctrl → multiples du cran (défaut 5°), Shift → multiples de 90°.
            let step = if uniform {
                Some(rot_step)
            } else if snap {
                Some(90.0)
            } else {
                None
            };
            if let Some(step) = step {
                deg = (deg / step).round() * step;
            }
            t.rotation_deg = deg;
            t
        }
        // Redimensionnement : le coin opposé reste PIVOTÉ (fixe), le coin
        // saisi suit le curseur. L'angle de rotation et les skew sont
        // conservés tels quels (les axes locaux du boîtier ne changent pas).
        TransformHandle::Corner(corner) => resize_corner(base, w0, h0, corner, cur, uniform),
        // Poignée d'ÉCHELLE : 0.2× au-delà du coin bas-droite. On exploite le
        // redimensionnement uniforme du coin BR, mais avec un curseur EFFICACE
        // décalé du vecteur handle→coin à la saisie — ainsi le coin réel suit
        // le curseur 1:1 sans saut initial de +20%.
        TransformHandle::Scale => {
            let br_doc = base.local_to_doc(w0, h0, w0, h0);
            let cur_eff = (cur.0 - (start.0 - br_doc.0), cur.1 - (start.1 - br_doc.1));
            resize_corner(base, w0, h0, Corner::BottomRight, cur_eff, true)
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

/// Redimensionnement d'un coin PIVOTÉ (le coin opposé reste fixe). `cur` en
/// coordonnées document, `uniform` = aspect conservé (Ctrl ou poignée Scale).
fn resize_corner(
    base: &crate::layers::Transform2D,
    w0: f32,
    h0: f32,
    corner: Corner,
    cur: (f32, f32),
    uniform: bool,
) -> crate::layers::Transform2D {
    let cx = w0 / 2.0;
    let cy = h0 / 2.0;
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
    // Ctrl (ou poignée Scale) → échelle PROPORTIONNELLE : un seul facteur
    // dérivé de l'axe dominant et appliqué aux 2 échelles aspect conservé.
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
        // « raw » est une échelle ABSOLUE (b = K·S'·(dc−oc)), pas un facteur :
        // q = f/dom cale l'axe dominant sur le curseur et préserve l'aspect.
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
    // Clic validé : la loupe disparaît avec la pipette.
    app.tools.pick_loupe = None;
    // Revient à l'outil précédent (comportement pipette standard).
    app.tools.selected_tool = app
        .tools
        .previous_tool
        .unwrap_or(crate::message::Tool::Brush);
    app.tools.previous_tool = None;
    Task::none()
}

/// Survol pipette : échantillonne un patch carré autour du curseur (LOUPE).
/// Garde anti-empilement : si un patch tourne déjà, on ne relance pas — le
/// suivant sera pris au prochain mouvement (le quanta de 4 px doc du canvas
/// rend l'écart de position négligeable). Même pattern spawn_blocking que
/// [`handle_pick_color`]. Pas d'entrée dans `background_tasks` (rafale
/// trop rapide) — seul un drapeau dans `tools` synchronise.
fn handle_pick_hover(app: &mut PhotoApp, x: f32, y: f32) -> Task<Message> {
    if app.tools.loupe_sample_pending || app.tools.selected_tool != Tool::Eyedropper {
        return Task::none();
    }
    app.tools.loupe_sample_pending = true;
    let resp = app.tools.loupe_last_resp.unwrap_or(0) + 1;
    app.tools.loupe_last_resp = Some(resp);
    let side = ui_kit::image_canvas::LOUPE_PATCH_SIDE;
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    doc_copy.warm_cache_from(&app.document.doc);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || doc_copy.sample_region(x, y, side))
                .await
                .unwrap_or(None)
        },
        move |result| Message::PickSampleReady { resp, result },
    )
}

/// Réception d'un patch de loupe : pixels conservés (zéro copie via l'Arc)
/// pour le chemin GPU unique, qui les grossit au curseur.
fn handle_pick_sample_ready(
    app: &mut PhotoApp,
    resp: u64,
    result: Option<Vec<u8>>,
) -> Task<Message> {
    app.tools.loupe_sample_pending = false;
    // Stale : un patch plus récent a été demandé — on ignore cet arrivage.
    if app.tools.loupe_last_resp != Some(resp) {
        return Task::none();
    }
    let side = ui_kit::image_canvas::LOUPE_PATCH_SIDE;
    app.tools.pick_loupe = result.map(|rgba| (Arc::<[u8]>::from(rgba), side));
    Task::none()
}

fn handle_undo_redo(app: &mut PhotoApp, is_undo: bool) -> Task<Message> {
    // Invalide les réglages en vol : calculés sur un état révolu, leur
    // application réécrirait par-dessus l'état annulé. Le pouce retombe
    // sur la valeur vivante.
    app.rendering.param_epoch = app.rendering.param_epoch.wrapping_add(1);
    app.rendering.pending_param = None;
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
            app.tools.pending_paint = None;
            app.tools.stroke_layer = None;
        }
        // Chemin GPU unique : le draw relit le document à chaque frame,
        // aucune invalidation explicite n'est nécessaire.
        Some(UndoAction::Applied(_)) | None => {}
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

/// Spacebar enfoncée → outil Main temporaire. Mémorise l'outil précédent
/// dans `previous_tool` (réutilise le slot déjà présent pour l'eyedropper
/// afin de garder la pile minimale).
fn handle_space_hold_down(app: &mut PhotoApp) -> Task<Message> {
    // Ne pas écraser si l'utilisateur a déjà un outil temporaire mémorisé
    // (ex. eyedropper en cours) : on ne capture que Main direct.
    if app.tools.selected_tool != Tool::Hand {
        // previous_tool sert déjà pour l'eyedropper — on n'écrase que si
        // aucun outil temporaire n'est en attente (logique identique aux
        // autres gestions temporaires).
        if app.tools.previous_tool.is_none() {
            app.tools.previous_tool = Some(app.tools.selected_tool);
        }
        app.tools.selected_tool = Tool::Hand;
    }
    Task::none()
}

/// Spacebar relâchée → restaure l'outil précédent si on est toujours sur
/// Main « temporaire ». Si l'utilisateur a explicitement choisi Main entre
/// temps, on ne touche pas à previous_tool pour ne pas effacer son choix.
fn handle_space_hold_up(app: &mut PhotoApp) -> Task<Message> {
    if app.tools.selected_tool == Tool::Hand
        && let Some(prev) = app.tools.previous_tool
    {
        app.tools.selected_tool = prev;
        app.tools.previous_tool = None;
    }
    Task::none()
}

fn handle_detect_gpu(app: &mut PhotoApp) -> Task<Message> {
    let task_id = app.rendering.background_tasks.start("Détection du GPU...");
    // `detect_gpu_info_sync` fait init wgpu + compilation shaders via
    // `pollster::block_on` : jamais sur l'executor Tokio, toujours en
    // `spawn_blocking` (pool bloquant dédié).
    Task::perform(
        async move {
            tokio::task::spawn_blocking(crate::components::gpu::detect_gpu_info_sync)
                .await
                .unwrap_or_else(|e| format!("Tâche annulée : {e}"))
        },
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
        Message::PickSampleReady { resp, result } => {
            Some(handle_pick_sample_ready(app, resp, result))
        }
        Message::ZoomInPressed => Some(handle_zoom_in(app)),
        Message::ZoomOutPressed => Some(handle_zoom_out(app)),
        Message::MockAction => Some(Task::none()),
        Message::DetectGpu => Some(handle_detect_gpu(app)),
        Message::GpuDetected { task_id, info } => Some(handle_gpu_detected(app, task_id, info)),
        Message::SpaceHeldDown => Some(handle_space_hold_down(app)),
        Message::SpaceHeldUp => Some(handle_space_hold_up(app)),
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
            | Message::PickSampleReady { .. }
            | Message::ZoomInPressed
            | Message::ZoomOutPressed
            | Message::MockAction
            | Message::DetectGpu
            | Message::GpuDetected { .. }
            | Message::SpaceHeldDown
            | Message::SpaceHeldUp
    )
}
