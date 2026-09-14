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

//! Menu contextuel PHOTO (barre haute, widget métier).
//!
//! Cinq menus : Fichier (nouveau document avec paramètres, ouvrir,
//! exporter avec paramètres, quitter), Édition (annuler, rétablir),
//! Calque (vide, image, dupliquer, masque, supprimer), Affichage
//! (grille, zoom), Aide. Style exclusivement ui-kit ; les fenêtres de
//! paramètres (nouveau document, export) sont des modales détenues
//! par l'app (voir `super::dialogs`).

/// Action rapportée par [`draw_menu_bar`], traitée par l'app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoMenuAction {
    /// Nouveau document (ouvre la fenêtre de paramètres).
    NewDocument,
    /// Ouvrir une image (file picker non bloquant).
    OpenImage,
    /// Exporter (ouvre la fenêtre de paramètres d'export).
    Export,
    /// Quitter l'application.
    Quit,
    /// Annuler / rétablir (worker).
    Undo,
    /// Rétablir.
    Redo,
    /// Nouveau calque vide.
    AddEmptyLayer,
    /// Dupliquer le calque sélectionné.
    DuplicateLayer,
    /// Ajouter un masque au calque sélectionné.
    AddMask,
    /// Supprimer le calque sélectionné.
    DeleteLayer,
    /// Basculer la grille du canvas.
    ToggleGrid,
    /// Zoom avant / arrière (ancré au centre).
    ZoomIn,
    /// Zoom arrière.
    ZoomOut,
    /// Réinitialiser le zoom à 100 %.
    ZoomReset,
    /// Ouvrir la fenêtre d'aide.
    ShowHelp,
}

/// Disponibilités pour griser les entrées (historique, sélection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuAvailability {
    /// Undo / redo possibles.
    pub can_undo: bool,
    /// Redo possible.
    pub can_redo: bool,
    /// Un calque est sélectionné.
    pub has_selection: bool,
}

/// Dessine la barre de menus et retourne les actions.
pub fn draw_menu_bar(ui: &mut egui::Ui, availability: MenuAvailability) -> Vec<PhotoMenuAction> {
    let mut actions = Vec::new();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("Fichier", |ui| {
            if ui.button("Nouveau document...").clicked() {
                actions.push(PhotoMenuAction::NewDocument);
                ui.close();
            }
            if ui.button("Ouvrir une image...").clicked() {
                actions.push(PhotoMenuAction::OpenImage);
                ui.close();
            }
            if ui.button("Exportation...").clicked() {
                actions.push(PhotoMenuAction::Export);
                ui.close();
            }
            ui.separator();
            if ui.button("Quitter").clicked() {
                actions.push(PhotoMenuAction::Quit);
                ui.close();
            }
        });
        ui.menu_button("Édition", |ui| {
            ui.add_enabled_ui(availability.can_undo, |ui| {
                if ui.button("Annuler").clicked() {
                    actions.push(PhotoMenuAction::Undo);
                    ui.close();
                }
            });
            ui.add_enabled_ui(availability.can_redo, |ui| {
                if ui.button("Rétablir").clicked() {
                    actions.push(PhotoMenuAction::Redo);
                    ui.close();
                }
            });
        });
        ui.menu_button("Calque", |ui| {
            if ui.button("Nouveau calque vide").clicked() {
                actions.push(PhotoMenuAction::AddEmptyLayer);
                ui.close();
            }
            if ui.button("Calque depuis une image...").clicked() {
                actions.push(PhotoMenuAction::OpenImage);
                ui.close();
            }
            ui.add_enabled_ui(availability.has_selection, |ui| {
                if ui.button("Dupliquer le calque").clicked() {
                    actions.push(PhotoMenuAction::DuplicateLayer);
                    ui.close();
                }
                if ui.button("Ajouter un masque").clicked() {
                    actions.push(PhotoMenuAction::AddMask);
                    ui.close();
                }
                if ui.button("Supprimer le calque").clicked() {
                    actions.push(PhotoMenuAction::DeleteLayer);
                    ui.close();
                }
            });
        });
        ui.menu_button("Affichage", |ui| {
            if ui.button("Grille on/off").clicked() {
                actions.push(PhotoMenuAction::ToggleGrid);
                ui.close();
            }
            if ui.button("Zoom avant").clicked() {
                actions.push(PhotoMenuAction::ZoomIn);
                ui.close();
            }
            if ui.button("Zoom arrière").clicked() {
                actions.push(PhotoMenuAction::ZoomOut);
                ui.close();
            }
            if ui.button("Zoom 100 %").clicked() {
                actions.push(PhotoMenuAction::ZoomReset);
                ui.close();
            }
        });
        ui.menu_button("Aide", |ui| {
            if ui.button("Aide de Photo").clicked() {
                actions.push(PhotoMenuAction::ShowHelp);
                ui.close();
            }
        });
    });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_renders_without_panic_and_idle() {
        let ctx = egui::Context::default();
        ui_kit::theme::setup_fonts(&ctx);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let actions = draw_menu_bar(ui, MenuAvailability::default());
                assert!(actions.is_empty(), "aucun clic sans interaction");
            });
        })
        .drop_without_applying_deltas();
    }
}
