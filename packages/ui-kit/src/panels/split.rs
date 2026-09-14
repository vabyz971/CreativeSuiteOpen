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

//! Panneau divisé redimensionnable (horizontal ou vertical).
//!
//! Le redimensionnement est géré nativement par [`egui::Panel`]
//! (poignée de drag) ; [`CygnusSplitState`] expose la logique pure de
//! clamp (tailles minimales) pour les ajustements programmatiques, ce
//! qui la rend testable sans contexte UI.

/// État et logique pure d'un panneau divisé.
///
/// `fraction` est la part (0..=1) de l'espace total allouée à la
/// première zone. Les tailles minimales garantissent qu'aucune zone
/// ne s'effondre sous son seuil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CygnusSplitState {
    fraction: f32,
    min_first: f32,
    min_second: f32,
}

impl CygnusSplitState {
    /// Crée un état de split. `fraction` est clampée à 0..=1.
    pub fn new(fraction: f32, min_first: f32, min_second: f32) -> Self {
        let mut state = Self {
            fraction: 0.5,
            min_first: min_first.max(0.0),
            min_second: min_second.max(0.0),
        };
        state.set_fraction(fraction);
        state
    }

    /// Met à jour la fraction (clampée à 0..=1).
    pub fn set_fraction(&mut self, fraction: f32) {
        self.fraction = fraction.clamp(0.0, 1.0);
    }

    /// Fraction courante (0..=1).
    pub fn fraction(self) -> f32 {
        self.fraction
    }

    /// Taille minimale de la première zone.
    pub fn min_first(self) -> f32 {
        self.min_first
    }

    /// Taille minimale de la seconde zone.
    pub fn min_second(self) -> f32 {
        self.min_second
    }

    /// Clamp une taille demandée pour la première zone au respect des
    /// deux minimums. Ne panique jamais, même si `total` est plus petit
    /// que la somme des minimums (cas dégénéré : la première zone prend
    /// tout l'espace disponible).
    pub fn clamp_first_size(total: f32, requested: f32, min_first: f32, min_second: f32) -> f32 {
        let total = total.max(0.0);
        let max_first = (total - min_second.max(0.0)).max(0.0);
        let min_first = min_first.max(0.0).min(total);
        if min_first >= max_first {
            return min_first.min(total);
        }
        requested.clamp(min_first, max_first)
    }

    /// Taille de la première zone pour un espace total donné.
    pub fn first_size(self, total: f32) -> f32 {
        Self::clamp_first_size(
            total,
            self.fraction * total,
            self.min_first,
            self.min_second,
        )
    }

    /// Taille de la seconde zone pour un espace total donné.
    pub fn second_size(self, total: f32) -> f32 {
        (total.max(0.0) - self.first_size(total)).max(0.0)
    }
}

/// Panneau divisé redimensionnable (poignée de drag native egui).
///
/// # Exemple
/// ```rust
/// # use ui_kit::panels::CygnusSplitPanel;
/// # egui::__run_test_ui(|ui| {
/// CygnusSplitPanel::new("photo_split", 280.0, 160.0, 200.0).show_horizontal(
///     ui,
///     |ui| { ui.label("calques"); },
///     |ui| { ui.label("canvas"); },
/// );
/// # });
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CygnusSplitPanel {
    id: &'static str,
    default_first: f32,
    min_first: f32,
    min_second: f32,
}

impl CygnusSplitPanel {
    /// Crée un split : `default_first` est la taille initiale de la
    /// première zone, `min_first` sa taille minimale native,
    /// `min_second` le minimum à garantir à la seconde zone lors
    /// d'ajustements programmatiques (voir [`CygnusSplitState`]).
    pub fn new(id: &'static str, default_first: f32, min_first: f32, min_second: f32) -> Self {
        Self {
            id,
            default_first,
            min_first,
            min_second,
        }
    }

    /// Minimum garanti à la seconde zone (ajustements programmatiques).
    pub fn min_second(self) -> f32 {
        self.min_second
    }

    /// Split horizontal : zone redimensionnable à gauche, reste au centre.
    pub fn show_horizontal<R>(
        self,
        ui: &mut egui::Ui,
        first: impl FnOnce(&mut egui::Ui),
        second: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        egui::Panel::left(self.id)
            .resizable(true)
            .default_size(self.default_first)
            .min_size(self.min_first)
            .show(ui, |ui| {
                first(ui);
            });
        egui::CentralPanel::default().show(ui, second)
    }

    /// Split vertical : zone redimensionnable en haut, reste au centre.
    pub fn show_vertical<R>(
        self,
        ui: &mut egui::Ui,
        first: impl FnOnce(&mut egui::Ui),
        second: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        egui::Panel::top(self.id)
            .resizable(true)
            .default_size(self.default_first)
            .min_size(self.min_first)
            .show(ui, |ui| {
                first(ui);
            });
        egui::CentralPanel::default().show(ui, second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_respects_min_sizes() {
        // Demande sous le minimum -> minimum appliqué.
        assert_eq!(
            CygnusSplitState::clamp_first_size(1000.0, 10.0, 160.0, 200.0),
            160.0
        );
        // Demande écrasant la seconde zone -> seconde zone préservée.
        assert_eq!(
            CygnusSplitState::clamp_first_size(1000.0, 900.0, 160.0, 200.0),
            800.0
        );
        // Demande valide -> inchangée.
        assert_eq!(
            CygnusSplitState::clamp_first_size(1000.0, 300.0, 160.0, 200.0),
            300.0
        );
    }

    #[test]
    fn split_degenerate_total_never_panics() {
        // Espace total inférieur à la somme des minimums : pas de panic,
        // tailles contenues dans [0, total].
        for total in [0.0, 50.0, 200.0, 359.0] {
            let state = CygnusSplitState::new(0.5, 160.0, 200.0);
            let first = state.first_size(total);
            let second = state.second_size(total);
            assert!(
                (0.0..=total).contains(&first),
                "first={first} total={total}"
            );
            assert!(
                (0.0..=total).contains(&second),
                "second={second} total={total}"
            );
        }
    }

    #[test]
    fn split_fraction_is_clamped() {
        let mut state = CygnusSplitState::new(2.0, 100.0, 100.0);
        assert_eq!(state.fraction(), 1.0);
        state.set_fraction(-1.0);
        assert_eq!(state.fraction(), 0.0);
    }

    #[test]
    fn splits_render_without_panic() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            CygnusSplitPanel::new("test_split_h", 280.0, 160.0, 200.0).show_horizontal(
                ui,
                |ui| {
                    ui.label("gauche");
                },
                |ui| {
                    ui.label("centre");
                },
            );
        })
        .drop_without_applying_deltas();

        ctx.run_ui(egui::RawInput::default(), |ui| {
            CygnusSplitPanel::new("test_split_v", 200.0, 120.0, 200.0).show_vertical(
                ui,
                |ui| {
                    ui.label("haut");
                },
                |ui| {
                    ui.label("centre");
                },
            );
        })
        .drop_without_applying_deltas();
    }
}
