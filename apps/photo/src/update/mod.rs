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
/// Chemin de rendu UNIQUE (GPU) : le draw relit le document à chaque frame,
/// aucune tâche de composite d'affichage n'est nécessaire.
pub fn update(app: &mut PhotoApp, message: Message) -> Task<Message> {
    let task = dispatch(app, message);
    app.rendering.preview_cache.sync(&app.document.doc);
    task
}

fn dispatch(app: &mut PhotoApp, message: Message) -> Task<Message> {
    // Aiguillage SANS clonage. PaintApplied transporte un buffer image
    // COMPLET : un clone par module était un coût O(buffer) à CHAQUE
    // message, soit un gel du thread UI à la fin de chaque traitement.
    // `handles()` ne fait que traiter le discriminant (aucune copie), puis
    // le message est transmis PAR DÉPLACEMENT au module qui le possède.
    if layers::handles(&message) {
        return layers::handle(app, message).unwrap_or_default();
    }
    if paint::handles(&message) {
        return paint::handle(app, message).unwrap_or_default();
    }
    if project::handles(&message) {
        return project::handle(app, message).unwrap_or_default();
    }
    if panels::handles(&message) {
        return panels::handle(app, message).unwrap_or_default();
    }
    if misc::handles(&message) {
        return misc::handle(app, message).unwrap_or_default();
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
        app.document
            .doc
            .push_layer(photo_engine::LayerNode::Pixel(layer));
        app.document.selected_layer = Some(id);
        app.document.history.push_snapshot(app.snapshot());
        id
    }

    /// Entrée pré-chauffée factice (signatures nulles : jamais HIT, mais
    /// insertion et application sont testables telles quelles).
    fn fake_warmed() -> photo_engine::WarmedAppearance {
        let img = std::sync::Arc::new(image::DynamicImage::ImageRgba8(
            image::ImageBuffer::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])),
        ));
        photo_engine::WarmedAppearance {
            filter_signature: 0,
            source: std::sync::Arc::clone(&img),
            unmasked: std::sync::Arc::clone(&img),
            mask_signature: 0,
            mask_cover: None,
            appearance: photo_engine::Appearance {
                image: img,
                preview: photo_engine::RgbaBuf::from_vec(2, 2, vec![0u8; 16]),
                thumb: photo_engine::RgbaBuf::from_vec(2, 2, vec![0u8; 16]),
            },
        }
    }

    /// Sème un filtre brightness_contrast avec un param `brightness` fixé.
    fn seed_filter(app: &mut PhotoApp, pid: uuid::Uuid, brightness: f32) -> uuid::Uuid {
        let fid = app
            .document
            .doc
            .add_filter(
                pid,
                photo_engine::FilterLayer::neutral("brightness_contrast", Default::default()),
            )
            .expect("filtre");
        app.document.doc.set_filter_param(
            pid,
            fid,
            "brightness".to_string(),
            datatypes::ParamValue::Float(brightness),
        );
        fid
    }

    fn param_value(app: &PhotoApp, pid: uuid::Uuid, fid: uuid::Uuid) -> Option<f32> {
        app.document
            .doc
            .find(pid)
            .and_then(|n| match n {
                photo_engine::LayerNode::Pixel(l) => l
                    .filter_layers
                    .iter()
                    .find(|f| f.id == fid)?
                    .params
                    .get("brightness")
                    .cloned(),
                _ => None,
            })
            .and_then(|v| v.as_float())
    }

    /// Slider différé : le tick ne touche pas le vivant (sync HIT), le
    /// pouce lit `pending_param`, la réception applique + rattrape.
    #[test]
    fn slider_differe_puis_rattrape() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let pid = seed_layer(&mut app, 2, 2);
        let fid = seed_filter(&mut app, pid, 10.0);

        let _ = update(
            &mut app,
            Message::SetFilterParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
            },
        );
        // Vivant intact (zéro freeze), demande mémorisée pour le pouce.
        assert_eq!(param_value(&app, pid, fid), Some(10.0));
        assert_eq!(
            app.rendering.pending_param,
            Some(crate::message::PendingParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
            })
        );

        let _ = update(
            &mut app,
            Message::ParamWarmed {
                task_id: 0,
                epoch: 0,
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(20.0));
        assert_eq!(app.rendering.pending_param, None);
        assert!(app.rendering.warm_inflight.is_empty());
    }

    /// Rafale : un second tick pendant le vol est mémorisé puis enchaîné
    /// par le front descendant (coalescé en une seule entrée undo).
    #[test]
    fn slider_rafale_enchainee_coalescee() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let pid = seed_layer(&mut app, 2, 2);
        let fid = seed_filter(&mut app, pid, 10.0);
        let before = app.document.history.undo_len();

        // Tick 1 : front montant (vol simulé : on ne complète pas).
        let _ = update(
            &mut app,
            Message::SetFilterParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
            },
        );
        assert!(app.rendering.warm_inflight.contains(&pid));
        // Tick 2 pendant le vol : mémorisé, aucun nouveau vol.
        let _ = update(
            &mut app,
            Message::SetFilterParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(30.0),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(10.0));

        // Réception du vol 1 : applique 20, enchaîne 30 (nouveau vol).
        let _ = update(
            &mut app,
            Message::ParamWarmed {
                task_id: 0,
                epoch: 0,
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(20.0));
        assert!(app.rendering.warm_inflight.contains(&pid));

        // Réception du vol 2 : applique 30, rattrapé.
        let _ = update(
            &mut app,
            Message::ParamWarmed {
                task_id: 1,
                epoch: 0,
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(30.0),
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(30.0));
        assert_eq!(app.rendering.pending_param, None);

        // Un seul undo pour tout le geste (coalescence préservée).
        let _ = update(&mut app, Message::Undo);
        assert_eq!(param_value(&app, pid, fid), Some(10.0));
        assert_eq!(app.document.history.undo_len(), before);
    }

    /// Undo pendant le vol : l'époque invalide la réception, le vivant
    /// garde l'état annulé, le pouce retombe.
    #[test]
    fn slider_vol_perime_par_undo() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let pid = seed_layer(&mut app, 2, 2);
        let fid = seed_filter(&mut app, pid, 10.0);
        let before = app.document.history.undo_len();

        // Vol 1 appliqué : vivant à 20, entrée d'historique coalescée.
        let _ = update(
            &mut app,
            Message::SetFilterParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
            },
        );
        let _ = update(
            &mut app,
            Message::ParamWarmed {
                task_id: 0,
                epoch: 0,
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(20.0),
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(20.0));

        // Vol 2 en cours, puis undo : annule le vol 1, bump l'époque,
        // vide l'attente.
        let _ = update(
            &mut app,
            Message::SetFilterParam {
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(30.0),
            },
        );
        let _ = update(&mut app, Message::Undo);
        assert_eq!(param_value(&app, pid, fid), Some(10.0));
        assert_eq!(app.rendering.pending_param, None);

        // Réception périmée : jetée, le vivant garde l'état annulé.
        let _ = update(
            &mut app,
            Message::ParamWarmed {
                task_id: 1,
                epoch: 0,
                layer_id: pid,
                filter_id: fid,
                key: "brightness".to_string(),
                value: datatypes::ParamValue::Float(30.0),
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(param_value(&app, pid, fid), Some(10.0));
        assert!(app.rendering.warm_inflight.is_empty());
        assert_eq!(app.document.history.undo_len(), before);
    }

    /// Toggle différé : le flag ne bouge qu'à la réception.
    #[test]
    fn toggle_filtre_differe() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let pid = seed_layer(&mut app, 2, 2);
        let fid = seed_filter(&mut app, pid, 10.0);
        let before = app.document.history.undo_len();

        let _ = update(
            &mut app,
            Message::ToggleFilterEnabled {
                layer_id: pid,
                filter_id: fid,
            },
        );
        // Vivant intact pendant le pré-chauffage.
        assert_eq!(
            app.document.doc.find(pid).and_then(|n| match n {
                photo_engine::LayerNode::Pixel(l) => l
                    .filter_layers
                    .iter()
                    .find(|f| f.id == fid)
                    .map(|f| f.enabled),
                _ => None,
            }),
            Some(true)
        );
        let _ = update(
            &mut app,
            Message::AppearanceWarmed {
                task_id: 0,
                layer_id: pid,
                op: crate::message::AppearanceToggle::FilterEnabled {
                    filter_id: fid,
                    enabled: false,
                },
                result: Ok(fake_warmed()),
            },
        );
        assert_eq!(
            app.document.doc.find(pid).and_then(|n| match n {
                photo_engine::LayerNode::Pixel(l) => l
                    .filter_layers
                    .iter()
                    .find(|f| f.id == fid)
                    .map(|f| f.enabled),
                _ => None,
            }),
            Some(false)
        );
        assert_eq!(app.document.history.undo_len(), before + 1);
        assert!(app.rendering.warm_inflight.is_empty());
    }

    #[test]
    fn cycle_calque_undo_redo() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
        let _ = update(&mut app, Message::SetLayerOpacity { id, opacity: 42.0 });
        assert_eq!(
            app.document
                .doc
                .find(id)
                .expect("layer should exist after SetLayerOpacity")
                .opacity(),
            42.0
        );
        let _ = update(&mut app, Message::Undo);
        assert_eq!(
            app.document
                .doc
                .find(id)
                .expect("layer should exist after Undo")
                .opacity(),
            100.0
        );
        let _ = update(&mut app, Message::Redo);
        assert_eq!(
            app.document
                .doc
                .find(id)
                .expect("layer should exist after Redo")
                .opacity(),
            42.0
        );
    }

    #[test]
    fn duplication_produit_nouvel_id() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(2, 2);
        let id = seed_layer(&mut app, 2, 2);
        let id2 = seed_layer(&mut app, 2, 2);
        let _ = update(&mut app, Message::DuplicateLayer(id2));
        let dup = app
            .document
            .selected_layer
            .expect("selected layer should exist");
        assert_ne!(dup, id2);
        assert_ne!(dup, id);
        assert_eq!(app.document.doc.pixel_count(), 3);
    }

    #[test]
    fn suppression_dernier_calque_refusee() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(2, 2);
        let id = seed_layer(&mut app, 2, 2);
        assert_eq!(app.document.doc.pixel_count(), 1);
        let _ = update(&mut app, Message::DeleteLayer(id));
        assert_eq!(
            app.document.doc.pixel_count(),
            1,
            "dernier calque non supprimable"
        );
    }

    #[test]
    fn coalescing_opacite_en_un_undo() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(2, 2);
        let id = seed_layer(&mut app, 2, 2);
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            let _ = update(&mut app, Message::SetLayerOpacity { id, opacity: v });
        }
        let _ = update(&mut app, Message::Undo);
        assert_eq!(
            app.document
                .doc
                .find(id)
                .expect("layer should exist after coalesced undo")
                .opacity(),
            100.0
        );
    }

    /// Pendant un déplacement, AUCUNE tâche : le chemin GPU unique redessine
    /// le transform en direct au draw. Un simple clic de sélection ne coûte
    /// rien ; un geste n'enregistre qu'UNE entrée d'historique.
    #[test]
    fn deplacement_gpu_zero_tache() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
        // Masque + fusion MULTIPLY : l'ancien repli CPU est parti, le GPU
        // couvre le cas — le geste reste gratuit dans tous les cas.
        let mask_img = image::ImageBuffer::from_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        app.document
            .doc
            .pixel_layer_mut(id)
            .expect("layer should exist")
            .masks
            .push(photo_engine::LayerMask {
                id: uuid::Uuid::new_v4(),
                name: String::from("Masque"),
                image: std::sync::Arc::new(mask_img),
                enabled: true,
                inverted: false,
                version: 0,
            });
        app.document.doc.pixel_layer_mut(id).unwrap().blend_mode =
            photo_engine::BlendMode::Multiply;

        let _ = update(&mut app, Message::SelectTool(crate::message::Tool::Select));

        // Sélection seule (clic sans mouvement) : geste actif, rien d'autre.
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformStart {
                id: Some(id),
                kind: ui_kit::image_canvas::TransformHandle::Move,
                doc: (0.0, 0.0),
            }),
        );
        assert!(app.tools.move_anchor.is_some(), "geste actif");
        assert!(
            app.rendering.background_tasks.is_empty(),
            "rien de lancé au clic seul"
        );

        // Mouvements : le transform suit en direct, sans tâche ni historique.
        let avant = app.document.history.undo_len();
        for (dx, dy) in [(1.0, 0.0), (2.0, 0.5), (3.0, 0.75), (3.5, 1.25)] {
            let _ = update(
                &mut app,
                Message::ImageCanvasEvent(
                    ui_kit::image_canvas::ImageCanvasEvent::TransformCursor {
                        doc: (dx, dy),
                        uniform: false,
                        snap: false,
                    },
                ),
            );
            let t = app.document.doc.pixel_layer(id).unwrap().transform;
            assert!(
                (t.offset_x - dx).abs() < 1e-6 && (t.offset_y - dy).abs() < 1e-6,
                "le transform suit le curseur en direct",
            );
            assert!(
                app.rendering.background_tasks.is_empty(),
                "aucune tâche pendant le geste"
            );
            assert_eq!(
                app.document.history.undo_len(),
                avant,
                "aucune entrée d'historique pendant le geste"
            );
        }

        // Fin du geste : UNE entrée ancre→finale, ancre libérée.
        let _ = update(
            &mut app,
            Message::ImageCanvasEvent(ui_kit::image_canvas::ImageCanvasEvent::TransformEnd),
        );
        assert_eq!(app.tools.move_anchor, None, "geste terminé");
        assert_eq!(
            app.document.history.undo_len(),
            avant + 1,
            "exactement UNE entrée au relâchement"
        );
        let t = app.document.doc.pixel_layer(id).unwrap().transform;
        assert!(
            (t.offset_x - 3.5).abs() < 1e-6 && (t.offset_y - 1.25).abs() < 1e-6,
            "position finale conservée"
        );
    }

    /// Clic simple dans une scène masquée : ni tâche, ni entrée
    /// d'historique — la sélection ne doit rien coûter.
    #[test]
    fn clic_selection_sans_mouvement_ne_lance_aucune_composite() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id = seed_layer(&mut app, 2, 2);
        let mask_img = image::ImageBuffer::from_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        app.document
            .doc
            .pixel_layer_mut(id)
            .expect("layer should exist")
            .masks
            .push(photo_engine::LayerMask {
                id: uuid::Uuid::new_v4(),
                name: String::from("Masque"),
                image: std::sync::Arc::new(mask_img),
                enabled: true,
                inverted: false,
                version: 0,
            });
        app.document.doc.pixel_layer_mut(id).unwrap().blend_mode =
            photo_engine::BlendMode::Multiply;

        // Clic simple : Start (sélection) + End, AUCUN mouvement entre les
        // deux. Doit être gratuit — ni tâche, ni entrée d'historique
        // au-delà du seed initial.
        let before = app.document.history.undo_len();
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
        assert_eq!(app.tools.move_anchor, None, "geste terminé");
        assert!(
            app.rendering.background_tasks.is_empty(),
            "aucune tâche lancée"
        );
        assert_eq!(
            app.document.history.undo_len(),
            before,
            "aucune entrée d'historique pour un clic immobile"
        );
    }

    #[test]
    fn drag_drop_couche_cree_une_entree_unique() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id1 = seed_layer(&mut app, 2, 2);
        let id2 = seed_layer(&mut app, 2, 2);
        let before = app.document.history.undo_len();

        let _ = update(&mut app, Message::LayerDragPressed { id: id2 });
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (0.0, 0.0),
            },
        );
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (10.0, 0.0),
            },
        );
        let _ = update(
            &mut app,
            Message::LayerDragHover {
                hovered: Some(id1),
                position: crate::components::layers::DropPosition::Before,
            },
        );
        let _ = update(&mut app, Message::LayerDragReleased);

        let order: Vec<uuid::Uuid> = app.document.doc.root.iter().map(|node| node.id()).collect();
        assert_eq!(order, vec![id2, id1]);
        assert_eq!(app.document.history.undo_len(), before + 1);
    }

    #[test]
    fn drag_sans_cible_sans_historique() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id1 = seed_layer(&mut app, 2, 2);
        let _ = seed_layer(&mut app, 2, 2);
        let before = app.document.history.undo_len();

        let _ = update(&mut app, Message::LayerDragPressed { id: id1 });
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (0.0, 0.0),
            },
        );
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (10.0, 0.0),
            },
        );
        let _ = update(&mut app, Message::LayerDragReleased);

        assert_eq!(app.document.history.undo_len(), before);
        assert!(matches!(
            app.tools.layer_drag,
            crate::components::layers::LayerDragState::Idle
        ));
    }

    #[test]
    fn drag_annule_sans_selection_ni_historique() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id1 = seed_layer(&mut app, 2, 2);
        let _ = seed_layer(&mut app, 2, 2);
        let before = app.document.history.undo_len();

        let _ = update(&mut app, Message::LayerDragPressed { id: id1 });
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (10.0, 0.0),
            },
        );
        let _ = update(&mut app, Message::LayerDragCancelled);

        assert_eq!(app.document.history.undo_len(), before);
        assert!(matches!(
            app.tools.layer_drag,
            crate::components::layers::LayerDragState::Idle
        ));
    }

    #[test]
    fn pinceau_masque_filtre_cible_porteur() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let pid = seed_layer(&mut app, 2, 2);
        let fid = app
            .document
            .doc
            .add_filter(
                pid,
                photo_engine::FilterLayer::neutral("brightness_contrast", Default::default()),
            )
            .expect("filtre");
        let mid = {
            let masks = app
                .document
                .doc
                .masks_of_mut(fid)
                .expect("masques du filtre");
            let mask = photo_engine::LayerMask::full(2, 2);
            let mid = mask.id;
            masks.push(mask);
            mid
        };

        // Masque de filtre actif : le trait part du calque porteur.
        let _ = update(&mut app, Message::SelectLayer(fid));
        let _ = update(
            &mut app,
            Message::SetActiveMask(Some(crate::message::MaskTarget {
                layer_id: fid,
                mask_id: mid,
            })),
        );
        let _ = update(
            &mut app,
            Message::BrushStart {
                x: 0.0,
                y: 0.0,
                erase: false,
            },
        );
        assert_eq!(app.tools.stroke_layer, Some(pid));

        // Sans masque actif : filtre sélectionné → pas de trait (conservé).
        let _ = update(&mut app, Message::SetActiveMask(None));
        app.tools.stroke_layer = None;
        let _ = update(
            &mut app,
            Message::BrushStart {
                x: 0.0,
                y: 0.0,
                erase: false,
            },
        );
        assert_eq!(app.tools.stroke_layer, None);
    }

    #[test]
    fn drop_noop_sans_entree_historique() {
        let mut app = PhotoApp::default();
        app.document.doc = photo_engine::Document::new(4, 4);
        let id1 = seed_layer(&mut app, 2, 2);
        let id2 = seed_layer(&mut app, 2, 2);
        let before = app.document.history.undo_len();

        let _ = update(&mut app, Message::LayerDragPressed { id: id2 });
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (0.0, 0.0),
            },
        );
        let _ = update(
            &mut app,
            Message::LayerDragMoved {
                position: (10.0, 0.0),
            },
        );
        let _ = update(
            &mut app,
            Message::LayerDragHover {
                hovered: Some(id1),
                position: crate::components::layers::DropPosition::After,
            },
        );
        let _ = update(&mut app, Message::LayerDragReleased);

        let order: Vec<uuid::Uuid> = app.document.doc.root.iter().map(|node| node.id()).collect();
        assert_eq!(order, vec![id1, id2]);
        assert_eq!(app.document.history.undo_len(), before);
    }
}
