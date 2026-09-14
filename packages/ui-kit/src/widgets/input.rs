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

//! Champs de saisie standard Cygnus (texte et nombre).
//!
//! Utilisés pour le renommage de calques, les dimensions, les valeurs
//! numériques — toujours avec le style du thème, jamais en dur.

use crate::theme::tokens::CygnusTheme;

/// Champ texte standard Cygnus (une ligne, avec libellé et placeholder).
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::input::CygnusTextInput;
/// # egui::__run_test_ui(|ui| {
/// let mut name = String::from("Calque 1");
/// CygnusTextInput::new("Nom").hint("Nom du calque").show(ui, &mut name);
/// # });
/// ```
#[derive(Debug, Clone, Default)]
pub struct CygnusTextInput<'a> {
    label: &'a str,
    hint: &'a str,
}

impl<'a> CygnusTextInput<'a> {
    /// Crée un champ texte avec libellé.
    pub fn new(label: &'a str) -> Self {
        Self { label, hint: "" }
    }

    /// Texte d'aide affiché quand le champ est vide.
    #[must_use]
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// Affiche le champ et retourne la réponse egui.
    pub fn show(self, ui: &mut egui::Ui, value: &mut String) -> egui::Response {
        let theme = CygnusTheme::dark();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.label)
                    .size(theme.typography.body_size)
                    .color(theme.colors.fg_secondary),
            );
            ui.add(
                egui::TextEdit::singleline(value)
                    .hint_text(self.hint)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner
    }
}

/// Champ numérique standard Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::input::CygnusNumberInput;
/// # egui::__run_test_ui(|ui| {
/// let mut zoom = 1.0;
/// CygnusNumberInput::new("Zoom").show(ui, &mut zoom);
/// # });
/// ```
#[derive(Debug, Clone, Default)]
pub struct CygnusNumberInput<'a> {
    label: &'a str,
    range: Option<std::ops::RangeInclusive<f64>>,
}

impl<'a> CygnusNumberInput<'a> {
    /// Crée un champ numérique avec libellé.
    pub fn new(label: &'a str) -> Self {
        Self { label, range: None }
    }

    /// Plage de valeurs autorisées.
    #[must_use]
    pub fn range(mut self, range: std::ops::RangeInclusive<f64>) -> Self {
        self.range = Some(range);
        self
    }

    /// Affiche le champ et retourne la réponse egui.
    pub fn show(self, ui: &mut egui::Ui, value: &mut f64) -> egui::Response {
        let theme = CygnusTheme::dark();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.label)
                    .size(theme.typography.body_size)
                    .color(theme.colors.fg_secondary),
            );
            let mut edit = egui::DragValue::new(value).speed(0.1);
            if let Some(range) = self.range {
                edit = edit.range(range);
            }
            ui.add(edit)
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_render_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut name = String::from("Calque 1");
                let _ = CygnusTextInput::new("Nom").hint("Nom").show(ui, &mut name);
                let mut zoom = 1.0;
                let _ = CygnusNumberInput::new("Zoom")
                    .range(0.1..=8.0)
                    .show(ui, &mut zoom);
            });
        })
        .drop_without_applying_deltas();
    }
}
