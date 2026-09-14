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

//! App Photo eframe : documents multiples, channels moteur, boucle non bloquante.
//!
//! Chaque document ouvert possède SON worker `photo-engine` (thread
//! background) et son état UI. La boucle egui poll tous les channels
//! en non bloquant (`try_recv`), dessine le layout façon Affinity
//! (rail outils, menu, options, canvas à onglets, studio droit,
//! statut) et ne repeint que sur activité.

use crate::layout::draw_photo_layout;
use crate::ui::canvas::{PhotoBrushSettings, PhotoCanvasTool};
use crate::ui::dialogs::{ExportDialogState, NewDocumentDialogState};
use crate::ui::engine_bridge::{
    PhotoEngineCommand, PhotoEngineResponse, PreviewImage, spawn_photo_engine_worker,
};
use crate::ui::layers::{LayerRenameState, PhotoLayerInfo};
use crate::ui::modebar::PhotoEditMode;
use photo_engine::Document;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use ui_kit::utils::ReorderDragState;
use ui_kit::viewport::{ViewportState, ViewportTextureCache};
use uuid::Uuid;

/// État UI d'un document ouvert.
#[derive(Default)]
pub struct PhotoUiState {
    /// Snapshot pour le panneau (haut de pile d'abord).
    pub layers: Vec<PhotoLayerInfo>,
    /// Calque sélectionné (état UI pur).
    pub selected: Option<Uuid>,
    /// Outil canvas actif.
    pub tool: PhotoCanvasTool,
    /// Mode d'édition (Vector / Pixel / Layout, 2e rangée haute).
    pub edit_mode: PhotoEditMode,
    /// Zoom/pan du canvas.
    pub viewport: ViewportState,
    /// Texture du composite moteur.
    pub texture_cache: ViewportTextureCache,
    /// Dernier aperçu reçu (échantillonnage pipette).
    pub last_preview: Option<PreviewImage>,
    /// Trait de pinceau en cours (pixels image).
    pub stroke: Vec<egui::Vec2>,
    /// Réglages pinceau/gomme.
    pub brush: PhotoBrushSettings,
    /// État de drag du panneau calques.
    pub drag_state: ReorderDragState,
    /// Renommage inline d'un calque (double-clic).
    pub rename: LayerRenameState,
    /// Profondeurs d'historique (boutons undo/redo).
    pub can_undo: bool,
    /// Redo disponible.
    pub can_redo: bool,
    /// Message de statut.
    pub status: String,
    /// Grille du canvas.
    pub show_grid: bool,
    /// Repaint explicite demandé.
    pub needs_repaint: bool,
}

/// Un document ouvert : worker moteur + état UI.
pub struct OpenDocument {
    /// Titre de l'onglet.
    pub title: String,
    /// État UI du document.
    pub ui: PhotoUiState,
    /// Commandes vers le worker.
    pub tx: Sender<PhotoEngineCommand>,
    /// Réponses du worker (poll non bloquant).
    pub rx: Receiver<PhotoEngineResponse>,
    // NOTE : le JoinHandle du worker n'est pas conservé : le thread
    // se termine seul quand le `Sender` est lâché (fin du `recv`).
}

/// Modale d'ajout de filtre (sélection dans le registre moteur).
#[derive(Default)]
pub struct FilterModalState {
    /// Modale visible.
    pub open: bool,
    /// Index dans la liste des filtres.
    pub choice: usize,
}

/// App Photo : documents à onglets + état global.
pub struct PhotoApp {
    /// Documents ouverts (toujours au moins un).
    pub docs: Vec<OpenDocument>,
    /// Index du document actif.
    pub active: usize,
    /// Section propriétés dépliée.
    pub props_open: bool,
    /// Modale d'ajout de filtre.
    pub filter_modal: FilterModalState,
    /// Catalogue des filtres (registre moteur, statique).
    pub filter_types: Vec<(String, String)>,
    /// File picker d'ouverture en cours (non bloquant).
    pub open_picker: Option<Receiver<Option<PathBuf>>>,
    /// Fenêtre « Nouveau document » (format, dimensions, orientation).
    pub new_doc_dialog: NewDocumentDialogState,
    /// Fenêtre « Exportation » (dossier, format, validation).
    pub export_dialog: ExportDialogState,
    /// Fenêtre d'aide visible.
    pub help_open: bool,
    /// Compteur « Sans titre ».
    pub untitled_counter: usize,
}

impl PhotoApp {
    /// Crée l'app avec un document vide.
    pub fn new() -> Self {
        let filter_types = photo_engine::filterable_types()
            .iter()
            .map(|definition| (definition.name.clone(), definition.type_id.clone()))
            .collect();
        let mut app = Self {
            docs: Vec::new(),
            active: 0,
            props_open: true,
            filter_modal: FilterModalState::default(),
            filter_types,
            open_picker: None,
            new_doc_dialog: NewDocumentDialogState::default(),
            export_dialog: ExportDialogState::default(),
            help_open: false,
            untitled_counter: 0,
        };
        app.open_blank_tab();
        app
    }

    /// Nombre de documents ouverts.
    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    /// Document actif (toujours présent : jamais zéro document).
    pub fn active_doc(&self) -> &OpenDocument {
        &self.docs[self.active.min(self.docs.len().saturating_sub(1))]
    }

    /// Document actif mutable.
    pub fn active_doc_mut(&mut self) -> &mut OpenDocument {
        let index = self.active.min(self.docs.len().saturating_sub(1));
        &mut self.docs[index]
    }

    /// Ouvre un onglet sur un document neuf et demande son snapshot.
    pub fn open_blank_tab(&mut self) {
        self.untitled_counter += 1;
        let title = format!("Sans titre {}", self.untitled_counter);
        self.spawn_document(title, Document::new(1920, 1080));
    }

    /// Ouvre un onglet aux dimensions validées (fenêtre Nouveau doc).
    pub fn open_sized_tab(&mut self, width: u32, height: u32) {
        self.untitled_counter += 1;
        let title = format!("Sans titre {}", self.untitled_counter);
        self.spawn_document(title, Document::new(width, height));
    }

    /// Crée le worker d'un document et l'active.
    pub fn spawn_document(&mut self, title: String, document: Document) {
        let (tx, worker_rx) = channel();
        let (worker_tx, rx) = channel();
        // Handle détaché volontairement : le worker vit tant que
        // le channel est ouvert, sans jamais bloquer l'UI.
        std::mem::forget(spawn_photo_engine_worker(document, worker_rx, worker_tx));
        self.docs.push(OpenDocument {
            title,
            ui: PhotoUiState {
                needs_repaint: true,
                status: String::from("Pret"),
                ..Default::default()
            },
            tx,
            rx,
        });
        self.active = self.docs.len() - 1;
        let _ = self.active_doc().tx.send(PhotoEngineCommand::Refresh);
    }

    /// Ferme l'onglet actif (jamais zéro document : le dernier est
    /// remplacé par un neuf). Le worker s'arrête à la chute du channel.
    pub fn close_active_tab(&mut self) {
        if self.docs.is_empty() {
            self.open_blank_tab();
            return;
        }
        self.docs.remove(self.active);
        if self.docs.is_empty() {
            self.open_blank_tab();
        } else {
            self.active = self.active.min(self.docs.len() - 1);
        }
    }

    /// Bascule vers l'onglet `index` (clampé).
    pub fn switch_tab(&mut self, index: usize) {
        if !self.docs.is_empty() {
            self.active = index.min(self.docs.len() - 1);
        }
    }

    /// Poll non bloquant : réponses workers + file pickers.
    pub fn poll(&mut self, ctx: &egui::Context) {
        for doc in &mut self.docs {
            while let Ok(response) = doc.rx.try_recv() {
                apply_response(ctx, &mut doc.ui, response);
            }
        }
        // Ouverture d'image → document actif.
        if let Some(rx) = self.open_picker.take() {
            match rx.try_recv() {
                Ok(Some(path)) => {
                    let _ = self
                        .active_doc()
                        .tx
                        .send(PhotoEngineCommand::OpenImage { path });
                }
                Ok(None) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.open_picker = Some(rx);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.active_doc_mut().ui.status =
                        String::from("Dialogue de fichier indisponible");
                }
            }
        }
    }

    /// Un frame complet : poll puis dessin.
    pub fn draw(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.poll(ctx);
        draw_photo_layout(ui, self);
        if self.docs.iter().any(|doc| doc.ui.needs_repaint) {
            for doc in &mut self.docs {
                doc.ui.needs_repaint = false;
            }
            ctx.request_repaint();
        }
    }
}

impl Default for PhotoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for PhotoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.draw(&ctx, ui);
    }
}

/// Applique une réponse worker à l'état UI d'un document.
pub fn apply_response(ctx: &egui::Context, ui: &mut PhotoUiState, response: PhotoEngineResponse) {
    match response {
        PhotoEngineResponse::LayersChanged {
            layers,
            preview,
            can_undo,
            can_redo,
        } => {
            ui.layers = layers;
            ui.can_undo = can_undo;
            ui.can_redo = can_redo;
            if ui
                .selected
                .is_some_and(|id| !ui.layers.iter().any(|layer| layer.id == id))
            {
                ui.selected = None;
            }
            match preview {
                Some(image) => {
                    ui.texture_cache.update(
                        ctx,
                        "photo_preview",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width as usize, image.height as usize],
                            &image.rgba,
                        ),
                    );
                    ui.last_preview = Some(image);
                    ui.status.clear();
                }
                None => {
                    ui.texture_cache.clear();
                    ui.last_preview = None;
                }
            }
            ui.needs_repaint = true;
        }
        PhotoEngineResponse::EngineError { message } => {
            ui.status = message;
            ui.needs_repaint = true;
        }
        PhotoEngineResponse::ExportDone { path } => {
            ui.status = format!("Exporte : {}", path.display());
            ui.needs_repaint = true;
        }
    }
}

/// Échantillonne la couleur du composite sous `(x, y)` pixels image.
/// `None` hors bornes ou sans aperçu (la pipette ignore alors le clic).
pub fn sample_preview_color(preview: &PreviewImage, x: f32, y: f32) -> Option<[u8; 3]> {
    if preview.width == 0 || preview.height == 0 {
        return None;
    }
    let xi = x.round() as i64;
    let yi = y.round() as i64;
    if xi < 0 || yi < 0 || xi >= preview.width as i64 || yi >= preview.height as i64 {
        return None;
    }
    let offset = (yi as usize * preview.width as usize + xi as usize) * 4;
    preview
        .rgba
        .get(offset..offset + 3)
        .and_then(|pixel| <&[u8] as TryInto<&[u8; 3]>>::try_into(pixel).ok())
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::engine_bridge::PhotoEngineCommand;

    #[test]
    fn app_boots_with_one_blank_doc() {
        let mut app = PhotoApp::new();
        assert_eq!(app.doc_count(), 1);
        assert_eq!(app.active, 0);
        // Snapshot initial du worker (document vide).
        let response = app
            .active_doc()
            .rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("snapshot initial");
        let ctx = egui::Context::default();
        apply_response(&ctx, &mut app.active_doc_mut().ui, response);
        assert!(app.active_doc().ui.layers.is_empty());
    }

    #[test]
    fn tabs_open_switch_close() {
        let mut app = PhotoApp::new();
        app.open_blank_tab();
        app.open_blank_tab();
        assert_eq!(app.doc_count(), 3);
        app.switch_tab(0);
        assert_eq!(app.active, 0);
        app.switch_tab(99);
        assert_eq!(app.active, 2);
        app.close_active_tab();
        assert_eq!(app.doc_count(), 2);
        // Fermer jusqu'au dernier : jamais zéro document.
        app.close_active_tab();
        app.close_active_tab();
        assert_eq!(app.doc_count(), 1);
    }

    #[test]
    fn sample_preview_color_clamps() {
        let preview = PreviewImage {
            width: 2,
            height: 2,
            rgba: vec![
                10, 20, 30, 255, 40, 50, 60, 255, //
                70, 80, 90, 255, 100, 110, 120, 255,
            ],
        };
        assert_eq!(sample_preview_color(&preview, 0.0, 0.0), Some([10, 20, 30]));
        assert_eq!(
            sample_preview_color(&preview, 1.0, 1.0),
            Some([100, 110, 120])
        );
        assert_eq!(sample_preview_color(&preview, 5.0, 0.0), None);
        assert_eq!(sample_preview_color(&preview, -1.0, 0.0), None);
        let empty = PreviewImage {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        assert_eq!(sample_preview_color(&empty, 0.0, 0.0), None);
    }

    #[test]
    fn engine_error_sets_status() {
        let ctx = egui::Context::default();
        let mut ui = PhotoUiState::default();
        apply_response(
            &ctx,
            &mut ui,
            PhotoEngineResponse::EngineError {
                message: String::from("echec test"),
            },
        );
        assert_eq!(ui.status, "echec test");
    }

    #[test]
    fn export_done_sets_status() {
        let ctx = egui::Context::default();
        let mut ui = PhotoUiState::default();
        apply_response(
            &ctx,
            &mut ui,
            PhotoEngineResponse::ExportDone {
                path: PathBuf::from("/tmp/test.png"),
            },
        );
        assert!(ui.status.contains("test.png"));
    }

    #[test]
    fn full_layout_draws_without_panic() {
        let mut app = PhotoApp::new();
        app.active_doc_mut().ui.status = String::from("test");
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_photo_layout(ui, &mut app);
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn heavy_image_open_never_blocks_ui_thread() {
        use std::time::{Duration, Instant};

        // Grosse image (3000x3000) : décodage + composite coûteux,
        // intégralement sur le worker.
        let path =
            std::env::temp_dir().join(format!("cygnus_phase5_big_{}.png", std::process::id()));
        image::save_buffer(
            &path,
            &vec![128u8; 3000 * 3000 * 3],
            3000,
            3000,
            image::ColorType::Rgb8,
        )
        .expect("image de test");
        let app = PhotoApp::new();
        app.active_doc()
            .tx
            .send(PhotoEngineCommand::OpenImage { path: path.clone() })
            .expect("envoi commande");

        // Pattern de la boucle egui : poll non bloquant, le thread UI
        // reste libre (chaque try_recv retourne immédiatement). On
        // ignore les réponses antérieures (snapshot initial du boot).
        let start = Instant::now();
        let mut polls = 0;
        let layers = loop {
            polls += 1;
            match app.active_doc().rx.try_recv() {
                Ok(PhotoEngineResponse::LayersChanged { layers, .. })
                    if layers
                        .iter()
                        .any(|layer| layer.name.starts_with("cygnus_phase5_big_")) =>
                {
                    break layers;
                }
                Ok(_) => {
                    // Snapshot initial ou aperçu intermédiaire : on continue.
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    assert!(
                        start.elapsed() < Duration::from_secs(60),
                        "worker bloqué ou perdu"
                    );
                    std::thread::yield_now();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    panic!("worker arrêté prématurément")
                }
            }
        };
        assert!(polls >= 1, "au moins un poll non bloquant");
        assert!(
            layers
                .iter()
                .any(|layer| layer.name.starts_with("cygnus_phase5_big_")),
            "calque de l'image lourde présent"
        );
        let _ = std::fs::remove_file(&path);
    }
}
