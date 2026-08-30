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

//! Update loop: one handler per message, side effects via Task.
//! The message arms are split into submodules (layers, paint, project,
//! panels, misc); `dispatch` delegates to each in turn.

use iced::Task;

use crate::message::Message;
use crate::state::PhotoApp;

mod layers;
mod misc;
mod paint;
mod panels;
mod project;

/// Point d'entrée : délègue au dispatch puis synchronise les handles UI
/// (cache dérivé des buffers purs du moteur — UN seul point de sync).
pub fn update(app: &mut PhotoApp, message: Message) -> Task<Message> {
    let task = dispatch(app, message);
    app.preview_cache.sync(&app.doc);
    // Le fallback périmé est recalculé HORS thread UI — jamais de gel.
    let fallback = app.take_fallback_task();
    Task::batch([task, fallback.unwrap_or_else(Task::none)])
}

fn dispatch(app: &mut PhotoApp, message: Message) -> Task<Message> {
    if let Some(t) = layers::handle(app, message.clone()) {
        return t;
    }
    if let Some(t) = paint::handle(app, message.clone()) {
        return t;
    }
    if let Some(t) = project::handle(app, message.clone()) {
        return t;
    }
    if let Some(t) = panels::handle(app, message.clone()) {
        return t;
    }
    if let Some(t) = misc::handle(app, message.clone()) {
        return t;
    }
    Task::none()
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::state::PhotoApp;

    #[allow(dead_code)]
    fn solid_img(w: u32, h: u32) -> std::sync::Arc<image::DynamicImage> {
        std::sync::Arc::new(image::DynamicImage::ImageRgba8(
            image::ImageBuffer::from_pixel(w, h, image::Rgba([10, 20, 30, 255])),
        ))
    }

    #[test]
    fn cycle_calque_undo_redo() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(4, 4);
        let _ = update(&mut app, Message::AddEmptyLayer);
        let id = app.selected_layer.expect("sélection");
        let _ = update(&mut app, Message::SetLayerOpacity { id, opacity: 42.0 });
        assert_eq!(app.doc.find(id).unwrap().opacity(), 42.0);
        let _ = update(&mut app, Message::Undo);
        assert_eq!(app.doc.find(id).unwrap().opacity(), 100.0);
        let _ = update(&mut app, Message::Redo);
        assert_eq!(app.doc.find(id).unwrap().opacity(), 42.0);
    }

    #[test]
    fn duplication_produit_nouvel_id() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(2, 2);
        let _ = update(&mut app, Message::AddEmptyLayer);
        let id = app.selected_layer.unwrap();
        // ajoute second calque pour pouvoir dupliquer
        let _ = update(&mut app, Message::AddEmptyLayer);
        let id2 = app.selected_layer.unwrap();
        let _ = update(&mut app, Message::DuplicateLayer(id2));
        let dup = app.selected_layer.unwrap();
        assert_ne!(dup, id2);
        assert_ne!(dup, id);
        assert_eq!(app.doc.pixel_count(), 3);
    }

    #[test]
    fn suppression_dernier_calque_refusee() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(2, 2);
        let _ = update(&mut app, Message::AddEmptyLayer);
        let id = app.selected_layer.unwrap();
        assert_eq!(app.doc.pixel_count(), 1);
        let _ = update(&mut app, Message::DeleteLayer(id));
        assert_eq!(app.doc.pixel_count(), 1, "dernier calque non supprimable");
    }

    #[test]
    fn coalescing_opacite_en_un_undo() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(2, 2);
        let _ = update(&mut app, Message::AddEmptyLayer);
        let id = app.selected_layer.unwrap();
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            let _ = update(&mut app, Message::SetLayerOpacity { id, opacity: v });
        }
        let _ = update(&mut app, Message::Undo);
        assert_eq!(app.doc.find(id).unwrap().opacity(), 100.0);
    }
}
