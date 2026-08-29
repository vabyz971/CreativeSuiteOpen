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
        let is_mask =
            app.mask_paint_target == Some(id) && app.doc.find(id).and_then(|n| n.mask()).is_some();
        let (source, transform) = if is_mask {
            let m = app.doc.find(id).and_then(|n| n.mask()).unwrap();
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
            tex: tex.clone(),
        });
        app.history.push_snapshot(app.snapshot());
        let brush = photo_engine::paint::BrushParams {
            radius: app.brush_size / 2.0,
            color: if is_mask {
                [255, 255, 255]
            } else {
                [
                    (app.brush_color.r * 255.0) as u8,
                    (app.brush_color.g * 255.0) as u8,
                    (app.brush_color.b * 255.0) as u8,
                ]
            },
            opacity: app.brush_opacity,
            mode: if erase {
                photo_engine::paint::StrokeMode::Erase
            } else {
                photo_engine::paint::StrokeMode::Paint
            },
        };
        return Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    photo_engine::paint::commit_stroke(&source, &pts, &transform, &brush)
                })
                .await
            },
            move |result| match result {
                Ok(buf) => Message::PaintApplied { layer_id: id, buf },
                Err(_) => Message::PaintFailed { layer_id: id },
            },
        );
    }
    Task::none()
}

pub fn handle_paint_failed(app: &mut PhotoApp, layer_id: Uuid) -> Task<Message> {
    if app
        .pending_paint
        .as_ref()
        .is_some_and(|p| p.layer_id == layer_id)
    {
        app.pending_paint = None;
    }
    app.image_error = Some("Échec interne lors de l'application du trait".into());
    Task::none()
}

pub fn handle_paint_applied(
    app: &mut PhotoApp,
    layer_id: Uuid,
    buf: photo_engine::paint::StrokeCommit,
) -> Task<Message> {
    if let Some(img) = image::RgbaImage::from_raw(buf.width, buf.height, buf.rgba) {
        if app.mask_paint_target == Some(layer_id)
            && let Some(mask) = app
                .doc
                .find_mut(layer_id)
                .and_then(|n| n.mask_mut())
                .and_then(|m| m.as_mut())
        {
            mask.image = Arc::new(img);
            mask.touch();
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
    Task::none()
}
pub fn handle_toggle_tools(app: &mut PhotoApp) -> Task<Message> {
    app.tools_visible = !app.tools_visible;
    Task::none()
}
pub fn handle_set_mask_target(app: &mut PhotoApp, id: Option<Uuid>) -> Task<Message> {
    app.mask_paint_target = id;
    Task::none()
}
pub fn handle_add_mask(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    let has = app.doc.find(id).and_then(|n| n.mask()).is_some();
    if !has && app.doc.find(id).is_some() {
        let pre = app.snapshot();
        let (w, h) = match app.doc.find(id) {
            Some(photo_engine::LayerNode::Pixel(l)) => l.dimensions(),
            _ => (app.doc.width.max(1), app.doc.height.max(1)),
        };
        if let Some(slot) = app.doc.find_mut(id).and_then(|n| n.mask_mut()) {
            *slot = Some(photo_engine::LayerMask::full(w, h));
        }
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_remove_mask(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if app.doc.find(id).and_then(|n| n.mask()).is_some() {
        let pre = app.snapshot();
        if let Some(slot) = app.doc.find_mut(id).and_then(|n| n.mask_mut()) {
            *slot = None;
        }
        if app.mask_paint_target == Some(id) {
            app.mask_paint_target = None;
        }
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_toggle_mask_enabled(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(m) = app.doc.find(id).and_then(|n| n.mask()) {
        let cmd = photo_engine::Command::SetMaskEnabled {
            node_id: id,
            old: m.enabled,
            new: !m.enabled,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
pub fn handle_invert_mask(app: &mut PhotoApp, id: Uuid) -> Task<Message> {
    if let Some(m) = app.doc.find(id).and_then(|n| n.mask()) {
        let cmd = photo_engine::Command::SetMaskInverted {
            node_id: id,
            old: m.inverted,
            new: !m.inverted,
        };
        app.history.push_command_immediate(cmd.clone());
        let _ = app.doc.apply_command(cmd);
        app.invalidate_fallback();
    }
    Task::none()
}
