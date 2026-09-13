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

//! Project / image / document handlers — extracted from update/mod.rs

use std::sync::Arc;

use iced::{Task, Vector};

use crate::layers::{LayerNode, PixelLayer};
use crate::message::{DecodedLayer, Message};
use crate::state::PhotoApp;

fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("projet")
        .to_string()
}

fn node_dimensions(node: &LayerNode) -> (u32, u32) {
    match node {
        LayerNode::Pixel(l) => l.dimensions(),
        LayerNode::Group(_) => (0, 0),
        LayerNode::Adjustment(_) => (0, 0),
    }
}

/// Open dialog: projects (.cygp, and legacy .csophoto/.csphoto) AND images.
fn open_document_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Projet Cygnus",
                    &[
                        photo_engine::project::PROJECT_EXTENSION,
                        photo_engine::project::LEGACY_PROJECT_EXTENSIONS[0],
                        photo_engine::project::LEGACY_PROJECT_EXTENSIONS[1],
                    ],
                )
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "bmp", "tiff", "webp", "gif"],
                )
                .set_title("Ouvrir un projet ou une image")
                .pick_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        Message::ProjectOpenPicked,
    )
}

/// "Save As" dialog (.cygp).
fn save_as_dialog_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("Projet Cygnus", &[photo_engine::project::PROJECT_EXTENSION])
                .set_title("Enregistrer le projet")
                .set_file_name("sans-titre.cygp")
                .save_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        Message::SaveProjectPathPicked,
    )
}

/// Generic file read off the UI thread — pousse son libellé dans
/// `background_tasks` avant de partir (retiré par le handler de sabliers).
fn read_file_task(
    app: &mut PhotoApp,
    path: std::path::PathBuf,
    label: impl Into<String>,
) -> Task<Message> {
    let task_id = app.rendering.background_tasks.start(label);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let bytes = std::fs::read(&path).map_err(|e| format!("Lecture échouée: {e}"))?;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("image")
                    .to_string();
                Ok::<(Vec<u8>, String), String>((bytes, name))
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))?
        },
        move |result| Message::ImageRead { task_id, result },
    )
}

fn load_project_task(app: &mut PhotoApp, path: std::path::PathBuf) -> Task<Message> {
    let task_id = app
        .rendering
        .background_tasks
        .start(format!("Ouverture de {}", file_label(&path)));
    Task::perform(
        async move {
            let res = tokio::task::spawn_blocking(move || photo_engine::project::load(&path))
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(res)
        },
        move |result| Message::ProjectOpened { task_id, result },
    )
}

fn save_project_task(app: &mut PhotoApp, path: std::path::PathBuf) -> Task<Message> {
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    let name = file_label(&path);
    let task_id = app
        .rendering
        .background_tasks
        .start(format!("Enregistrement de {name}"));
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || photo_engine::project::save(&path, &doc_copy))
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(name)
        },
        move |result| Message::ProjectSaved { task_id, result },
    )
}

/// "Export image" dialog — PNG by default, JPEG if the extension is .jpg/.jpeg.
fn export_dialog_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("Image PNG", &["png"])
                .add_filter("Image JPEG", &["jpg", "jpeg"])
                .set_title("Exporter l'image")
                .set_file_name("sans-titre.png")
                .save_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        Message::ExportPathPicked,
    )
}

/// Export off the UI thread: full composite (infinite plane cropped to the
/// document) then encoding according to the chosen extension.
fn export_image_task(app: &mut PhotoApp, path: std::path::PathBuf) -> Task<Message> {
    let mut doc_copy = photo_engine::Document::new(app.document.doc.width, app.document.doc.height);
    doc_copy.restore_snapshot(app.document.doc.snapshot());
    doc_copy.warm_cache_from(&app.document.doc);
    let name = file_label(&path);
    let task_id = app
        .rendering
        .background_tasks
        .start(format!("Export de {name}"));
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let img = doc_copy
                    .composite()
                    .ok_or("Rien à exporter : le document est vide")?;
                photo_engine::export_image(
                    &img,
                    &path,
                    photo_engine::ExportFormat::from_path(&path),
                )
            })
            .await
            .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(name)
        },
        move |result| Message::ImageExported { task_id, result },
    )
}

fn pick_image_task(map: fn(Option<std::path::PathBuf>) -> Message) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "bmp", "tiff", "webp", "gif"],
                )
                .set_title("Ouvrir une image")
                .pick_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        map,
    )
}

fn handle_new_project(app: &mut PhotoApp) -> Task<Message> {
    app.document.doc = photo_engine::Document::new(0, 0);
    app.document.selected_layer = None;
    app.canvas.canvas_pan = Vector::new(0.0, 0.0);
    app.canvas.zoom_level = 100;
    app.rendering.fallback_size = None;
    app.rendering.fallback_handle = None;
    app.canvas.image_path = None;
    app.canvas.image_error = None;
    app.tools.move_anchor = None;
    app.tools.transform_anchor = None;
    app.tools.new_doc_w = "1920".to_string();
    app.tools.new_doc_h = "1080".to_string();
    app.tools.welcome_error = None;
    app.document.project_path = None;
    app.document.history.reset();
    Task::none()
}

fn handle_open_project(app: &mut PhotoApp) -> Task<Message> {
    if app.rendering.background_tasks.is_empty() {
        open_document_task()
    } else {
        Task::none()
    }
}

fn handle_project_open_picked(
    app: &mut PhotoApp,
    path_opt: Option<std::path::PathBuf>,
) -> Task<Message> {
    if let Some(path) = path_opt
        && app.rendering.background_tasks.is_empty()
    {
        if photo_engine::project::is_project_path(&path) {
            return load_project_task(app, path);
        }
        // Plain image -> existing layer flow
        let label = format!("Ouverture de {}", file_label(&path));
        return read_file_task(app, path, label);
    }
    Task::none()
}

fn handle_project_opened_ok(
    app: &mut PhotoApp,
    task_id: u64,
    loaded: photo_engine::project::LoadedProject,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = None;
    app.document.selected_layer = loaded.document.iter_pixels().last().map(|l| l.id);
    app.document.doc = loaded.document;
    app.canvas.image_path = loaded.source_name.clone();
    app.document.project_path = loaded.path.clone();
    app.canvas.canvas_pan = Vector::new(0.0, 0.0);
    app.canvas.zoom_level = 100;
    app.canvas.canvas_selection = None;
    app.tools.welcome_error = None;
    app.document.history.reset();
    app.invalidate_fallback();
    Task::none()
}

fn handle_project_opened_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(e);
    Task::none()
}

fn handle_save_project(app: &mut PhotoApp) -> Task<Message> {
    if app.rendering.background_tasks.is_empty() && app.doc_dims().is_some() {
        match app.document.project_path.clone() {
            Some(path) => return save_project_task(app, path),
            None => return save_as_dialog_task(),
        }
    }
    Task::none()
}

fn handle_save_project_as(app: &mut PhotoApp) -> Task<Message> {
    if app.rendering.background_tasks.is_empty() {
        save_as_dialog_task()
    } else {
        Task::none()
    }
}

fn handle_save_project_path_picked(
    app: &mut PhotoApp,
    path_opt: Option<std::path::PathBuf>,
) -> Task<Message> {
    if let Some(mut path) = path_opt
        && app.doc_dims().is_some()
    {
        if path.extension().and_then(|e| e.to_str())
            != Some(photo_engine::project::PROJECT_EXTENSION)
        {
            path.set_extension(photo_engine::project::PROJECT_EXTENSION);
        }
        app.document.project_path = Some(path.clone());
        return save_project_task(app, path);
    }
    Task::none()
}

fn handle_project_saved_ok(app: &mut PhotoApp, task_id: u64, name: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = None;
    // The project name feeds the canvas title if it is empty
    app.canvas.image_path.get_or_insert(name);
    Task::none()
}

fn handle_project_saved_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(e);
    Task::none()
}

fn handle_export_image(app: &mut PhotoApp) -> Task<Message> {
    if app.rendering.background_tasks.is_empty() && app.doc_dims().is_some() {
        export_dialog_task()
    } else {
        Task::none()
    }
}

fn handle_export_path_picked(
    app: &mut PhotoApp,
    path_opt: Option<std::path::PathBuf>,
) -> Task<Message> {
    if let Some(path) = path_opt
        && app.rendering.background_tasks.is_empty()
        && app.doc_dims().is_some()
    {
        return export_image_task(app, path);
    }
    Task::none()
}

fn handle_image_exported_ok(app: &mut PhotoApp, task_id: u64, _name: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = None;
    // Discreet confirmation via the error zone (green in the future UI)
    Task::none()
}

fn handle_image_exported_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(e);
    Task::none()
}

fn handle_open_image(app: &mut PhotoApp) -> Task<Message> {
    if app.rendering.background_tasks.is_empty() {
        pick_image_task(Message::ImagePicked)
    } else {
        Task::none()
    }
}

fn handle_image_picked(app: &mut PhotoApp, path_opt: Option<std::path::PathBuf>) -> Task<Message> {
    if let Some(path) = path_opt
        && app.rendering.background_tasks.is_empty()
    {
        let label = format!("Lecture de {}", file_label(&path));
        return read_file_task(app, path, label);
    }
    Task::none()
}

fn handle_image_read_ok(
    app: &mut PhotoApp,
    task_id: u64,
    bytes: Vec<u8>,
    name: String,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_path = Some(name.clone());
    app.canvas.image_error = None;
    let decode_task_id = app
        .rendering
        .background_tasks
        .start(format!("Décodage de {name}"));
    // La lecture du tas + le décodage + la construction du buffer tournent
    // hors thread UI (spawn_blocking) pendant que le spinner anime.
    Task::perform(
        async move {
            let decoded =
                tokio::task::spawn_blocking(move || match ::image::load_from_memory(&bytes) {
                    Ok(dyn_img) => Ok(DecodedLayer(PixelLayer::new(name, Arc::new(dyn_img)))),
                    Err(e) => Err(format!("Décodage échoué: {e}")),
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(decoded)
        },
        move |res| Message::ImageDecoded {
            task_id: decode_task_id,
            result: res,
        },
    )
}

fn handle_image_read_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(e);
    Task::none()
}

fn handle_image_decoded_ok(
    app: &mut PhotoApp,
    task_id: u64,
    decoded: DecodedLayer,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    let node = LayerNode::Pixel(decoded.0);
    // The document takes the dimensions of the first image
    if app.document.doc.width == 0 || app.document.doc.height == 0 {
        let (w, h) = node_dimensions(&node);
        app.document.doc.width = w;
        app.document.doc.height = h;
        app.canvas.canvas_pan = Vector::new(0.0, 0.0);
        app.canvas.canvas_selection = None;
        app.canvas.zoom_level = 100;
    }
    app.document.history.push_snapshot(app.snapshot());
    let new_id = node.id();
    app.document.doc.push_layer(node);
    app.document.selected_layer = Some(new_id);
    app.invalidate_fallback();
    Task::none()
}

fn handle_image_decoded_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.canvas.image_error = Some(e);
    Task::none()
}

fn handle_new_doc_width(app: &mut PhotoApp, v: String) -> Task<Message> {
    app.tools.new_doc_w = v;
    app.tools.welcome_error = None;
    Task::none()
}

fn handle_new_doc_height(app: &mut PhotoApp, v: String) -> Task<Message> {
    app.tools.new_doc_h = v;
    app.tools.welcome_error = None;
    Task::none()
}

fn handle_set_doc_preset(app: &mut PhotoApp, w: u32, h: u32) -> Task<Message> {
    app.tools.new_doc_w = w.to_string();
    app.tools.new_doc_h = h.to_string();
    app.tools.welcome_error = None;
    Task::none()
}

fn handle_create_document(app: &mut PhotoApp) -> Task<Message> {
    // Garde anti-empilement : l'allocation peut atteindre 400 Mo
    // (10000x10000) — jamais deux créations en vol, jamais en sync.
    if !app.rendering.background_tasks.is_empty() {
        return Task::none();
    }
    let parsed = (
        app.tools.new_doc_w.trim().parse::<u32>(),
        app.tools.new_doc_h.trim().parse::<u32>(),
    );
    match parsed {
        (Ok(w), Ok(h)) if (1..=10000).contains(&w) && (1..=10000).contains(&h) => {
            app.tools.welcome_error = None;
            let task_id = app
                .rendering
                .background_tasks
                .start(format!("Création du document {w}x{h}..."));
            // Allocation HORS thread UI : `ImageBuffer::from_pixel` sur de
            // grandes dimensions gelait l'interface en sync (state-only).
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let white =
                            ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
                                w,
                                h,
                                ::image::Rgba([255, 255, 255, 255]),
                            ));
                        DecodedLayer(PixelLayer::new("Arrière-plan", Arc::new(white)))
                    })
                    .await
                    .map_err(|e| format!("Tâche annulée : {e}"))
                },
                move |result| Message::DocumentCreated {
                    task_id,
                    w,
                    h,
                    result,
                },
            )
        }
        _ => {
            app.tools.welcome_error = Some("Dimensions invalides (1 à 10000 px)".into());
            Task::none()
        }
    }
}

fn handle_document_created_ok(
    app: &mut PhotoApp,
    task_id: u64,
    w: u32,
    h: u32,
    decoded: DecodedLayer,
) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    let node = LayerNode::Pixel(decoded.0);
    let id = node.id();
    app.document.doc.restore(w, h, vec![node]);
    app.document.selected_layer = Some(id);
    app.canvas.image_path = None;
    app.canvas.image_error = None;
    app.canvas.canvas_pan = Vector::new(0.0, 0.0);
    app.canvas.zoom_level = 100;
    app.tools.welcome_error = None;
    app.document.project_path = None;
    app.document.history.reset();
    app.tools.move_anchor = None;
    app.tools.transform_anchor = None;
    app.rendering.fallback_size = None;
    app.rendering.fallback_handle = None;
    app.invalidate_fallback();
    Task::none()
}

fn handle_document_created_err(app: &mut PhotoApp, task_id: u64, e: String) -> Task<Message> {
    app.rendering.background_tasks.finish(task_id);
    app.tools.welcome_error = Some(e);
    Task::none()
}

fn handle_show_resize_dialog(app: &mut PhotoApp) -> Task<Message> {
    app.tools.resize_dialog_open = !app.tools.resize_dialog_open;
    if app.tools.resize_dialog_open {
        let (w, h) = app.doc_dims().unwrap_or((800, 600));
        app.tools.resize_w = w.to_string();
        app.tools.resize_h = h.to_string();
    }
    Task::none()
}

fn handle_set_resize_width(app: &mut PhotoApp, s: String) -> Task<Message> {
    app.tools.resize_w = s;
    Task::none()
}

fn handle_set_resize_height(app: &mut PhotoApp, s: String) -> Task<Message> {
    app.tools.resize_h = s;
    Task::none()
}

fn handle_resize_document(app: &mut PhotoApp, width: u32, height: u32) -> Task<Message> {
    let w = width.max(1);
    let h = height.max(1);
    let pre = app.snapshot();
    app.document.doc.width = w;
    app.document.doc.height = h;
    app.tools.resize_dialog_open = false;
    app.document.history.push_snapshot(pre);
    app.invalidate_fallback();
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::NewProject => Some(handle_new_project(app)),
        Message::OpenProject => Some(handle_open_project(app)),
        Message::ProjectOpenPicked(p) => Some(handle_project_open_picked(app, p)),
        Message::ProjectOpened { task_id, result } => match result {
            Ok(loaded) => Some(handle_project_opened_ok(app, task_id, loaded)),
            Err(e) => Some(handle_project_opened_err(app, task_id, e)),
        },
        Message::SaveProject => Some(handle_save_project(app)),
        Message::SaveProjectAs => Some(handle_save_project_as(app)),
        Message::SaveProjectPathPicked(p) => Some(handle_save_project_path_picked(app, p)),
        Message::ProjectSaved { task_id, result } => match result {
            Ok(name) => Some(handle_project_saved_ok(app, task_id, name)),
            Err(e) => Some(handle_project_saved_err(app, task_id, e)),
        },
        Message::ExportImage => Some(handle_export_image(app)),
        Message::ExportPathPicked(p) => Some(handle_export_path_picked(app, p)),
        Message::ImageExported { task_id, result } => match result {
            Ok(name) => Some(handle_image_exported_ok(app, task_id, name)),
            Err(e) => Some(handle_image_exported_err(app, task_id, e)),
        },
        Message::OpenImage => Some(handle_open_image(app)),
        Message::ImagePicked(p) => Some(handle_image_picked(app, p)),
        Message::ImageRead { task_id, result } => match result {
            Ok((bytes, name)) => Some(handle_image_read_ok(app, task_id, bytes, name)),
            Err(e) => Some(handle_image_read_err(app, task_id, e)),
        },
        Message::ImageDecoded { task_id, result } => match result {
            Ok(decoded) => Some(handle_image_decoded_ok(app, task_id, decoded)),
            Err(e) => Some(handle_image_decoded_err(app, task_id, e)),
        },
        Message::NewDocWidth(v) => Some(handle_new_doc_width(app, v)),
        Message::NewDocHeight(v) => Some(handle_new_doc_height(app, v)),
        Message::SetDocPreset { w, h } => Some(handle_set_doc_preset(app, w, h)),
        Message::CreateDocument => Some(handle_create_document(app)),
        Message::DocumentCreated {
            task_id,
            w,
            h,
            result,
        } => match result {
            Ok(decoded) => Some(handle_document_created_ok(app, task_id, w, h, decoded)),
            Err(e) => Some(handle_document_created_err(app, task_id, e)),
        },
        Message::ShowResizeDialog => Some(handle_show_resize_dialog(app)),
        Message::SetResizeWidth(s) => Some(handle_set_resize_width(app, s)),
        Message::SetResizeHeight(s) => Some(handle_set_resize_height(app, s)),
        Message::ResizeDocument { width, height } => {
            Some(handle_resize_document(app, width, height))
        }
        _ => None,
    }
}

/// Pré-dispatch sans clonage — voir `mod.rs`.
pub fn handles(msg: &Message) -> bool {
    matches!(
        msg,
        Message::NewProject
            | Message::OpenProject
            | Message::ProjectOpenPicked(_)
            | Message::ProjectOpened { .. }
            | Message::SaveProject
            | Message::SaveProjectAs
            | Message::SaveProjectPathPicked(_)
            | Message::ProjectSaved { .. }
            | Message::ExportImage
            | Message::ExportPathPicked(_)
            | Message::ImageExported { .. }
            | Message::OpenImage
            | Message::ImagePicked(_)
            | Message::ImageRead { .. }
            | Message::ImageDecoded { .. }
            | Message::NewDocWidth(_)
            | Message::NewDocHeight(_)
            | Message::SetDocPreset { .. }
            | Message::CreateDocument
            | Message::DocumentCreated { .. }
            | Message::ShowResizeDialog
            | Message::SetResizeWidth(_)
            | Message::SetResizeHeight(_)
            | Message::ResizeDocument { .. }
    )
}
