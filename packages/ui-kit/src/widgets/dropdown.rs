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

//! Menu déroulant standard Cygnus (sélection parmi des options).
//!
//! Utilisé pour les modes de fusion, les presets, les choix
//! d'export — style unifié via les tokens du thème.

use super::super::theme::tokens::CygnusTheme;

/// Clamp un index d'option à la plage valide (0 pour liste vide).
pub fn sanitize_selected(selected: usize, len: usize) -> usize {
    if len == 0 { 0 } else { selected.min(len - 1) }
}

/// Menu déroulant standard Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::dropdown::CygnusDropdown;
/// # egui::__run_test_ui(|ui| {
/// let mut blend = 0;
/// CygnusDropdown::new("Fusion", &["Normal", "Multiplier", "Ecran"])
///     .show(ui, &mut blend);
/// # });
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CygnusDropdown<'a> {
    label: &'a str,
    options: &'a [&'a str],
}

impl<'a> CygnusDropdown<'a> {
    /// Crée un menu déroulant avec libellé et options.
    pub fn new(label: &'a str, options: &'a [&'a str]) -> Self {
        Self { label, options }
    }

    /// Nombre d'options.
    pub fn len(self) -> usize {
        self.options.len()
    }

    /// Vrai si aucune option.
    pub fn is_empty(self) -> bool {
        self.options.is_empty()
    }

    /// Affiche le menu et met à jour l'index sélectionné.
    pub fn show(self, ui: &mut egui::Ui, selected: &mut usize) -> egui::Response {
        let theme = CygnusTheme::dark();
        *selected = sanitize_selected(*selected, self.options.len());
        let current = self.options.get(*selected).copied().unwrap_or("");
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.label)
                    .size(theme.typography.body_size)
                    .color(theme.colors.fg_secondary),
            );
            egui::ComboBox::from_label("")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (index, option) in self.options.iter().enumerate() {
                        ui.selectable_value(selected, index, *option);
                    }
                })
                .response
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps() {
        assert_eq!(sanitize_selected(0, 0), 0);
        assert_eq!(sanitize_selected(9, 3), 2);
        assert_eq!(sanitize_selected(1, 3), 1);
    }

    #[test]
    fn dropdown_renders_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut selected = 5;
                let response = CygnusDropdown::new("Fusion", &["Normal", "Multiplier"])
                    .show(ui, &mut selected);
                assert_eq!(selected, 1);
                let _ = response;
            });
        })
        .drop_without_applying_deltas();
    }
}
