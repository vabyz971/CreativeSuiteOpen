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
    // Aiguillage SANS clonage. FallbackComputed, PaintApplied et les
    // Drag*Computed transportent des buffers image COMPLETS : un clone par
    // module (x4) était un coût O(buffer) à CHAQUE message, soit un gel du
    // thread UI à la fin de chaque traitement. `handles()` ne fait que
    // traiter le discriminant (aucune copie), puis le message est transmis
    // PAR DÉPLACEMENT au module qui le possède.
    if layers::handles(&message) {
        return layers::handle(app, message).unwrap_or_else(Task::none);
    }
    if paint::handles(&message) {
        return paint::handle(app, message).unwrap_or_else(Task::none);
    }
    if project::handles(&message) {
        return project::handle(app, message).unwrap_or_else(Task::none);
    }
    if panels::handles(&message) {
        return panels::handle(app, message).unwrap_or_else(Task::none);
    }
    if misc::handles(&message) {
        return misc::handle(app, message).unwrap_or_else(Task::none);
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

    /// Ajoute un calque PIXEL directement via le moteur (équivalent
    /// synchrone du message asynchrone `AddEmptyLayer` + `ImageDecoded`) —
    /// les tests se concentrent sur les handlers de transformation, pas sur
    /// le pipeline async.
    fn seed_layer(app: &mut PhotoApp, w: u32, h: u32) -> uuid::Uuid {
        let layer = photo_engine::PixelLayer::new("Test", solid_img(w, h));
        let id = layer.id;
        app.doc.push_layer(photo_engine::LayerNode::Pixel(layer));
        app.selected_layer = Some(id);
        app.history.push_snapshot(app.snapshot());
        id
    }

    #[test]
    fn cycle_calque_undo_redo() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
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
        let id = seed_layer(&mut app, 2, 2);
        let id2 = seed_layer(&mut app, 2, 2);
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
        let id = seed_layer(&mut app, 2, 2);
        assert_eq!(app.doc.pixel_count(), 1);
        let _ = update(&mut app, Message::DeleteLayer(id));
        assert_eq!(app.doc.pixel_count(), 1, "dernier calque non supprimable");
    }

    #[test]
    fn coalescing_opacite_en_un_undo() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(2, 2);
        let id = seed_layer(&mut app, 2, 2);
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            let _ = update(&mut app, Message::SetLayerOpacity { id, opacity: v });
        }
        let _ = update(&mut app, Message::Undo);
        assert_eq!(app.doc.find(id).unwrap().opacity(), 100.0);
    }

    /// Pendant un déplacement (outil Déplacer), AUCUN message ne doit
    /// déclencher de recomposite : les pré-calculs drag (fond sans le calque
    /// et composite masqué) ne sont lancés qu'au PREMIER mouvement réel
    /// (`TransformCursor`). Un simple clic de sélection ne coûte rien.
    #[test]
    fn drag_masque_zero_recomposite_par_mouvement() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
        // Masque actif → le rendu passe obligatoirement par le fallback.
        let mask_img = image::ImageBuffer::from_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        app.doc
            .pixel_layer_mut(id)
            .unwrap()
            .masks
            .push(photo_engine::LayerMask {
                id: uuid::Uuid::new_v4(),
                image: std::sync::Arc::new(mask_img),
                enabled: true,
                inverted: false,
                version: 0,
            });
        assert!(app.needs_fallback(), "masque actif → fallback");

        let _ = update(&mut app, Message::SelectTool(crate::message::Tool::Move));

        // Sélection seule (clic sans mouvement) : AUCUN pré-calcul lancé.
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformStart {
                id: Some(id),
                kind: ui_kit::image_canvas::TransformHandle::Move,
                doc: (0.0, 0.0),
            }),
        );
        assert!(app.move_anchor.is_some(), "geste actif");
        assert!(
            app.drag_bg_in_flight.is_none(),
            "rien de lancé au clic seul"
        );
        assert!(!app.fallback_dirty, "clic seul : fallback intact");

        // Premier mouvement réel → pré-calculs lancés, une seule fois.
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformCursor {
                doc: (0.1, 0.0),
                uniform: false,
            }),
        );
        assert!(app.drag_bg_in_flight.is_some(), "fond de drag pré-calculé");
        assert!(
            app.drag_layer_composite_in_flight,
            "composite masqué pré-calculé"
        );

        // Mouvements : le transform seul change — JAMAIS de recomposite.
        for (i, (dx, dy)) in [(1.0, 0.0), (2.0, 0.5), (3.0, 0.75), (3.5, 1.25)]
            .iter()
            .enumerate()
        {
            let _ = update(
                &mut app,
                Message::ImageCanvasEvent(
                    ui_kit::image_canvas::ImageCanvasEvent::TransformCursor {
                        doc: (*dx, *dy),
                        uniform: false,
                    },
                ),
            );
            assert!(!app.fallback_dirty, "move {i} : fallback non invalide");
            assert!(
                app.take_fallback_task().is_none(),
                "move {i} : aucune recomposite pendant le geste"
            );
        }

        // Fin du geste : UNE recomposite (le vrai blend), lancée par boucle.
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformEnd),
        );
        assert!(
            app.fallback_in_flight,
            "le relâchement lance exactement UNE recomposite"
        );
        assert!(
            app.take_fallback_task().is_none(),
            "aucune seconde recomposite lancée"
        );
    }

    /// Clic simple dans une scène masquée : ni composite, ni invalidation,
    /// ni entrée d'historique — la sélection ne doit rien coûter.
    #[test]
    fn clic_selection_sans_mouvement_ne_lance_aucune_composite() {
        let mut app = PhotoApp::default();
        app.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
        let mask_img = image::ImageBuffer::from_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        app.doc
            .pixel_layer_mut(id)
            .unwrap()
            .masks
            .push(photo_engine::LayerMask {
                id: uuid::Uuid::new_v4(),
                image: std::sync::Arc::new(mask_img),
                enabled: true,
                inverted: false,
                version: 0,
            });
        assert!(app.needs_fallback(), "masque actif → fallback");

        // Clic simple : Start (sélection) + End, AUCUN mouvement entre les
        // deux. Doit être gratuit — ni pré-calcul drag, ni recomposite, ni
        // entrée d'historique au-delà du seed initial.
        let before = app.history.undo_len();
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformStart {
                id: Some(id),
                kind: ui_kit::image_canvas::TransformHandle::Move,
                doc: (0.0, 0.0),
            }),
        );
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformEnd),
        );
        assert_eq!(app.move_anchor, None, "geste terminé");
        assert!(app.drag_bg_in_flight.is_none(), "aucun pré-calcul lancé");
        assert!(!app.fallback_dirty, "aucune recomposite au relâchement");
        assert!(app.take_fallback_task().is_none(), "aucune tâche fallback");
        assert_eq!(
            app.history.undo_len(),
            before,
            "aucune entrée d'historique pour un clic immobile"
        );
    }
}
