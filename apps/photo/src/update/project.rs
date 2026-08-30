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

/// Open dialog: projects (.csophoto, and legacy .csphoto) AND images.
fn open_document_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Projet CreativeSuite",
                    &[photo_engine::project::PROJECT_EXTENSION, "csphoto"],
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

/// "Save As" dialog (.csophoto).
fn save_as_dialog_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Projet CreativeSuite",
                    &[photo_engine::project::PROJECT_EXTENSION],
                )
                .set_title("Enregistrer le projet")
                .set_file_name("sans-titre.csophoto")
                .save_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        Message::SaveProjectPathPicked,
    )
}

/// Result of a raw file read (bytes + name).
type FileRead = Result<(Vec<u8>, String), String>;

/// Generic file read off the UI thread.
fn read_file_task(path: std::path::PathBuf, map: fn(FileRead) -> Message) -> Task<Message> {
    Task::perform(
        async move {
            let bytes = std::fs::read(&path).map_err(|e| format!("Lecture échouée: {e}"))?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image")
                .to_string();
            Ok::<(Vec<u8>, String), String>((bytes, name))
        },
        map,
    )
}

fn load_project_task(path: std::path::PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let res = tokio::task::spawn_blocking(move || photo_engine::project::load(&path))
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(res)
        },
        Message::ProjectOpened,
    )
}

fn save_project_task(app: &mut PhotoApp, path: std::path::PathBuf) -> Task<Message> {
    let mut doc_copy = photo_engine::Document::new(app.doc.width, app.doc.height);
    doc_copy.restore_snapshot(app.doc.snapshot());
    let name = file_label(&path);
    app.background_tasks
        .retain(|t| !t.starts_with("Enregistrement"));
    app.background_tasks
        .push(format!("Enregistrement de {name}"));
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || photo_engine::project::save(&path, &doc_copy))
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(name)
        },
        Message::ProjectSaved,
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
    let mut doc_copy = photo_engine::Document::new(app.doc.width, app.doc.height);
    doc_copy.restore_snapshot(app.doc.snapshot());
    let name = file_label(&path);
    app.background_tasks.retain(|t| !t.starts_with("Export"));
    app.background_tasks.push(format!("Export de {name}"));
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
        Message::ImageExported,
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
    app.doc = photo_engine::Document::new(0, 0);
    app.selected_layer = None;
    app.canvas_pan = Vector::new(0.0, 0.0);
    app.zoom_level = 100;
    app.fallback_size = None;
    app.fallback_handle = None;
    app.image_path = None;
    app.image_error = None;
    app.move_anchor = None;
    app.new_doc_w = "1920".to_string();
    app.new_doc_h = "1080".to_string();
    app.welcome_error = None;
    app.project_path = None;
    app.history.reset();
    Task::none()
}

fn handle_open_project(app: &mut PhotoApp) -> Task<Message> {
    if app.background_tasks.is_empty() {
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
        && app.background_tasks.is_empty()
    {
        if photo_engine::project::is_project_path(&path) {
            let name = file_label(&path);
            app.background_tasks.push(format!("Ouverture de {name}"));
            return load_project_task(path);
        }
        // Plain image -> existing layer flow
        return read_file_task(path, Message::ImageRead);
    }
    Task::none()
}

fn handle_project_opened_ok(
    app: &mut PhotoApp,
    loaded: photo_engine::project::LoadedProject,
) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = None;
    app.selected_layer = loaded.document.iter_pixels().last().map(|l| l.id);
    app.doc = loaded.document;
    app.image_path = loaded.source_name.clone();
    app.project_path = loaded.path.clone();
    app.canvas_pan = Vector::new(0.0, 0.0);
    app.zoom_level = 100;
    app.canvas_selection = None;
    app.welcome_error = None;
    app.history.reset();
    app.invalidate_fallback();
    Task::none()
}

fn handle_project_opened_err(app: &mut PhotoApp, e: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = Some(e);
    Task::none()
}

fn handle_save_project(app: &mut PhotoApp) -> Task<Message> {
    if app.background_tasks.is_empty() && app.doc_dims().is_some() {
        match app.project_path.clone() {
            Some(path) => return save_project_task(app, path),
            None => return save_as_dialog_task(),
        }
    }
    Task::none()
}

fn handle_save_project_as(app: &mut PhotoApp) -> Task<Message> {
    if app.background_tasks.is_empty() {
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
        app.project_path = Some(path.clone());
        return save_project_task(app, path);
    }
    Task::none()
}

fn handle_project_saved_ok(app: &mut PhotoApp, name: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = None;
    // The project name feeds the canvas title if it is empty
    app.image_path.get_or_insert(name);
    Task::none()
}

fn handle_project_saved_err(app: &mut PhotoApp, e: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = Some(e);
    Task::none()
}

fn handle_export_image(app: &mut PhotoApp) -> Task<Message> {
    if app.background_tasks.is_empty() && app.doc_dims().is_some() {
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
        && app.background_tasks.is_empty()
        && app.doc_dims().is_some()
    {
        return export_image_task(app, path);
    }
    Task::none()
}

fn handle_image_exported_ok(app: &mut PhotoApp, _name: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = None;
    // Discreet confirmation via the error zone (green in the future UI)
    Task::none()
}

fn handle_image_exported_err(app: &mut PhotoApp, e: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = Some(e);
    Task::none()
}

fn handle_open_image(app: &mut PhotoApp) -> Task<Message> {
    if app.background_tasks.is_empty() {
        pick_image_task(Message::ImagePicked)
    } else {
        Task::none()
    }
}

fn handle_image_picked(app: &mut PhotoApp, path_opt: Option<std::path::PathBuf>) -> Task<Message> {
    if let Some(path) = path_opt
        && app.background_tasks.is_empty()
    {
        app.background_tasks
            .push(format!("Lecture de {}", file_label(&path)));
        return read_file_task(path, Message::ImageRead);
    }
    Task::none()
}

fn handle_image_read_ok(app: &mut PhotoApp, bytes: Vec<u8>, name: String) -> Task<Message> {
    app.image_path = Some(name.clone());
    app.image_error = None;
    app.background_tasks.clear();
    app.background_tasks.push(format!("Décodage de {name}"));
    // The decoding + buffer construction run off the UI thread (Task::perform)
    // while the spinner keeps animating.
    Task::perform(
        async move {
            match ::image::load_from_memory(&bytes) {
                Ok(dyn_img) => Ok(DecodedLayer(PixelLayer::new(name, Arc::new(dyn_img)))),
                Err(e) => Err(format!("Décodage échoué: {e}")),
            }
        },
        Message::ImageDecoded,
    )
}

fn handle_image_read_err(app: &mut PhotoApp, e: String) -> Task<Message> {
    app.background_tasks.clear();
    app.image_error = Some(e);
    Task::none()
}

fn handle_image_decoded_ok(app: &mut PhotoApp, decoded: DecodedLayer) -> Task<Message> {
    // Ne nettoie QUE les tâches de décodage/lecture : un Export ou une
    // sauvegarde concurrente ne doivent pas être effacés.
    app.background_tasks
        .retain(|t| !t.starts_with("Décodage") && !t.starts_with("Lecture"));
    let node = LayerNode::Pixel(decoded.0);
    // The document takes the dimensions of the first image
    if app.doc.width == 0 || app.doc.height == 0 {
        let (w, h) = node_dimensions(&node);
        app.doc.width = w;
        app.doc.height = h;
        app.canvas_pan = Vector::new(0.0, 0.0);
        app.canvas_selection = None;
        app.zoom_level = 100;
    }
    app.history.push_snapshot(app.snapshot());
    let new_id = node.id();
    app.doc.push_layer(node);
    app.selected_layer = Some(new_id);
    app.invalidate_fallback();
    Task::none()
}

fn handle_image_decoded_err(app: &mut PhotoApp, e: String) -> Task<Message> {
    app.background_tasks
        .retain(|t| !t.starts_with("Décodage") && !t.starts_with("Lecture"));
    app.image_error = Some(e);
    Task::none()
}

fn handle_new_doc_width(app: &mut PhotoApp, v: String) -> Task<Message> {
    app.new_doc_w = v;
    app.welcome_error = None;
    Task::none()
}

fn handle_new_doc_height(app: &mut PhotoApp, v: String) -> Task<Message> {
    app.new_doc_h = v;
    app.welcome_error = None;
    Task::none()
}

fn handle_set_doc_preset(app: &mut PhotoApp, w: u32, h: u32) -> Task<Message> {
    app.new_doc_w = w.to_string();
    app.new_doc_h = h.to_string();
    app.welcome_error = None;
    Task::none()
}

fn handle_create_document(app: &mut PhotoApp) -> Task<Message> {
    let parsed = (
        app.new_doc_w.trim().parse::<u32>(),
        app.new_doc_h.trim().parse::<u32>(),
    );
    match parsed {
        (Ok(w), Ok(h)) if (1..=10000).contains(&w) && (1..=10000).contains(&h) => {
            let white = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
                w,
                h,
                ::image::Rgba([255, 255, 255, 255]),
            ));
            let layer = PixelLayer::new("Arrière-plan", Arc::new(white));
            let id = layer.id;
            app.doc.restore(w, h, vec![LayerNode::Pixel(layer)]);
            app.selected_layer = Some(id);
            app.image_path = None;
            app.image_error = None;
            app.canvas_pan = Vector::new(0.0, 0.0);
            app.zoom_level = 100;
            app.welcome_error = None;
            app.project_path = None;
            app.history.reset();
            app.invalidate_fallback();
        }
        _ => {
            app.welcome_error = Some("Dimensions invalides (1 à 10000 px)".into());
        }
    }
    Task::none()
}

fn handle_show_resize_dialog(app: &mut PhotoApp) -> Task<Message> {
    app.resize_dialog_open = !app.resize_dialog_open;
    if app.resize_dialog_open {
        let (w, h) = app.doc_dims().unwrap_or((800, 600));
        app.resize_w = w.to_string();
        app.resize_h = h.to_string();
    }
    Task::none()
}

fn handle_set_resize_width(app: &mut PhotoApp, s: String) -> Task<Message> {
    app.resize_w = s;
    Task::none()
}

fn handle_set_resize_height(app: &mut PhotoApp, s: String) -> Task<Message> {
    app.resize_h = s;
    Task::none()
}

fn handle_resize_document(app: &mut PhotoApp, width: u32, height: u32) -> Task<Message> {
    let w = width.max(1);
    let h = height.max(1);
    let pre = app.snapshot();
    app.doc.width = w;
    app.doc.height = h;
    app.resize_dialog_open = false;
    app.history.push_snapshot(pre);
    app.invalidate_fallback();
    Task::none()
}

pub fn handle(app: &mut PhotoApp, msg: Message) -> Option<Task<Message>> {
    match msg {
        Message::NewProject => Some(handle_new_project(app)),
        Message::OpenProject => Some(handle_open_project(app)),
        Message::ProjectOpenPicked(p) => Some(handle_project_open_picked(app, p)),
        Message::ProjectOpened(Ok(loaded)) => Some(handle_project_opened_ok(app, loaded)),
        Message::ProjectOpened(Err(e)) => Some(handle_project_opened_err(app, e)),
        Message::SaveProject => Some(handle_save_project(app)),
        Message::SaveProjectAs => Some(handle_save_project_as(app)),
        Message::SaveProjectPathPicked(p) => Some(handle_save_project_path_picked(app, p)),
        Message::ProjectSaved(Ok(name)) => Some(handle_project_saved_ok(app, name)),
        Message::ProjectSaved(Err(e)) => Some(handle_project_saved_err(app, e)),
        Message::ExportImage => Some(handle_export_image(app)),
        Message::ExportPathPicked(p) => Some(handle_export_path_picked(app, p)),
        Message::ImageExported(Ok(name)) => Some(handle_image_exported_ok(app, name)),
        Message::ImageExported(Err(e)) => Some(handle_image_exported_err(app, e)),
        Message::OpenImage => Some(handle_open_image(app)),
        Message::ImagePicked(p) => Some(handle_image_picked(app, p)),
        Message::ImageRead(Ok((bytes, name))) => Some(handle_image_read_ok(app, bytes, name)),
        Message::ImageRead(Err(e)) => Some(handle_image_read_err(app, e)),
        Message::ImageDecoded(Ok(decoded)) => Some(handle_image_decoded_ok(app, decoded)),
        Message::ImageDecoded(Err(e)) => Some(handle_image_decoded_err(app, e)),
        Message::NewDocWidth(v) => Some(handle_new_doc_width(app, v)),
        Message::NewDocHeight(v) => Some(handle_new_doc_height(app, v)),
        Message::SetDocPreset { w, h } => Some(handle_set_doc_preset(app, w, h)),
        Message::CreateDocument => Some(handle_create_document(app)),
        Message::ShowResizeDialog => Some(handle_show_resize_dialog(app)),
        Message::SetResizeWidth(s) => Some(handle_set_resize_width(app, s)),
        Message::SetResizeHeight(s) => Some(handle_set_resize_height(app, s)),
        Message::ResizeDocument { width, height } => {
            Some(handle_resize_document(app, width, height))
        }
        _ => None,
    }
}
