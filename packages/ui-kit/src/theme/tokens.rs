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

//! Tokens de design centralisés pour toute la suite Cygnus.
//!
//! SEULE source de vérité pour couleurs, espacements, radius et typo.
//! Les widgets et les apps référencent ces tokens au lieu de coder
//! des valeurs en dur.

/// Couleurs du thème Cygnus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CygnusColors {
    /// Fond principal des panneaux.
    pub bg_primary: egui::Color32,
    /// Fond des fenêtres et barres.
    pub bg_secondary: egui::Color32,
    /// Fond des zones encastrées (inputs, canvas vide).
    pub bg_tertiary: egui::Color32,
    /// Texte principal.
    pub fg_primary: egui::Color32,
    /// Texte secondaire / désactivé.
    pub fg_secondary: egui::Color32,
    /// Accent (sélection, boutons primaires).
    pub accent: egui::Color32,
    /// Accent au survol.
    pub accent_hover: egui::Color32,
    /// Bordures.
    pub border: egui::Color32,
    /// Erreur.
    pub error: egui::Color32,
    /// Succès.
    pub success: egui::Color32,
    /// Avertissement.
    pub warning: egui::Color32,
    /// Fond d'un item sélectionné (calque, clip, piste).
    pub item_selected: egui::Color32,
    /// Fond d'un item survolé.
    pub item_hover: egui::Color32,
    /// Indicateur de position de drop (drag & drop).
    pub drop_indicator: egui::Color32,
}

/// Espacements du thème.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CygnusSpacing {
    /// Très petit espacement (4px).
    pub xs: f32,
    /// Petit espacement (8px).
    pub sm: f32,
    /// Espacement moyen (12px).
    pub md: f32,
    /// Grand espacement (16px).
    pub lg: f32,
    /// Très grand espacement (24px).
    pub xl: f32,
}

/// Rayons de coins du thème.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CygnusRadius {
    /// Petit rayon (4px).
    pub sm: f32,
    /// Rayon moyen (8px).
    pub md: f32,
    /// Grand rayon (12px).
    pub lg: f32,
    /// Rayon "pilule" (cercles).
    pub full: f32,
}

/// Tailles typographiques du thème.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CygnusTypography {
    /// Titres de panneaux.
    pub heading_size: f32,
    /// Texte courant.
    pub body_size: f32,
    /// Légendes et textes secondaires.
    pub caption_size: f32,
    /// Icônes.
    pub icon_size: f32,
}

/// Thème complet de Cygnus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CygnusTheme {
    /// Couleurs.
    pub colors: CygnusColors,
    /// Espacements.
    pub spacing: CygnusSpacing,
    /// Rayons.
    pub radius: CygnusRadius,
    /// Typographie.
    pub typography: CygnusTypography,
}

impl CygnusTheme {
    /// Thème sombre par défaut de Cygnus.
    pub fn dark() -> Self {
        Self {
            colors: CygnusColors {
                bg_primary: egui::Color32::from_rgb(18, 18, 22),
                bg_secondary: egui::Color32::from_rgb(24, 24, 30),
                bg_tertiary: egui::Color32::from_rgb(32, 32, 40),
                fg_primary: egui::Color32::from_rgb(240, 240, 245),
                fg_secondary: egui::Color32::from_rgb(160, 160, 175),
                accent: egui::Color32::from_rgb(88, 124, 255),
                accent_hover: egui::Color32::from_rgb(108, 144, 255),
                border: egui::Color32::from_rgb(48, 48, 60),
                error: egui::Color32::from_rgb(255, 85, 85),
                success: egui::Color32::from_rgb(80, 220, 120),
                warning: egui::Color32::from_rgb(255, 190, 60),
                item_selected: egui::Color32::from_rgb(45, 55, 90),
                item_hover: egui::Color32::from_rgb(35, 35, 45),
                drop_indicator: egui::Color32::from_rgb(88, 124, 255),
            },
            spacing: CygnusSpacing {
                xs: 4.0,
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 24.0,
            },
            radius: CygnusRadius {
                sm: 4.0,
                md: 8.0,
                lg: 12.0,
                full: 9999.0,
            },
            typography: CygnusTypography {
                heading_size: 16.0,
                body_size: 13.0,
                caption_size: 11.0,
                icon_size: 18.0,
            },
        }
    }
}

impl Default for CygnusTheme {
    /// Thème sombre par défaut.
    fn default() -> Self {
        Self::dark()
    }
}

/// Luminance relative approximative (canal par canal, sans gamma).
#[cfg(test)]
fn luminance(color: egui::Color32) -> f32 {
    0.2126 * f32::from(color.r()) + 0.7152 * f32::from(color.g()) + 0.0722 * f32::from(color.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_colors_distinct() {
        let theme = CygnusTheme::dark();
        assert_ne!(theme.colors.bg_primary, theme.colors.bg_secondary);
        assert_ne!(theme.colors.bg_secondary, theme.colors.bg_tertiary);
        assert_ne!(theme.colors.bg_primary, theme.colors.bg_tertiary);
        assert_ne!(theme.colors.fg_primary, theme.colors.fg_secondary);
        assert_ne!(theme.colors.accent, theme.colors.accent_hover);
    }

    #[test]
    fn dark_theme_fg_contrasts_bg() {
        // Le texte principal doit fortement contraster avec le fond principal.
        let theme = CygnusTheme::dark();
        let contrast =
            (luminance(theme.colors.fg_primary) - luminance(theme.colors.bg_primary)).abs();
        assert!(contrast > 150.0, "contraste fg/bg trop faible : {contrast}");
    }

    #[test]
    fn dark_theme_spacing_ordered() {
        let spacing = CygnusTheme::dark().spacing;
        assert!(spacing.xs < spacing.sm);
        assert!(spacing.sm < spacing.md);
        assert!(spacing.md < spacing.lg);
        assert!(spacing.lg < spacing.xl);
    }
}
