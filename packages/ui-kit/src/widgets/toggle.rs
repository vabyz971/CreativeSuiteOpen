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

//! Interrupteur standard Cygnus (booléen avec libellé).
//!
//! Utilisé pour la visibilité des calques, les options d'export, les
//! préférences — style unifié via les tokens du thème.

use crate::theme::tokens::CygnusTheme;

/// Interrupteur labellisé Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::toggle::CygnusToggle;
/// # egui::__run_test_ui(|ui| {
/// let mut visible = true;
/// CygnusToggle::new("Visible").show(ui, &mut visible);
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct CygnusToggle<'a> {
    label: &'a str,
}

impl<'a> CygnusToggle<'a> {
    /// Crée un interrupteur avec libellé.
    pub fn new(label: &'a str) -> Self {
        Self { label }
    }

    /// Affiche l'interrupteur et retourne la réponse egui.
    pub fn show(self, ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
        let theme = CygnusTheme::dark();
        ui.horizontal(|ui| {
            ui.checkbox(
                on,
                egui::RichText::new(self.label)
                    .size(theme.typography.body_size)
                    .color(theme.colors.fg_primary),
            )
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_renders_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut on = true;
                let _ = CygnusToggle::new("Visible").show(ui, &mut on);
            });
        })
        .drop_without_applying_deltas();
    }
}
