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

//! Options applicatives persistées — structure JSON unique partagée par
//! toutes les apps. Fichier : `~/.config/creativesuite-open/settings.json`
//!
//! ```json
//! {
//!   "shortcuts": { "open": { "key": "o", "ctrl": true, "shift": false, "alt": false } }
//! }
//! ```
//!
//! Ajouter une option = ajouter un champ ici + une section dans le fichier.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// Raccourcis clavier : nom d'action → combinaison
    #[serde(default)]
    pub shortcuts: HashMap<String, ShortcutJson>,
    /// Options générales (extensible — nouvelles sections = nouveaux champs
    /// avec `#[serde(default)]` pour la rétrocompatibilité)
    #[serde(default)]
    pub general: GeneralSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShortcutJson {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Afficher la barre d'outils flottante
    #[serde(default = "default_true")]
    pub show_tools: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self { show_tools: true }
    }
}

fn default_true() -> bool {
    true
}

impl AppSettings {
    fn path() -> Option<std::path::PathBuf> {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))?;
        Some(
            std::path::PathBuf::from(base)
                .join("creativesuite-open")
                .join("settings.json"),
        )
    }

    /// Charge les options utilisateur (fichier absent/corrompu → défauts)
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Sauvegarde (silencieux en cas d'échec disque)
    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(path, json);
        }
    }
}
