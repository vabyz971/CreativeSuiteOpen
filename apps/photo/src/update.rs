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
//! Toutes les mutations de calques passent par les méthodes du `Document`
//! moteur (arbre LayerTree) — l'app n'écrit jamais dans les champs directement.

use std::sync::Arc;

use iced::widget::pane_grid;
use iced::{Size, Task, Vector};
use uuid::Uuid;

use crate::components;
use crate::layers::{LayerNode, PixelLayer, Transform2D};
use crate::message::{DecodedLayer, Message, OffsetAxis, PanelType};
use crate::state::PhotoApp;
use photo_engine::{Command, UndoAction};

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
    match message {
        Message::NewProject => {
            app.doc = photo_engine::Document::new(0, 0);
            app.selected_layer = None;
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
                if photo_engine::project::is_project_path(&path) {
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
        }
        Message::ProjectOpened(Err(e)) => {
            app.background_tasks.clear();
            app.image_error = Some(e);
        }
        Message::SaveProject => {
            if app.background_tasks.is_empty() && app.doc_dims().is_some() {
                match app.project_path.clone() {
                    Some(path) => return save_project_task(app, path),
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
        Message::ExportImage => {
            if app.background_tasks.is_empty() && app.doc_dims().is_some() {
                return export_dialog_task();
            }
        }
        Message::ExportPathPicked(path_opt) => {
            if let Some(path) = path_opt
                && app.background_tasks.is_empty()
                && app.doc_dims().is_some()
            {
                return export_image_task(app, path);
            }
        }
        Message::ImageExported(Ok(name)) => {
            app.background_tasks.clear();
            app.image_error = None;
            // Confirmation discrète via la zone d'erreur (vert côté UI future)
            let _ = name;
        }
        Message::ImageExported(Err(e)) => {
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
            return Task::perform(
                async move {
                    match ::image::load_from_memory(&bytes) {
                        Ok(dyn_img) => Ok(DecodedLayer(PixelLayer::new(name, Arc::new(dyn_img)))),
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
            let node = LayerNode::Pixel(decoded.0);
            // Le document prend les dimensions de la première image
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
            // Masqué → aucun dessin/gomme possible
            if app.pending_paint.is_none()
                && let Some(id) = app.selected_layer
                && app.doc.pixel_layer(id).is_some()
                && app.doc.find(id).map(|n| n.visible()).unwrap_or(false)
            {
                app.stroke_layer = Some(id);
            }
        }
        Message::BrushEnd { points, tex, erase } => {
            // Le travail lourd (copie RGBA, rastérisation, aperçu, miniature)
            // part sur un thread de fond (spawn_blocking) : l'UI reste fluide.
            // L'aperçu rastérisé au relâchement reste affiché tel quel
            // (pending_paint) jusqu'à PaintApplied — zéro clignotement.
            let stroke_target = app.stroke_layer.take();
            if let Some(id) = stroke_target
                && app.pending_paint.is_none()
                && points.len() > 1
                && let Some(layer) = app.doc.pixel_layer(id)
                && let Some(tex) = tex
            {
                let source = Arc::clone(&layer.source_image);
                let transform = layer.transform;
                let pts = points;
                app.pending_paint = Some(crate::message::PendingPaint {
                    layer_id: id,
                    tex: tex.clone(),
                });
                // État PRÉ-trait figé dans l'historique maintenant : le
                // snapshot partage les pixels via Arc, coût quasi nul.
                app.history.push_snapshot(app.snapshot());
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
            if let Some(img) = ::image::RgbaImage::from_raw(buf.width, buf.height, buf.rgba) {
                app.doc
                    .set_source_image(layer_id, ::image::DynamicImage::ImageRgba8(img));
            }
            app.pending_paint = None;
            app.invalidate_fallback();
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
        }

        Message::OpenPreferences => {
            // Déjà ouverte : on lui rend le focus (comportement pro)
            if let Some(id) = app.preferences_window_id {
                return iced::window::gain_focus(id);
            }
            app.preferences_window = Some(crate::preferences_window::PreferencesWindow::new(
                app.preferences.clone(),
            ));
            let (_, open) = iced::window::open(iced::window::Settings {
                size: iced::Size::new(780.0, 580.0),
                min_size: Some(iced::Size::new(620.0, 460.0)),
                resizable: true,
                exit_on_close_request: false,
                ..iced::window::Settings::default()
            });
            // Détection matérielle HORS thread UI pour la section Hardware
            let detect = Task::perform(
                async { preferences::HardwareReport::detect().await },
                Message::HardwareDetected,
            );
            return Task::batch([open.map(Message::WindowOpened), detect]);
        }
        Message::WindowOpened(id) => {
            app.preferences_window_id = Some(id);
        }
        Message::WindowClosed(id) => {
            // Fermeture par la croix OS : purge de l'état associé
            if app.is_preferences_window(id) {
                app.preferences_window = None;
                app.preferences_window_id = None;
            }
        }

        Message::Event { event, window } => {
            // Les touches pressées dans la fenêtre préférences ne doivent
            // JAMAIS atteindre les raccourcis du document.
            if app.is_preferences_window(window) {
                if let Some(w) = &mut app.preferences_window
                    && let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key,
                        modifiers,
                        ..
                    }) = event
                {
                    w.key_event(key, modifiers);
                }
                return Task::none();
            }
            // Résolution globale : l'abonnement ne livre que les touches
            // NON consommées par un widget (les champs texte sont donc sûrs)
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) = event
                && let Some(action) = app.resolver.resolve(&key, modifiers)
            {
                return dispatch(app, Message::ExecuteAction(action));
            }
        }

        Message::ExecuteAction(action) => {
            // Pont unique action typée → messages existants (réutilisation
            // intégrale des handlers, zéro duplication de logique)
            use preferences::PhotoAction;
            let target = || app.selected_layer.unwrap_or_else(uuid::Uuid::nil);
            let msg = match action {
                PhotoAction::ToolBrush => Message::SelectTool(crate::message::Tool::Brush),
                PhotoAction::ToolEraser => Message::SelectTool(crate::message::Tool::Eraser),
                PhotoAction::ToolEyedropper => Message::SelectTool(crate::message::Tool::Select),
                PhotoAction::ToolMove => Message::SelectTool(crate::message::Tool::Move),
                PhotoAction::ToolHand => Message::SelectTool(crate::message::Tool::Hand),
                PhotoAction::ToolZoom => Message::SelectTool(crate::message::Tool::Zoom),
                PhotoAction::Undo => Message::Undo,
                PhotoAction::Redo => Message::Redo,
                PhotoAction::DeleteLayer => Message::DeleteLayer(target()),
                PhotoAction::NewProject => Message::NewProject,
                PhotoAction::Open => Message::OpenProject,
                PhotoAction::Save => Message::SaveProject,
                PhotoAction::SaveAs => Message::SaveProjectAs,
                PhotoAction::Export => Message::ExportImage,
                PhotoAction::ZoomIn => Message::ZoomInPressed,
                PhotoAction::ZoomOut => Message::ZoomOutPressed,
                PhotoAction::ZoomFit => Message::CanvasFit,
                PhotoAction::Zoom100 => {
                    app.zoom_level = 100;
                    app.canvas_pan = Vector::new(0.0, 0.0);
                    app.canvas_selection = None;
                    return Task::none();
                }
                PhotoAction::ToggleLayersPanel => Message::TogglePanel(PanelType::Layers),
                PhotoAction::ToggleToolsPanel => Message::ToggleToolsPanel,
                PhotoAction::NewLayer => Message::AddEmptyLayer,
                PhotoAction::DuplicateLayer => Message::DuplicateLayer(target()),
                PhotoAction::OpenPreferences => Message::OpenPreferences,
            };
            return dispatch(app, msg);
        }

        Message::PreferencesMsg(msg) => {
            use crate::preferences_window::Message as PrefsMsg;
            match msg {
                PrefsMsg::Close | PrefsMsg::SaveAndClose => {
                    if let Some(window) = &mut app.preferences_window {
                        window.update(msg.clone());
                        if matches!(msg, PrefsMsg::SaveAndClose) {
                            app.preferences = window.draft.clone();
                            app.resolver = preferences::KeybindingResolver::from_bindings(
                                &window.draft.keybindings.bindings,
                            );
                        }
                    }
                    return app.close_preferences_window();
                }
                inner => {
                    if let Some(window) = &mut app.preferences_window {
                        window.update(inner);
                    }
                }
            }
        }

        Message::HardwareDetected(report) => {
            if let Some(window) = &mut app.preferences_window {
                window.set_hardware(report);
            }
        }

        Message::TickFrame => {
            // Animation du spinner (~30 fps)
            app.spinner_angle = (app.spinner_angle + 24.0) % 360.0;
        }

        // ---- Calques (arbre LayerTree) ----
        Message::SelectLayer(id) => {
            // Masqué → non sélectionnable (ni dessin/gomme/déplacement)
            if app.doc.find(id).map(|n| !n.visible()).unwrap_or(true) {
                return Task::none();
            }
            app.selected_layer = Some(id);
            app.move_anchor = None;
        }
        Message::ToggleLayerVisible(id) => {
            // Micro-édition : commande légère, zéro clonage d'arbre
            if let Some(node) = app.doc.find(id) {
                let new_visible = !node.visible();
                let cmd = Command::SetVisibility {
                    node_id: id,
                    old: node.visible(),
                    new: new_visible,
                };
                app.history.push_command_immediate(cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
                // Si on vient de masquer le calque sélectionné, on le désélectionne
                if !new_visible && app.selected_layer == Some(id) {
                    app.selected_layer = None;
                    app.move_anchor = None;
                    app.stroke_layer = None;
                }
            }
        }
        Message::SetLayerOpacity { id, opacity } => {
            // Simple changement d'état appliqué AU DRAW — la commande ne
            // coûte que quelques octets : plus aucun clonage de l'arbre
            if let Some(node) = app.doc.find(id) {
                let cmd = Command::SetOpacity {
                    layer_id: id,
                    old: node.opacity(),
                    new: opacity,
                };
                // Geste continu (slider) : une seule entrée fusionnée
                app.history.push_command(coalesce_key(id, 1), cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                // L'opacité est CUITE dans la composite fallback → à chaque
                // tick le dirty-flag redéclenchera un calcul async (coalescé
                // par le mécanisme in-flight, jamais bloquant)
                app.invalidate_fallback();
            }
        }
        Message::SetLayerBlend { id, mode } => {
            if let Some(node) = app.doc.find(id)
                && let Some(old) = node.blend_mode()
            {
                let cmd = Command::SetBlendMode {
                    node_id: id,
                    old,
                    new: mode,
                };
                app.history.push_command_immediate(cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                // Bascule chemin rapide ↔ fallback selon le mode
                app.invalidate_fallback();
            }
        }
        Message::RenameLayer { id, name } => {
            // Saisie clavier : coalescée pour ne pas spammer l'historique
            if let Some(node) = app.doc.find(id) {
                let cmd = Command::RenameLayer {
                    node_id: id,
                    old: node.name().to_string(),
                    new: name,
                };
                app.history.push_command(coalesce_key(id, 0), cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
            }
        }
        Message::SetLayerOffset { id, axis, value } => {
            if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
                let mut new_t = l.transform;
                match axis {
                    OffsetAxis::X => new_t.offset_x = value,
                    OffsetAxis::Y => new_t.offset_y = value,
                }
                let cmd = Command::SetTransform {
                    layer_id: id,
                    old: l.transform,
                    new: new_t,
                };
                app.history.push_command(coalesce_key(id, 2), cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                // Le composite fallback cuit les offsets → recomposite
                app.invalidate_fallback();
            }
        }
        Message::SetLayerRotation { id, degrees } => {
            // Rotation au draw (GPU) — zéro travail sur les pixels
            if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
                let cmd = Command::SetTransform {
                    layer_id: id,
                    old: l.transform,
                    new: Transform2D {
                        rotation_deg: degrees.clamp(-360.0, 360.0),
                        ..l.transform
                    },
                };
                app.history.push_command(coalesce_key(id, 3), cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
            }
        }
        Message::RotateLayer90 { id, clockwise } => {
            let target = resolve_target(app, id);
            let delta = if clockwise { 90.0 } else { -90.0 };
            if let Some(tid) = target
                && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
            {
                // Normalise dans [-180, 180[ pour garder des valeurs lisibles
                let r = (l.transform.rotation_deg + delta + 180.0).rem_euclid(360.0) - 180.0;
                let cmd = Command::SetTransform {
                    layer_id: tid,
                    old: l.transform,
                    new: Transform2D {
                        rotation_deg: r,
                        ..l.transform
                    },
                };
                app.history.push_command_immediate(cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
            }
        }
        Message::FlipLayer { id, horizontal } => {
            let target = resolve_target(app, id);
            if let Some(tid) = target {
                let pre = app.snapshot();
                match app.doc.flip(tid, horizontal) {
                    Ok(()) => app.history.push_snapshot(pre),
                    Err(e) => app.image_error = Some(e),
                }
                app.invalidate_fallback();
            }
        }
        Message::RotateLayer { id, delta } => {
            let target = resolve_target(app, id);
            if let Some(tid) = target
                && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
            {
                let r = (l.transform.rotation_deg + delta + 180.0).rem_euclid(360.0) - 180.0;
                // -180 et 180 sont équivalents, on garde 180 pour lisibilité
                let new_rot = if r == -180.0 { 180.0 } else { r };
                let cmd = Command::SetTransform {
                    layer_id: tid,
                    old: l.transform,
                    new: Transform2D {
                        rotation_deg: new_rot,
                        ..l.transform
                    },
                };
                app.history.push_command_immediate(cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
            }
        }
        Message::SetLayerScale { id, scale } => {
            if let Some(LayerNode::Pixel(l)) = app.doc.find(id) {
                let cmd = Command::SetTransform {
                    layer_id: id,
                    old: l.transform,
                    new: Transform2D {
                        scale: scale.clamp(0.05, 8.0),
                        ..l.transform
                    },
                };
                app.history.push_command(coalesce_key(id, 4), cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
            }
        }
        Message::ResetLayerTransform(id) => {
            let target = resolve_target(app, id);
            if let Some(tid) = target
                && let Some(LayerNode::Pixel(l)) = app.doc.find(tid)
            {
                let cmd = Command::SetTransform {
                    layer_id: tid,
                    old: l.transform,
                    new: Transform2D {
                        rotation_deg: 0.0,
                        scale: 1.0,
                        ..l.transform
                    },
                };
                app.history.push_command_immediate(cmd.clone());
                let _inverse = app.doc.apply_command(cmd);
                app.invalidate_fallback();
            }
        }
        Message::CropLayerToSelection => {
            crop_layer_to_selection(app);
        }
        Message::AddEmptyLayer => {
            add_empty_layer(app);
        }
        Message::AddSolidColorLayer => {
            add_solid_color_layer(app, app.brush_color);
        }
        Message::SetDraggedLayer(id) => {
            app.dragged_layer = Some(id);
        }
        Message::DropLayerOn(target) => {
            if let Some(dragged) = app.dragged_layer.take() {
                if dragged != target
                    && app.doc.find(dragged).is_some()
                    && app.doc.find(target).is_some()
                {
                    let pre = app.snapshot();
                    if app.doc.reorder_before(dragged, target, true) {
                        app.history.push_snapshot(pre);
                        app.invalidate_fallback();
                    }
                }
            } else {
                app.dragged_layer = None;
            }
        }
        Message::SetResizeWidth(s) => {
            app.resize_w = s;
        }
        Message::SetResizeHeight(s) => {
            app.resize_h = s;
        }
        Message::ShowResizeDialog => {
            app.resize_dialog_open = !app.resize_dialog_open;
            if app.resize_dialog_open {
                let (w, h) = app.doc_dims().unwrap_or((800, 600));
                app.resize_w = w.to_string();
                app.resize_h = h.to_string();
            }
        }
        Message::ResizeDocument { width, height } => {
            let w = width.max(1);
            let h = height.max(1);
            let pre = app.snapshot();
            app.doc.width = w;
            app.doc.height = h;
            app.resize_dialog_open = false;
            app.history.push_snapshot(pre);
            app.invalidate_fallback();
        }
        Message::ReorderLayer {
            dragged,
            target,
            before,
        } => {
            if app.doc.reorder_before(dragged, target, before) {
                let pre = app.snapshot();
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::DuplicateLayer(id) => {
            let target = resolve_target(app, id);
            if let Some(src) = target {
                let pre = app.snapshot();
                if let Some(new_id) = app.doc.duplicate(src) {
                    rename_duplicate_suffix(&mut app.doc, new_id);
                    app.selected_layer = Some(new_id);
                    app.history.push_snapshot(pre);
                    app.invalidate_fallback();
                }
            }
        }
        Message::DeleteLayer(id) => {
            let target = resolve_target(app, id);
            if let Some(t) = target
                && app.doc.pixel_count() > 1
            {
                let pre = app.snapshot();
                if app.doc.remove(t).is_some() {
                    // Réparation de sélection : dernier calque pixels restant
                    app.selected_layer = app.doc.iter_pixels().last().map(|l| l.id);
                    app.history.push_snapshot(pre);
                    app.invalidate_fallback();
                }
            }
        }
        Message::MoveLayerUp(id) => {
            if app.doc.move_up(id) {
                let pre = app.snapshot();
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::MoveLayerDown(id) => {
            if app.doc.move_down(id) {
                let pre = app.snapshot();
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::GroupLayers(id) => {
            let pre = app.snapshot();
            if let Some(gid) = app.doc.group(&[id]) {
                app.selected_layer = Some(gid);
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::UngroupLayers(id) => {
            let pre = app.snapshot();
            if let Some(freed) = app.doc.ungroup(id) {
                app.selected_layer = freed.first().copied();
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::ToggleGroupCollapsed(id) => {
            // État de vue du panneau : pas de point d'historique
            if let Some(LayerNode::Group(g)) = app.doc.find_mut(id) {
                g.collapsed = !g.collapsed;
            }
        }
        Message::AddLiveFilter { id, type_id } => {
            if let Some(filter) = photo_engine::new_filter(&type_id) {
                let pre = app.snapshot();
                if app.doc.add_filter(id, filter).is_some() {
                    app.history.push_snapshot(pre);
                    app.invalidate_fallback();
                }
            }
        }
        Message::RemoveLiveFilter {
            layer_id,
            filter_id,
        } => {
            let pre = app.snapshot();
            if app.doc.remove_filter(layer_id, filter_id).is_some() {
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
            }
        }
        Message::SetFilterParam {
            layer_id,
            filter_id,
            key,
            value,
        } => {
            // Micro-édition par excellence : commande légère coalescée.
            // Calque pixels : l'apparence se recalcule seule via le cache
            // de versions (zéro recomposite global). Ajustement : le blend
            // global change → recomposite.
            let old_value = app
                .doc
                .find(layer_id)
                .and_then(|n| n.filters())
                .and_then(|fs| fs.iter().find(|f| f.id == filter_id))
                .and_then(|f| f.params.get(&key))
                .cloned();
            match old_value {
                Some(old) => {
                    let cmd = Command::SetFilterParam {
                        layer_id,
                        filter_id,
                        param_name: key.clone(),
                        old,
                        new: value.clone(),
                    };
                    app.history
                        .push_command(coalesce_key(filter_id, 5), cmd.clone());
                    let _inverse = app.doc.apply_command(cmd);
                }
                None => {
                    // Paramètre inexistant (initialisation) : hors historique
                    app.doc.set_filter_param(layer_id, filter_id, key, value);
                }
            }
            // Cuit dans la composite fallback si elle est active ;
            // en chemin rapide c'est un simple flag sans coût.
            app.invalidate_fallback();
        }
        Message::ToggleFilterEnabled {
            layer_id,
            filter_id,
        } => {
            let pre = app.snapshot();
            if app.doc.set_filter_enabled(layer_id, filter_id, {
                // inverse l'état courant
                app.doc
                    .find(layer_id)
                    .and_then(|n| n.filters())
                    .and_then(|fs| fs.iter().find(|f| f.id == filter_id))
                    .map(|f| !f.enabled)
                    .unwrap_or(false)
            }) {
                app.history.push_snapshot(pre);
                app.invalidate_fallback();
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
            if let Some((iw, ih)) = app.doc_dims().map(|(w, h)| (w as f32, h as f32)) {
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
            ui_kit::image_canvas::ImageCanvasEvent::BrushStart { x, y, erase } => {
                return dispatch(app, Message::BrushStart { x, y, erase });
            }
            ui_kit::image_canvas::ImageCanvasEvent::BrushEnd { points, tex, erase } => {
                return dispatch(app, Message::BrushEnd { points, tex, erase });
            }
            ui_kit::image_canvas::ImageCanvasEvent::Viewport(size) => {
                app.canvas_viewport = size;
            }
            ui_kit::image_canvas::ImageCanvasEvent::Pan(pan) => {
                if app.selected_tool == crate::message::Tool::Hand {
                    app.canvas_pan = pan;
                }
            }
            ui_kit::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui_kit::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui_kit::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
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
            ui_kit::image_canvas::ImageCanvasEvent::MoveLayerStart => {
                if app.selected_tool == crate::message::Tool::Move {
                    // Masqué → déplacement interdit
                    if app
                        .selected_layer
                        .and_then(|id| app.doc.find(id))
                        .map(|n| !n.visible())
                        .unwrap_or(true)
                    {
                        return Task::none();
                    }
                    // Lit l'ancre avant toute mutation (règle own-borrow-over-clone).
                    // Transform COMPLET capturé : la commande SetTransform
                    // ancre→finale ne sera poussée QU'AU relâchement —
                    // zéro snapshot pendant le geste.
                    let anchor = app
                        .selected_layer
                        .and_then(|id| app.doc.pixel_layer(id))
                        .map(|l| (l.id, l.transform));
                    if let Some((id, anchor_t)) = anchor {
                        app.move_anchor = Some((id, anchor_t));
                        // Fallback (blending inter-calques) : demande le fond
                        // SANS ce sous-arbre en tâche de fond. Pendant les
                        // quelques ms de calcul, le drag s'affiche déjà en
                        // dessin calque-par-calque (approximation), puis le
                        // fond exact arrive sans jamais geler l'UI.
                        if app.needs_fallback()
                            && let Some(task) = app.drag_background_task(id)
                        {
                            return task;
                        }
                    }
                }
            }
            ui_kit::image_canvas::ImageCanvasEvent::MoveLayer { dx, dy } => {
                if app.selected_tool == crate::message::Tool::Move
                    && let Some((id, anchor_t)) = app.move_anchor
                    && Some(id) == app.selected_layer
                {
                    let zoom = app.zoom_level as f32 / 100.0;
                    if zoom > 0.001 {
                        // ZÉRO recomposite dans les deux chemins :
                        // - rapide : le canvas redessine la texture à sa
                        //   nouvelle position (modèle Affinity)
                        // - fallback : fond pré-calculé + calque dessiné
                        //   par-dessus (approximation Normal pendant le geste)
                        let new_x = anchor_t.offset_x + dx / zoom;
                        let new_y = anchor_t.offset_y + dy / zoom;
                        if let Some(LayerNode::Pixel(l)) = app.doc.find_mut(id) {
                            l.transform.offset_x = new_x;
                            l.transform.offset_y = new_y;
                        }
                    }
                }
            }
            ui_kit::image_canvas::ImageCanvasEvent::MoveLayerEnd => {
                app.drag_bg_in_flight = None;
                // Fin du geste : UNE commande légère ancre→final remplace
                // l'ancien snapshot de début de drag. Drag immobile = pas
                // d'entrée d'historique du tout.
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
                // Vrai recomposite : le blend réel du calque à sa position
                // finale remplace l'approximation du drag
                let was_fallback = app.needs_fallback();
                app.drag_background = None;
                app.drag_background_size = None;
                if was_fallback {
                    app.invalidate_fallback();
                }
            }
        },
        Message::Quit => {
            std::process::exit(0);
        }
        Message::Undo | Message::Redo => {
            // Historique hybride : l'historique applique LUI-MÊME l'inverse
            // (undo) ou la commande (redo) au document, puis décrit quoi
            // invalider — recomposite complet ou rien (le sync du cache de
            // textures UI cible déjà les calques réellement modifiés).
            let action = if matches!(message, Message::Undo) {
                app.history.undo(&mut app.doc)
            } else {
                app.history.redo(&mut app.doc)
            };
            match action {
                Some(UndoAction::FullRestore) => {
                    // Structure restaurée : la sélection peut pointer un
                    // nœud disparu, on borne.
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
                    // Invalidation ciblée : recomposite UNIQUEMENT si le
                    // blending global dépend du nœud touché
                    app.invalidate_fallback();
                }
                Some(UndoAction::Applied(_)) | None => {}
            }
        }
        Message::FallbackComputed { generation, result } => {
            app.fallback_in_flight = false;
            if generation != app.fallback_generation {
                // Résultat périmé : le document a changé pendant le calcul.
                // take_fallback_task réémettra car dirty est encore vrai.
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
        }
        Message::DragBackgroundComputed { layer_id, result } => {
            app.drag_bg_in_flight = None;
            // N'applique que si on drague TOUJOURS le même sous-arbre
            if app.move_anchor.map(|(id, _)| id) == Some(layer_id)
                && let Some((rgba, w, h)) = result
            {
                app.drag_background = Some(iced::widget::image::Handle::from_rgba(w, h, rgba));
                app.drag_background_size = Some(Size::new(w as f32, h as f32));
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
            ui_kit::node_graph::NodeGraphEvent::NodeSelected(id) => {
                app.gen_selected_node = Some(id);
                app.node_context_menu = None;
            }
            ui_kit::node_graph::NodeGraphEvent::NodeMoved { id, position } => {
                app.gen_graph.move_node(id, position);
            }
            ui_kit::node_graph::NodeGraphEvent::BackgroundClicked => {
                app.gen_selected_node = None;
                app.node_context_menu = None;
            }
            ui_kit::node_graph::NodeGraphEvent::RequestContextMenu(world, local) => {
                app.pending_connect = None;
                app.node_context_menu = Some(local);
                app.node_context_world = Some(world);
            }
            ui_kit::node_graph::NodeGraphEvent::Connect {
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
            ui_kit::node_graph::NodeGraphEvent::Disconnect { node, socket } => {
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

/// Résout la cible d'une action : l'id fourni s'il existe, sinon la
/// sélection courante (pattern des sentinelles héritées des menus).
fn resolve_target(app: &PhotoApp, id: Uuid) -> Option<Uuid> {
    if app.doc.find(id).is_some() {
        Some(id)
    } else {
        app.selected_layer
            .filter(|sel| app.doc.find(*sel).is_some())
    }
}

/// Dimensions du nœud pixels (helper local pour l'ouverture d'image).
fn node_dimensions(node: &LayerNode) -> (u32, u32) {
    match node {
        LayerNode::Pixel(l) => l.dimensions(),
        _ => (800, 600),
    }
}

/// Ajoute un calque transparent vide au-dessus de la sélection.
fn add_empty_layer(app: &mut PhotoApp) {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let transparent = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
        w.max(1),
        h.max(1),
        ::image::Rgba([0, 0, 0, 0]),
    ));
    let count = app.doc.pixel_count();
    let layer = PixelLayer::new(format!("Calque {}", count + 1), Arc::new(transparent));
    let new_id = layer.id;
    let node = LayerNode::Pixel(layer);
    let pre = app.snapshot();
    // Insère AU-DESSUS du calque sélectionné (sinon tout en haut)
    let inserted = match app.selected_layer {
        Some(sel) if app.doc.find(sel).is_some() => app.doc.insert_above(sel, node),
        _ => {
            app.doc.push_layer(node);
            true
        }
    };
    if inserted {
        app.selected_layer = Some(new_id);
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
}

/// Ajoute un calque couleur uni à partir de la couleur globale du toolbar.
fn add_solid_color_layer(app: &mut PhotoApp, color: iced::Color) {
    let (w, h) = app.doc_dims().unwrap_or((800, 600));
    let rgba = ::image::Rgba([
        (color.r * 255.0).clamp(0.0, 255.0) as u8,
        (color.g * 255.0).clamp(0.0, 255.0) as u8,
        (color.b * 255.0).clamp(0.0, 255.0) as u8,
        (color.a * 255.0).clamp(0.0, 255.0) as u8,
    ]);
    let solid = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
        w.max(1),
        h.max(1),
        rgba,
    ));
    let count = app.doc.pixel_count();
    let layer = PixelLayer::new(format!("Couleur {}", count + 1), Arc::new(solid));
    let new_id = layer.id;
    let node = LayerNode::Pixel(layer);
    let pre = app.snapshot();
    let inserted = match app.selected_layer {
        Some(sel) if app.doc.find(sel).is_some() => app.doc.insert_above(sel, node),
        _ => {
            app.doc.push_layer(node);
            true
        }
    };
    if inserted {
        app.selected_layer = Some(new_id);
        app.history.push_snapshot(pre);
        app.invalidate_fallback();
    }
}

/// Suffixe « copie » après duplication (le moteur clone à l'identique).
fn rename_duplicate_suffix(doc: &mut photo_engine::Document, id: Uuid) {
    if let Some(node) = doc.find_mut(id) {
        let name = format!("{} copie", node.name());
        node.set_name(name);
    }
}

/// Rogne le calque sélectionné à la sélection rectangulaire active.
fn crop_layer_to_selection(app: &mut PhotoApp) {
    // Garde-fous explicites : un calque sélectionné, une sélection
    // rectangulaire, transform neutre
    let Some(id) = app.selected_layer else {
        app.image_error = Some("Rogner : aucun calque sélectionné".into());
        return;
    };
    let Some(sel) = app.canvas_selection else {
        app.image_error =
            Some("Rogner : faites d'abord une sélection rectangulaire (outil Sélect)".into());
        return;
    };
    let Some(layer) = app.doc.pixel_layer(id) else {
        return;
    };
    let offset = (layer.transform.offset_x, layer.transform.offset_y);
    if layer.transform.rotation_deg.abs() > 0.01 || (layer.transform.scale - 1.0).abs() > 0.01 {
        app.image_error = Some("Rogner : réinitialisez d'abord rotation/échelle du calque".into());
        return;
    }
    // Écran → monde → coordonnées calque
    let zoom = (app.zoom_level as f32 / 100.0).max(0.001);
    let (doc_w, doc_h) = app.doc_dims().unwrap_or((800, 600));
    let to_layer = |sx: f32, sy: f32| {
        let wx =
            (sx - app.canvas_viewport.width / 2.0 - app.canvas_pan.x) / zoom + doc_w as f32 / 2.0;
        let wy =
            (sy - app.canvas_viewport.height / 2.0 - app.canvas_pan.y) / zoom + doc_h as f32 / 2.0;
        (wx - offset.0, wy - offset.1)
    };
    let (x0, y0) = to_layer(sel.x, sel.y);
    let (x1, y1) = to_layer(sel.x + sel.width, sel.y + sel.height);
    let cx0 = x0.min(x1).round() as i32;
    let cy0 = y0.min(y1).round() as i32;
    let cw = ((x1 - x0).abs().round() as u32).max(1);
    let ch = ((y1 - y0).abs().round() as u32).max(1);
    let pre = app.snapshot();
    match app.doc.crop(id, cx0, cy0, cw, ch) {
        Ok(()) => {
            app.history.push_snapshot(pre);
            app.image_error = None;
            app.invalidate_fallback();
        }
        Err(e) => app.image_error = Some(e),
    }
}

/// Clé de coalescence historique : (nœud, paramètre).
fn coalesce_key(node_id: Uuid, param: u64) -> u64 {
    (node_id.as_u128() as u64)
        .wrapping_mul(16)
        .wrapping_add(param)
}

fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("projet")
        .to_string()
}

/// Boîte d'ouverture : projets (.csophoto, et hérités .csphoto) ET images.
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

/// Boîte « Enregistrer sous » (.csophoto).
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

fn save_project_task(app: &mut PhotoApp, path: std::path::PathBuf) -> Task<Message> {
    // Copie structurelle bon marché pour le thread de fond (Arcs partagés)
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

/// Boîte « Exporter l'image » — PNG par défaut, JPEG si extension .jpg/.jpeg.
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

/// Export HORS thread UI : composite complète (plan infini rogné au
/// document) puis encodage selon l'extension choisie.
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
