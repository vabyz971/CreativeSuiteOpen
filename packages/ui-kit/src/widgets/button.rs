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

//! Bouton standard Cygnus (variantes primary / secondary / ghost).
//!
//! Toute action cliquable des 3 apps passe par [`CygnusButton`] : aucun
//! style de bouton codé en dur côté app.

use crate::theme::tokens::CygnusTheme;

/// Variante visuelle d'un bouton Cygnus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CygnusButtonStyle {
    /// Action principale (fond accent).
    Primary,
    /// Action secondaire (fond tertiaire, bordure).
    #[default]
    Secondary,
    /// Action discrète (sans fond, texte seul).
    Ghost,
}

/// Bouton standard Cygnus.
///
/// # Exemple
/// ```rust
/// # use ui_kit::widgets::button::{CygnusButton, CygnusButtonStyle};
/// # egui::__run_test_ui(|ui| {
/// if CygnusButton::new("Exporter").style(CygnusButtonStyle::Primary).show(ui).clicked() {
///     // … envoyer EngineCommand::Export via le channel …
/// }
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct CygnusButton<'a> {
    label: &'a str,
    style: CygnusButtonStyle,
}

impl<'a> CygnusButton<'a> {
    /// Crée un bouton avec le libellé donné (style secondaire par défaut).
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            style: CygnusButtonStyle::Secondary,
        }
    }

    /// Variante visuelle du bouton.
    #[must_use]
    pub fn style(mut self, style: CygnusButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Affiche le bouton et retourne la réponse egui.
    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = CygnusTheme::dark();
        let text = egui::RichText::new(self.label).size(theme.typography.body_size);
        match self.style {
            CygnusButtonStyle::Primary => ui
                .add(egui::Button::new(text.color(egui::Color32::WHITE)).fill(theme.colors.accent)),
            CygnusButtonStyle::Secondary => ui.add(
                egui::Button::new(text.color(theme.colors.fg_primary))
                    .fill(theme.colors.bg_tertiary)
                    .stroke(egui::Stroke::new(1.0, theme.colors.border)),
            ),
            CygnusButtonStyle::Ghost => {
                ui.add(egui::Button::new(text.color(theme.colors.fg_secondary)).frame(false))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_render_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                for style in [
                    CygnusButtonStyle::Primary,
                    CygnusButtonStyle::Secondary,
                    CygnusButtonStyle::Ghost,
                ] {
                    let _ = CygnusButton::new("Test").style(style).show(ui);
                }
            });
        })
        .drop_without_applying_deltas();
    }
}
