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

//! Modèle de données des préférences — sérialisable, tolérant aux
//! évolutions (tous les champs portent `#[serde(default)]` : un fichier
//! d'une version antérieure ou postérieure se charge sans erreur).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// API de rendu demandée à wgpu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderApi {
    #[default]
    Auto,
    Vulkan,
    Metal,
    Dx12,
    Gl,
}

impl RenderApi {
    /// Cette API est-elle disponible sur la plateforme actuelle ?
    #[must_use]
    pub fn is_available_on_current_platform(self) -> bool {
        match self {
            RenderApi::Auto => true,
            RenderApi::Vulkan => cfg!(target_os = "linux") || cfg!(target_os = "windows"),
            RenderApi::Metal => cfg!(target_os = "macos"),
            RenderApi::Dx12 => cfg!(target_os = "windows"),
            // GL : backend logiciel possible partout
            RenderApi::Gl => true,
        }
    }

    /// API recommandée selon la plateforme.
    #[must_use]
    pub fn recommended_for_current_platform() -> Self {
        if cfg!(target_os = "macos") {
            RenderApi::Metal
        } else if cfg!(target_os = "windows") {
            RenderApi::Dx12
        } else {
            RenderApi::Vulkan
        }
    }

    pub const ALL: [RenderApi; 5] = [
        RenderApi::Auto,
        RenderApi::Vulkan,
        RenderApi::Metal,
        RenderApi::Dx12,
        RenderApi::Gl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderApi::Auto => "Automatique",
            RenderApi::Vulkan => "Vulkan",
            RenderApi::Metal => "Metal",
            RenderApi::Dx12 => "DirectX 12",
            RenderApi::Gl => "OpenGL",
        }
    }
}

/// Thème d'interface (le thème sombre est le seul rendu aujourd'hui ;
/// Light/System sont stockés pour l'avenir).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Light, Theme::System];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "Sombre",
            Theme::Light => "Clair",
            Theme::System => "Système",
        }
    }
}

/// Profil de qualité du rendu (réservé pour l'anti-aliasing/échelle).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderQuality {
    Performance,
    #[default]
    Balanced,
    Quality,
}

impl RenderQuality {
    pub const ALL: [RenderQuality; 3] = [
        RenderQuality::Performance,
        RenderQuality::Balanced,
        RenderQuality::Quality,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderQuality::Performance => "Performance",
            RenderQuality::Balanced => "Équilibré",
            RenderQuality::Quality => "Qualité",
        }
    }
}

/// Section Général.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeneralPreferences {
    pub language: String,
    pub theme: Theme,
    pub auto_save: bool,
    pub auto_save_interval_secs: u32,
}

/// Section Rendu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderPreferences {
    pub api: RenderApi,
    pub quality: RenderQuality,
    pub vsync: bool,
    pub gpu_cache_limit_mb: u32,
}

impl Default for RenderPreferences {
    fn default() -> Self {
        Self {
            api: RenderApi::Auto,
            quality: RenderQuality::Balanced,
            vsync: true,
            gpu_cache_limit_mb: 2048,
        }
    }
}

/// Table de raccourcis : id d'action → combinaison (« Ctrl+Shift+S »).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeybindingPreferences {
    pub bindings: HashMap<String, String>,
}

impl KeybindingPreferences {
    /// Combinaisons par défaut — la référence aussi pour « réinitialiser ».
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut bindings = HashMap::new();
        // Outils
        bindings.insert("tool_brush".to_string(), "B".to_string());
        bindings.insert("tool_eraser".to_string(), "E".to_string());
        bindings.insert("tool_eyedropper".to_string(), "I".to_string());
        bindings.insert("tool_move".to_string(), "V".to_string());
        bindings.insert("tool_hand".to_string(), "H".to_string());
        bindings.insert("tool_zoom".to_string(), "Z".to_string());
        // Édition
        bindings.insert("undo".to_string(), "Ctrl+Z".to_string());
        bindings.insert("redo".to_string(), "Ctrl+Y".to_string());
        bindings.insert("delete_layer".to_string(), "Delete".to_string());
        // Fichier
        bindings.insert("save".to_string(), "Ctrl+S".to_string());
        bindings.insert("save_as".to_string(), "Ctrl+Shift+S".to_string());
        bindings.insert("open".to_string(), "Ctrl+O".to_string());
        bindings.insert("new_project".to_string(), "Ctrl+N".to_string());
        bindings.insert("export".to_string(), "Ctrl+Shift+E".to_string());
        // Affichage
        bindings.insert("zoom_in".to_string(), "Ctrl+Plus".to_string());
        bindings.insert("zoom_out".to_string(), "Ctrl+Minus".to_string());
        bindings.insert("zoom_fit".to_string(), "Ctrl+0".to_string());
        bindings.insert("zoom_100".to_string(), "Ctrl+1".to_string());
        bindings.insert("toggle_layers_panel".to_string(), "F7".to_string());
        bindings.insert("toggle_tools_panel".to_string(), "Tab".to_string());
        // Calques
        bindings.insert("new_layer".to_string(), "Ctrl+Shift+N".to_string());
        bindings.insert("duplicate_layer".to_string(), "Ctrl+J".to_string());
        // Application
        bindings.insert("open_preferences".to_string(), "Ctrl+,".to_string());

        Self { bindings }
    }
}

impl Default for KeybindingPreferences {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Racine des préférences persistantes d'une app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub version: u32,
    #[serde(default)]
    pub general: GeneralPreferences,
    #[serde(default)]
    pub render: RenderPreferences,
    #[serde(default)]
    pub keybindings: KeybindingPreferences,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            general: GeneralPreferences::default(),
            render: RenderPreferences::default(),
            keybindings: KeybindingPreferences::default(),
        }
    }
}

impl Preferences {
    /// Chemin du fichier de configuration (`<config>/CreativeSuiteOpen/<app>/preferences.json`).
    /// None si la plateforme n'expose pas de dossier de configuration.
    #[must_use]
    pub fn config_path(app: &str) -> Option<PathBuf> {
        dirs::config_dir().map(|p| {
            p.join("CreativeSuiteOpen")
                .join(app)
                .join("preferences.json")
        })
    }

    /// Charge les préférences ; toute anomalie retombe sur les défauts
    /// (jamais de panic : un fichier corrompu ne doit pas empêcher le boot).
    #[must_use]
    pub fn load(app: &str) -> Self {
        let Some(path) = Self::config_path(app) else {
            return Self::default();
        };
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(prefs) => {
                    log::info!("Préférences chargées depuis {}", path.display());
                    prefs
                }
                Err(e) => {
                    log::warn!("Préférences illisibles ({e}) — valeurs par défaut");
                    Self::default()
                }
            },
            Err(e) => {
                log::warn!("Lecture des préférences impossible ({e}) — valeurs par défaut");
                Self::default()
            }
        }
    }

    /// Écrit les préférences sur disque (crée les dossiers parents).
    ///
    /// # Errors
    /// [`PreferencesError`] si le dossier config est introuvable, ou en
    /// cas d'erreur I/O / sérialisation.
    pub fn save(&self, app: &str) -> Result<(), PreferencesError> {
        let path = Self::config_path(app).ok_or(PreferencesError::NoConfigDir)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PreferencesError::Io(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| PreferencesError::Serialization(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| PreferencesError::Io(e.to_string()))?;
        log::info!("Préférences sauvegardées dans {}", path.display());
        Ok(())
    }
}

/// Erreurs de persistance des préférences.
#[derive(Debug, thiserror::Error)]
pub enum PreferencesError {
    #[error("impossible de trouver le dossier de configuration")]
    NoConfigDir,
    #[error("erreur I/O : {0}")]
    Io(String),
    #[error("erreur de sérialisation : {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_aller_retour_conserve_tout() {
        let mut prefs = Preferences::default();
        prefs.general.language = "en".into();
        prefs.render.vsync = false;
        prefs
            .keybindings
            .bindings
            .insert("undo".into(), "Ctrl+Alt+Z".into());

        let json = serde_json::to_string(&prefs).expect("sérialisation");
        let back: Preferences = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(back, prefs);
    }

    #[test]
    fn champs_manquants_tomber_sur_defauts() {
        // Un vieux fichier sans `render` ni `keybindings` doit se charger
        let json = r#"{"version":1,"general":{"language":"fr","theme":"Dark","auto_save":true,"auto_save_interval_secs":300}}"#;
        let prefs: Preferences = serde_json::from_str(json).expect("tolérant");
        assert_eq!(prefs.render, RenderPreferences::default());
        assert_eq!(
            prefs.keybindings.bindings.get("undo").map(String::as_str),
            Some("Ctrl+Z")
        );
    }

    #[test]
    fn api_filtrees_par_plateforme() {
        // Invariants vrais sur TOUTE plateforme de test
        assert!(RenderApi::Auto.is_available_on_current_platform());
        assert!(RenderApi::Gl.is_available_on_current_platform());
        assert_eq!(
            RenderApi::Metal.is_available_on_current_platform(),
            cfg!(target_os = "macos")
        );
        assert_eq!(
            RenderApi::Dx12.is_available_on_current_platform(),
            cfg!(target_os = "windows")
        );
    }
}
