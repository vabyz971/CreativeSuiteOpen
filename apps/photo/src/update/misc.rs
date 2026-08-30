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
use crate::state::PhotoApp;
use photo_engine::{Command, UndoAction};

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
        ui_kit::image_canvas::ImageCanvasEvent::MoveLayerStart => {
            if app.selected_tool == Tool::Move {
                // Hidden -> move forbidden
                if app
                    .selected_layer
                    .and_then(|id| app.doc.find(id))
                    .map(|n| !n.visible())
                    .unwrap_or(true)
                {
                    return Task::none();
                }
                // Reads the anchor before any mutation
                // (own-borrow-over-clone rule). FULL transform captured:
                // the anchor->final SetTransform command is only pushed at
                // release — zero snapshot during the gesture.
                let anchor = app
                    .selected_layer
                    .and_then(|id| app.doc.pixel_layer(id))
                    .map(|l| (l.id, l.transform));
                if let Some((id, anchor_t)) = anchor {
                    app.move_anchor = Some((id, anchor_t));
                    // Fallback (inter-layer blending): asks for the
                    // background WITHOUT this subtree in a background task.
                    // During the few ms of computation the drag already
                    // displays layer-by-layer (approximation), then the exact
                    // background arrives without ever freezing the UI.
                    if app.needs_fallback()
                        && let Some(task) = app.drag_background_task(id)
                    {
                        return task;
                    }
                }
            }
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::MoveLayer { dx, dy } => {
            if app.selected_tool == Tool::Move
                && let Some((id, anchor_t)) = app.move_anchor
                && Some(id) == app.selected_layer
            {
                let zoom = app.zoom_level as f32 / 100.0;
                if zoom > 0.001 {
                    // ZERO recomposite in both paths:
                    // - fast: the canvas redraws the texture at its new
                    //   position (Affinity model)
                    // - fallback: pre-computed background + layer drawn on
                    //   top (Normal approximation during the gesture)
                    let new_x = anchor_t.offset_x + dx / zoom;
                    let new_y = anchor_t.offset_y + dy / zoom;
                    if let Some(LayerNode::Pixel(l)) = app.doc.find_mut(id) {
                        l.transform.offset_x = new_x;
                        l.transform.offset_y = new_y;
                    }
                }
            }
            Task::none()
        }
        ui_kit::image_canvas::ImageCanvasEvent::MoveLayerEnd => {
            app.drag_bg_in_flight = None;
            // End of gesture: ONE light anchor->final command replaces the
            // old start-of-drag snapshot. Immobile drag = no history entry at all.
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
            // True recomposite: the layer's real blend at its final position
            // replaces the drag approximation
            let was_fallback = app.needs_fallback();
            app.drag_background = None;
            app.drag_background_size = None;
            if was_fallback {
                app.invalidate_fallback();
            }
            Task::none()
        }
    }
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
        Message::ZoomInPressed => Some(handle_zoom_in(app)),
        Message::ZoomOutPressed => Some(handle_zoom_out(app)),
        Message::MockAction => Some(Task::none()),
        Message::DetectGpu => Some(handle_detect_gpu(app)),
        Message::GpuDetected(info) => Some(handle_gpu_detected(app, info)),
        _ => None,
    }
}
