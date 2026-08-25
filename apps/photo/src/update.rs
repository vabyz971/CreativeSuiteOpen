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

//! Boucle de mise à jour : un handler par message, effets de bord via Task.

use std::sync::Arc;

use iced::widget::pane_grid;
use iced::{Task, Vector};

use crate::components;
use crate::layers::Layer;
use crate::message::{DecodedLayer, Message, OffsetAxis, PanelType};
use crate::state::PhotoApp;

/// Point d'entrée : délègue au dispatch puis synchronise les handles UI
/// (cache dérivé des buffers purs du moteur — UN seul point de sync).
pub fn update(app: &mut PhotoApp, message: Message) -> Task<Message> {
    let task = dispatch(app, message);
    app.preview_cache.sync(&app.layers);
    task
}

fn dispatch(app: &mut PhotoApp, message: Message) -> Task<Message> {
    match message {
        Message::NewProject => {
            app.layers.clear();
            app.selected_layer = None;
            app.next_layer_id = 0;
            app.doc_size = None;
            app.gen_graph = components::node_registry::create_empty_graph();
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
        }
        Message::OpenProject => {
            if app.background_tasks.is_empty() {
                return open_document_task();
            }
        }
        Message::ProjectOpenPicked(path_opt) => {
            if let Some(path) = path_opt
                && app.background_tasks.is_empty()
            {
                if path.extension().and_then(|e| e.to_str()) == Some("csphoto") {
                    let name = file_label(&path);
                    app.background_tasks.push(format!("Ouverture de {name}"));
                    return load_project_task(path);
                }
                // Image simple → flux de calque existant
                return read_file_task(path, Message::ImageRead);
            }
        }
        Message::ProjectOpened(Ok(loaded)) => {
            app.background_tasks.clear();
            app.image_error = None;
            app.doc_size = Some((loaded.width, loaded.height));
            app.next_layer_id = loaded.layers.iter().map(|l| l.id + 1).max().unwrap_or(0);
            app.layers = loaded.layers;
            app.selected_layer = app.layers.last().map(|l| l.id);
            app.image_path = loaded.source_name.clone();
            app.project_path = loaded.path.clone();
            app.canvas_pan = Vector::new(0.0, 0.0);
            app.zoom_level = 100;
            app.canvas_selection = None;
            app.welcome_error = None;
            app.history.reset();
            app.refresh_fallback();
        }
        Message::ProjectOpened(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::SaveProject => {
            if app.background_tasks.is_empty()
                && let Some((w, h)) = app.doc_size
            {
                match app.project_path.clone() {
                    Some(path) => return save_project_task(app, path, w, h),
                    None => return save_as_dialog_task(),
                }
            }
        }
        Message::SaveProjectAs => {
            if app.background_tasks.is_empty() {
                return save_as_dialog_task();
            }
        }
        Message::SaveProjectPathPicked(path_opt) => {
            if let Some(mut path) = path_opt
                && let Some((w, h)) = app.doc_size
            {
                if path.extension().and_then(|e| e.to_str()) != Some("csphoto") {
                    path.set_extension("csphoto");
                }
                app.project_path = Some(path.clone());
                return save_project_task(app, path, w, h);
            }
        }
        Message::ProjectSaved(Ok(name)) => {
            app.background_tasks.clear();
            app.image_error = None;
            // Le nom du projet alimente le titre du canvas s'il est vide
            app.image_path.get_or_insert(name);
        }
        Message::ProjectSaved(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::OpenImage => {
            if app.background_tasks.is_empty() {
                return pick_image_task(Message::ImagePicked);
            }
        }
        Message::ImagePicked(path_opt) => {
            if let Some(path) = path_opt
                && app.background_tasks.is_empty()
            {
                app.background_tasks
                    .push(format!("Lecture de {}", file_label(&path)));
                return read_file_task(path, Message::ImageRead);
            }
        }
        Message::ImageRead(Ok((bytes, name))) => {
            app.image_path = Some(name.clone());
            app.image_error = None;
            app.background_tasks.clear();
            app.background_tasks.push(format!("Décodage de {name}"));
            // Le décodage + la construction des buffers tournent hors UI
            // (Task::perform) — le spinner continue d'animer pendant ce temps
            let id = app.alloc_layer_id();
            return Task::perform(
                async move {
                    match ::image::load_from_memory(&bytes) {
                        Ok(dyn_img) => Ok(DecodedLayer(Layer::new(id, name, Arc::new(dyn_img)))),
                        Err(e) => Err(format!("Décodage échoué: {e}")),
                    }
                },
                Message::ImageDecoded,
            );
        }
        Message::ImageRead(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::ImageDecoded(Ok(decoded)) => {
            app.background_tasks.clear();
            let layer = decoded.0;
            // Le document prend les dimensions de la première image
            if app.doc_size.is_none() {
                let (w, h) = layer.dimensions();
                app.doc_size = Some((w, h));
                app.canvas_pan = Vector::new(0.0, 0.0);
                app.canvas_selection = None;
                app.zoom_level = 100;
            }
            app.history.push(app.snapshot());
            app.layers.push(layer);
            app.selected_layer = app.layers.last().map(|l| l.id);
            app.refresh_fallback();
        }
        Message::ImageDecoded(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::ToggleTaskMenu => {
            app.task_menu_open = !app.task_menu_open;
        }

        // ---- Pinceau / Gomme ----
        Message::BrushStart { .. } => {
            // Cible le calque sélectionné ; l'aperçu live est géré par le
            // canvas (State local). On ignore si un commit est en vol.
            if app.pending_paint.is_none()
                && let Some(id) = app.selected_layer
            {
                app.stroke_layer = Some(id);
            }
        }
        Message::BrushEnd { points, tex, erase } => {
            // Le travail lourd (copie RGBA, rastérisation, aperçu, miniature)
            // part sur un thread de fond (spawn_blocking) : l'UI reste fluide.
            // L'aperçu rastérisé au relâchement reste affiché tel quel
            // (pending_paint) jusqu'à PaintApplied — zéro clignotement.
            if let Some(id) = app.stroke_layer.take()
                && app.pending_paint.is_none()
                && points.len() > 1
                && let Some(layer) = app.layers.iter().find(|l| l.id == id).cloned()
                && let Some(tex) = tex
            {
                let pts = points;
                app.pending_paint = Some(crate::message::PendingPaint {
                    layer_id: id,
                    tex: tex.clone(),
                });
                // État PRÉ-trait figé dans l'historique maintenant : le
                // snapshot partage les pixels via Arc, coût quasi nul.
                app.history.push(app.snapshot());
                let brush = photo_engine::paint::BrushParams {
                    radius: app.brush_size / 2.0,
                    color: [
                        (app.brush_color.r * 255.0) as u8,
                        (app.brush_color.g * 255.0) as u8,
                        (app.brush_color.b * 255.0) as u8,
                    ],
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
                            photo_engine::paint::commit_stroke(
                                &layer.image,
                                &pts,
                                (layer.offset_x, layer.offset_y),
                                layer.scale,
                                layer.rotation,
                                &brush,
                            )
                        })
                        .await
                    },
                    move |result| match result {
                        Ok(buf) => Message::PaintApplied { layer_id: id, buf },
                        Err(_) => Message::PaintFailed { layer_id: id },
                    },
                );
            }
        }
        Message::PaintFailed { layer_id } => {
            // Le worker a paniqué (JoinError) : on retire l'aperçu figé et
            // on signale, sans jamais interrompre l'application.
            if app
                .pending_paint
                .as_ref()
                .is_some_and(|p| p.layer_id == layer_id)
            {
                app.pending_paint = None;
            }
            app.image_error = Some("Échec interne lors de l'application du trait".into());
        }
        Message::PaintApplied { layer_id, buf } => {
            if let Some(layer) = app.layers.iter_mut().find(|l| l.id == layer_id)
                && let Some(img) = ::image::RgbaImage::from_raw(buf.width, buf.height, buf.rgba)
            {
                layer.image = Arc::new(::image::DynamicImage::ImageRgba8(img));
                layer.preview = buf.preview;
                layer.thumb = buf.thumb;
            }
            app.pending_paint = None;
            app.refresh_fallback();
        }

        Message::SetBrushColor(color) => {
            app.brush_color = color;
            app.color_picker_open = false;
        }
        Message::SetBrushSize(size) => {
            app.brush_size = size;
        }
        Message::SetBrushOpacity(opacity) => {
            app.brush_opacity = opacity;
        }
        Message::ToggleColorPicker => {
            app.color_picker_open = !app.color_picker_open;
        }

        Message::NewDocWidth(v) => {
            app.new_doc_w = v;
            app.welcome_error = None;
        }
        Message::NewDocHeight(v) => {
            app.new_doc_h = v;
            app.welcome_error = None;
        }
        Message::SetDocPreset { w, h } => {
            app.new_doc_w = w.to_string();
            app.new_doc_h = h.to_string();
            app.welcome_error = None;
        }
        Message::CreateDocument => {
            let parsed = (
                app.new_doc_w.trim().parse::<u32>(),
                app.new_doc_h.trim().parse::<u32>(),
            );
            match parsed {
                (Ok(w), Ok(h)) if (1..=10000).contains(&w) && (1..=10000).contains(&h) => {
                    let white = ::image::DynamicImage::ImageRgba8(
                        ::image::ImageBuffer::from_pixel(w, h, ::image::Rgba([255, 255, 255, 255])),
                    );
                    let id = app.alloc_layer_id();
                    let layer = Layer::new(id, "Arrière-plan".into(), Arc::new(white));
                    app.layers.clear();
                    app.layers.push(layer);
                    app.selected_layer = Some(id);
                    app.doc_size = Some((w, h));
                    app.image_path = None;
                    app.image_error = None;
                    app.canvas_pan = Vector::new(0.0, 0.0);
                    app.zoom_level = 100;
                    app.welcome_error = None;
                    app.project_path = None;
                    app.history.reset();
                    app.refresh_fallback();
                }
                _ => {
                    app.welcome_error = Some("Dimensions invalides (1 à 10000 px)".into());
                }
            }
        }

        Message::OpenPreferences => {
            app.show_prefs = true;
            app.prefs_section = components::preferences::PrefsSection::General;
            app.capturing = None;
            // Détection GPU async pour la section Général
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::ClosePreferences => {
            app.show_prefs = false;
            app.capturing = None;
        }
        Message::PrefsSection(section) => {
            app.prefs_section = section;
        }
        Message::ShortcutCapture(action) => {
            app.capturing = Some(action);
        }
        Message::ShortcutCaptured(binding) => {
            if let Some(action) = app.capturing {
                if let Some(b) = binding {
                    app.shortcuts.set(action, b);
                    app.shortcuts.save();
                }
                app.capturing = None;
            }
        }
        Message::ShortcutCancelCapture => {
            app.capturing = None;
        }
        Message::ShortcutReset(action) => {
            app.shortcuts.reset(action);
            app.shortcuts.save();
        }
        Message::ShortcutResetAll => {
            app.shortcuts.reset_all();
            app.shortcuts.save();
        }
        Message::ShortcutAction(action) => {
            // Résolution action → Message (déléguée, une seule place)
            if let Some(msg) = PhotoApp::message_for(action) {
                // Re-dispatch récursif : réutilise tous les handlers existants
                return dispatch(app, msg);
            }
        }
        Message::TickFrame => {
            // Animation du spinner (~30 fps)
            app.spinner_angle = (app.spinner_angle + 24.0) % 360.0;
        }

        // ---- Calques ----
        Message::SelectLayer(id) => {
            app.selected_layer = Some(id);
            app.move_anchor = None;
        }
        Message::ToggleLayerVisible(id) => {
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].visible = !app.layers[i].visible;
                app.history.push(pre);
                app.refresh_fallback();
            }
        }
        Message::SetLayerOpacity { id, opacity } => {
            // Simple changement d'état : l'opacité est appliquée au draw
            // (GPU) — zéro régénération de pixels, zéro clignotement
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].opacity = opacity;
                // Geste continu (slider) : un seul point de restauration
                app.history.push_coalesced(coalesce_key(id, 1), pre);
            }
        }
        Message::SetLayerBlend { id, mode } => {
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].blend_mode = mode;
                app.history.push(pre);
                // Bascule chemin rapide ↔ fallback selon le mode
                if app.needs_fallback() {
                    app.refresh_fallback();
                } else {
                    app.fallback_handle = None;
                    app.fallback_size = None;
                }
            }
        }
        Message::RenameLayer { id, name } => {
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].name = name;
                // Saisie clavier : coalescé pour ne pas spammer l'historique
                app.history.push_coalesced(coalesce_key(id, 0), pre);
            }
        }
        Message::SetLayerOffset { id, axis, value } => {
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                match axis {
                    OffsetAxis::X => app.layers[i].offset_x = value,
                    OffsetAxis::Y => app.layers[i].offset_y = value,
                }
                app.history.push_coalesced(coalesce_key(id, 2), pre);
                app.refresh_fallback();
            }
        }
        Message::SetLayerRotation { id, degrees } => {
            // Rotation au draw (GPU) — zéro travail sur les pixels
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].rotation = degrees.clamp(-360.0, 360.0);
                app.history.push_coalesced(coalesce_key(id, 3), pre);
            }
        }
        Message::RotateLayer90 { id, clockwise } => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let delta = if clockwise { 90.0 } else { -90.0 };
                let pre = app.snapshot();
                // Normalise dans [-180, 180[ pour garder des valeurs lisibles
                let r = (app.layers[i].rotation + delta + 180.0).rem_euclid(360.0) - 180.0;
                app.layers[i].rotation = r;
                app.history.push(pre);
            }
        }
        Message::FlipLayer { id, horizontal } => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let pre = app.snapshot();
                app.layers[i].flip(horizontal);
                app.history.push(pre);
            }
        }
        Message::RotateLayer { id, delta } => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let pre = app.snapshot();
                let r = (app.layers[i].rotation + delta + 180.0).rem_euclid(360.0) - 180.0;
                // -180 et 180 sont équivalents, on garde 180 pour lisibilité
                app.layers[i].rotation = if r == -180.0 { 180.0 } else { r };
                app.history.push(pre);
            }
        }
        Message::SetLayerScale { id, scale } => {
            if let Some(i) = app.layer_index(id) {
                let pre = app.snapshot();
                app.layers[i].scale = scale.clamp(0.05, 8.0);
                app.history.push_coalesced(coalesce_key(id, 4), pre);
            }
        }
        Message::ResetLayerTransform(id) => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(tid) = target
                && let Some(i) = app.layer_index(tid)
            {
                let pre = app.snapshot();
                app.layers[i].rotation = 0.0;
                app.layers[i].scale = 1.0;
                app.history.push(pre);
            }
        }
        Message::CropLayerToSelection => {
            // Garde-fous explicites (démarche incrémentale) : un calque
            // sélectionné, une sélection rectangulaire, transform neutre
            let Some(id) = app.selected_layer else {
                app.image_error = Some("Rogner : aucun calque sélectionné".into());
                return Task::none();
            };
            let Some(sel) = app.canvas_selection else {
                app.image_error = Some(
                    "Rogner : faites d'abord une sélection rectangulaire (outil Sélect)".into(),
                );
                return Task::none();
            };
            let Some(i) = app.layer_index(id) else {
                return Task::none();
            };
            if app.layers[i].rotation.abs() > 0.01 || (app.layers[i].scale - 1.0).abs() > 0.01 {
                app.image_error =
                    Some("Rogner : réinitialisez d'abord rotation/échelle du calque".into());
                return Task::none();
            }
            // Écran → monde → coordonnées calque
            let zoom = (app.zoom_level as f32 / 100.0).max(0.001);
            let (doc_w, doc_h) = app.doc_size.unwrap_or((800, 600));
            let to_layer = |sx: f32, sy: f32| {
                let wx = (sx - app.canvas_viewport.width / 2.0 - app.canvas_pan.x) / zoom
                    + doc_w as f32 / 2.0;
                let wy = (sy - app.canvas_viewport.height / 2.0 - app.canvas_pan.y) / zoom
                    + doc_h as f32 / 2.0;
                (wx - app.layers[i].offset_x, wy - app.layers[i].offset_y)
            };
            let (x0, y0) = to_layer(sel.x, sel.y);
            let (x1, y1) = to_layer(sel.x + sel.width, sel.y + sel.height);
            let cx0 = x0.min(x1).round() as i32;
            let cy0 = y0.min(y1).round() as i32;
            let cw = ((x1 - x0).abs().round() as u32).max(1);
            let ch = ((y1 - y0).abs().round() as u32).max(1);
            let pre = app.snapshot();
            match app.layers[i].crop(cx0, cy0, cw, ch) {
                Ok(()) => {
                    app.history.push(pre);
                    app.image_error = None;
                    app.refresh_fallback();
                }
                Err(e) => app.image_error = Some(e),
            }
        }
        Message::AddEmptyLayer => {
            let (w, h) = app.doc_size.unwrap_or((800, 600));
            let transparent = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
                w.max(1),
                h.max(1),
                ::image::Rgba([0, 0, 0, 0]),
            ));
            let id = app.alloc_layer_id();
            let idx = app.selected_layer.and_then(|s| app.layer_index(s));
            let layer = Layer::new(id, format!("Calque {}", id + 1), Arc::new(transparent));
            let pre = app.snapshot();
            // Insère AU-DESSUS du calque sélectionné (sinon tout en haut)
            match idx {
                Some(i) => app.layers.insert(i + 1, layer),
                None => app.layers.push(layer),
            }
            app.selected_layer = Some(id);
            app.history.push(pre);
            app.refresh_fallback();
        }
        Message::DuplicateLayer(id) => {
            let src = id;
            let src = if app.layer_index(src).is_some() {
                src
            } else {
                app.selected_layer.unwrap_or(src)
            };
            if let Some(i) = app.layer_index(src) {
                let mut copy = Layer::new(
                    app.alloc_layer_id(),
                    format!("{} copie", app.layers[i].name),
                    app.layers[i].image.clone(),
                );
                copy.opacity = app.layers[i].opacity;
                copy.blend_mode = app.layers[i].blend_mode.clone();
                copy.visible = app.layers[i].visible;
                copy.offset_x = app.layers[i].offset_x;
                copy.offset_y = app.layers[i].offset_y;
                copy.rotation = app.layers[i].rotation;
                copy.scale = app.layers[i].scale;
                let pre = app.snapshot();
                app.layers.insert(i + 1, copy);
                app.selected_layer = Some(app.layers[i + 1].id);
                app.history.push(pre);
                app.refresh_fallback();
            }
        }
        Message::DeleteLayer(id) => {
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(t) = target
                && app.layers.len() > 1
                && let Some(i) = app.layer_index(t)
            {
                let pre = app.snapshot();
                app.layers.remove(i);
                app.selected_layer = app
                    .layers
                    .get(i.min(app.layers.len() - 1))
                    .or_else(|| app.layers.last())
                    .map(|l| l.id);
                app.history.push(pre);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerUp(id) => {
            if let Some(i) = app.layer_index(id)
                && i + 1 < app.layers.len()
            {
                let pre = app.snapshot();
                app.layers.swap(i, i + 1);
                app.history.push(pre);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerDown(id) => {
            if let Some(i) = app.layer_index(id)
                && i > 0
            {
                let pre = app.snapshot();
                app.layers.swap(i, i - 1);
                app.history.push(pre);
                app.refresh_fallback();
            }
        }

        Message::SelectTool(tool) => {
            app.selected_tool = tool;
            app.canvas_selection = None;
            app.move_anchor = None;
        }
        Message::ToggleToolsPanel => {
            app.tools_visible = !app.tools_visible;
        }
        Message::CanvasFit => {
            // Zoom pour voir toute l'image, centrée (pan nul)
            if let Some((iw, ih)) = app.doc_size.map(|(w, h)| (w as f32, h as f32)) {
                let vw = app.canvas_viewport.width.max(1.0);
                let vh = app.canvas_viewport.height.max(1.0);
                let fit = (vw / iw).min(vh / ih) * 0.95; // 5% de marge
                let zoom = fit.clamp(0.08, 6.0);
                app.zoom_level = (zoom * 100.0).round() as u32;
                app.canvas_pan = Vector::new(0.0, 0.0);
                app.canvas_selection = None;
            }
        }
        Message::ImageCanvasEvent(evt) => match evt {
            ui::image_canvas::ImageCanvasEvent::BrushStart { x, y, erase } => {
                return dispatch(app, Message::BrushStart { x, y, erase });
            }
            ui::image_canvas::ImageCanvasEvent::BrushEnd { points, tex, erase } => {
                return dispatch(app, Message::BrushEnd { points, tex, erase });
            }
            ui::image_canvas::ImageCanvasEvent::Viewport(size) => {
                app.canvas_viewport = size;
            }
            ui::image_canvas::ImageCanvasEvent::Pan(pan) => {
                if app.selected_tool == crate::message::Tool::Hand {
                    app.canvas_pan = pan;
                }
            }
            ui::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
                if app.selected_tool == crate::message::Tool::Select
                    || app.selected_tool == crate::message::Tool::Zoom
                {
                    if app.selected_tool == crate::message::Tool::Zoom {
                        // Zoom sur la zone sélectionnée
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
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerStart => {
                if app.selected_tool == crate::message::Tool::Move {
                    // Lit l'ancre avant toute mutation (règle own-borrow-over-clone)
                    let anchor = app
                        .selected_layer_mut()
                        .map(|l| (l.id, l.offset_x, l.offset_y));
                    if let Some((id, ox, oy)) = anchor {
                        app.move_anchor = Some((id, ox, oy));
                        // Un seul point de restauration pour TOUT le geste
                        app.history.push(app.snapshot());
                        // Fallback (fusion non-Normal) : pré-calcule UNE FOIS le
                        // fond sans le calque déplacé — coût unique au début du
                        // drag, ensuite zéro recomposite pendant tout le geste
                        if app.needs_fallback() {
                            app.prepare_drag_background(id);
                        }
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayer { dx, dy } => {
                if app.selected_tool == crate::message::Tool::Move
                    && let Some((id, ax, ay)) = app.move_anchor
                    && Some(id) == app.selected_layer
                {
                    let zoom = app.zoom_level as f32 / 100.0;
                    if zoom > 0.001 {
                        // ZÉRO recomposite dans les deux chemins :
                        // - rapide : le canvas redessine la texture à sa
                        //   nouvelle position (modèle Affinity)
                        // - fallback : fond pré-calculé + calque dessiné
                        //   par-dessus (approximation Normal pendant le geste)
                        let new_x = ax + dx / zoom;
                        let new_y = ay + dy / zoom;
                        if let Some(i) = app.layer_index(id) {
                            app.layers[i].offset_x = new_x;
                            app.layers[i].offset_y = new_y;
                        }
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerEnd => {
                app.move_anchor = None;
                // Vrai recomposite : le blend réel du calque à sa position
                // finale remplace l'approximation du drag
                let was_fallback = app.needs_fallback();
                app.drag_background = None;
                app.drag_background_size = None;
                if was_fallback {
                    app.refresh_fallback();
                }
            }
        },
        Message::Quit => {
            std::process::exit(0);
        }
        Message::Undo | Message::Redo => {
            let current = app.snapshot();
            let restored = if matches!(message, Message::Undo) {
                app.history.undo(current)
            } else {
                app.history.redo(current)
            };
            if let Some(snap) = restored {
                app.doc_size = snap.doc_size;
                app.layers = snap.layers;
                // La sélection peut pointer un calque disparu : on borne.
                if let Some(sel) = app.selected_layer
                    && app.layer_index(sel).is_none()
                {
                    app.selected_layer = app.layers.last().map(|l| l.id);
                }
                app.move_anchor = None;
                app.drag_background = None;
                app.drag_background_size = None;
                app.pending_paint = None;
                app.stroke_layer = None;
                app.refresh_fallback();
            }
        }
        Message::ZoomInPressed => {
            app.zoom_level = (app.zoom_level + 10).clamp(5, 1600);
        }
        Message::ZoomOutPressed => {
            app.zoom_level = app.zoom_level.saturating_sub(10).max(5);
        }
        Message::MockAction => {
            // Keep silent to avoid spam from subscription
        }

        Message::TogglePanel(panel_type) => {
            let existing_pane = app
                .panes
                .iter()
                .find(|(_, p)| **p == panel_type)
                .map(|(pane, _)| *pane);

            if let Some(pane) = existing_pane {
                app.panes.close(pane);
            } else {
                let target_canvas_pane = app
                    .panes
                    .iter()
                    .find(|(_, p)| **p == PanelType::Canvas)
                    .map(|(p, _)| *p);

                if let Some(canvas_pane) = target_canvas_pane {
                    let axis = match panel_type {
                        PanelType::Generator => pane_grid::Axis::Horizontal,
                        _ => pane_grid::Axis::Vertical,
                    };
                    app.panes.split(axis, canvas_pane, panel_type);
                }
            }
        }
        Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
            app.panes.resize(split, ratio);
        }
        Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
            app.panes.drop(pane, target);
        }
        Message::PaneDragged(_) => {}
        Message::PaneClicked(pane) => {
            app.focus = Some(pane);
        }
        Message::ClosePane(pane) => {
            app.panes.close(pane);
        }
        Message::CloseNodeContextMenu => {
            app.node_context_menu = None;
        }
        // ---- Générateur de textures (graphe nodal, usage futur filtres/génération) ----
        Message::NodeGraphEvent(evt) => match evt {
            ui::node_graph::NodeGraphEvent::NodeSelected(id) => {
                app.gen_selected_node = Some(id);
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::NodeMoved { id, position } => {
                app.gen_graph.move_node(id, position);
            }
            ui::node_graph::NodeGraphEvent::BackgroundClicked => {
                app.gen_selected_node = None;
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::RequestContextMenu(world, local) => {
                app.pending_connect = None;
                app.node_context_menu = Some(local);
                app.node_context_world = Some(world);
            }
            ui::node_graph::NodeGraphEvent::Connect {
                from,
                from_socket,
                to,
                to_socket,
            } => {
                let existing = app
                    .gen_graph
                    .connections
                    .iter()
                    .find(|c| c.to_node == to && c.to_socket == to_socket)
                    .cloned();
                if let Some(conn) = existing {
                    app.gen_graph.disconnect(&conn);
                }
                let from_ty = if from_socket == "factor" || to_socket == "factor" {
                    datatypes::SocketType::Float
                } else {
                    datatypes::SocketType::Image
                };
                let _ = app.gen_graph.connect(suite_core::Connection::new(
                    from,
                    from_socket.clone(),
                    to,
                    to_socket.clone(),
                    from_ty,
                ));
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::Disconnect { node, socket } => {
                app.gen_graph.disconnect_input(node, &socket);
            }
            // Événements non utilisés par le générateur (ignorés silencieusement)
            _ => {}
        },
        Message::UpdateParam { node, key, value } => {
            app.gen_graph.update_param(node, key.clone(), value.clone());
        }
        Message::AddNodeAt { type_id, world_pos } => {
            let pos = datatypes::Vec2::new(
                world_pos.x.clamp(-2000.0, 3000.0),
                world_pos.y.clamp(-2000.0, 3000.0),
            );
            if let Some(id) =
                components::node_registry::create_node_for_type(&type_id, pos, &mut app.gen_graph)
            {
                app.gen_selected_node = Some(id);
            }
            app.node_context_menu = None;
            app.node_context_world = None;
        }
        Message::DeleteSelectedNode => {
            if let Some(id) = app.gen_selected_node {
                app.gen_graph.remove_node(id);
                app.gen_selected_node = None;
            }
        }

        Message::DetectGpu => {
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::GpuDetected(info) => {
            app.gpu_info = Some(info);
            app.gpu_available = true;
        }
    }
    Task::none()
}

/// Clé de coalescence historique : (calque, paramètre).
fn coalesce_key(layer_id: u64, param: u64) -> u64 {
    layer_id.wrapping_mul(16).wrapping_add(param)
}

fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("projet")
        .to_string()
}

/// Boîte d'ouverture : projets .csphoto ET images brutes.
fn open_document_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("Projet CreativeSuite", &["csphoto"])
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

/// Boîte « Enregistrer sous » (.csphoto).
fn save_as_dialog_task() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("Projet CreativeSuite", &["csphoto"])
                .set_title("Enregistrer le projet")
                .set_file_name("sans-titre.csphoto")
                .save_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        Message::SaveProjectPathPicked,
    )
}

/// Résultat d'une lecture de fichier brut (octets + nom).
type FileRead = Result<(Vec<u8>, String), String>;

/// Lecture générique d'un fichier hors thread UI.
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

fn save_project_task(
    app: &mut PhotoApp,
    path: std::path::PathBuf,
    w: u32,
    h: u32,
) -> Task<Message> {
    let layers = app.layers.clone();
    let name = file_label(&path);
    app.background_tasks
        .retain(|t| !t.starts_with("Enregistrement"));
    app.background_tasks
        .push(format!("Enregistrement de {name}"));
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || photo_engine::project::save(&path, &layers, w, h))
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))??;
            Ok(name)
        },
        Message::ProjectSaved,
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
