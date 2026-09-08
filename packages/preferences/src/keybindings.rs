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

//! Résolveur de raccourcis clavier : actions typées de l'app photo,
//! parsing des combinaisons (« Ctrl+Shift+S ») et conversion d'un
//! événement clavier iced en action.

use std::collections::HashMap;

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

/// Toutes les actions raccourcissables de l'app photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhotoAction {
    // Outils
    ToolBrush,
    ToolEraser,
    ToolEyedropper,
    ToolMove,
    ToolHand,
    ToolZoom,
    // Édition
    Undo,
    Redo,
    DeleteLayer,
    // Fichier
    NewProject,
    Open,
    Save,
    SaveAs,
    Export,
    // Affichage
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Zoom100,
    ToggleLayersPanel,
    ToggleToolsPanel,
    // Calques
    NewLayer,
    DuplicateLayer,
    // Application
    OpenPreferences,
}

impl PhotoAction {
    /// Toutes les actions, ordre d'affichage stable dans la fenêtre.
    pub const ALL: [PhotoAction; 23] = [
        PhotoAction::ToolBrush,
        PhotoAction::ToolEraser,
        PhotoAction::ToolEyedropper,
        PhotoAction::ToolMove,
        PhotoAction::ToolHand,
        PhotoAction::ToolZoom,
        PhotoAction::Undo,
        PhotoAction::Redo,
        PhotoAction::DeleteLayer,
        PhotoAction::NewProject,
        PhotoAction::Open,
        PhotoAction::Save,
        PhotoAction::SaveAs,
        PhotoAction::Export,
        PhotoAction::ZoomIn,
        PhotoAction::ZoomOut,
        PhotoAction::ZoomFit,
        PhotoAction::Zoom100,
        PhotoAction::ToggleLayersPanel,
        PhotoAction::ToggleToolsPanel,
        PhotoAction::NewLayer,
        PhotoAction::DuplicateLayer,
        PhotoAction::OpenPreferences,
    ];

    /// Identifiant sérialisé (clé de la table de bindings).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::ToolBrush => "tool_brush",
            Self::ToolEraser => "tool_eraser",
            Self::ToolEyedropper => "tool_eyedropper",
            Self::ToolMove => "tool_move",
            Self::ToolHand => "tool_hand",
            Self::ToolZoom => "tool_zoom",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::DeleteLayer => "delete_layer",
            Self::NewProject => "new_project",
            Self::Open => "open",
            Self::Save => "save",
            Self::SaveAs => "save_as",
            Self::Export => "export",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ZoomFit => "zoom_fit",
            Self::Zoom100 => "zoom_100",
            Self::ToggleLayersPanel => "toggle_layers_panel",
            Self::ToggleToolsPanel => "toggle_tools_panel",
            Self::NewLayer => "new_layer",
            Self::DuplicateLayer => "duplicate_layer",
            Self::OpenPreferences => "open_preferences",
        }
    }

    /// Libellé affiché à l'utilisateur.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ToolBrush => "Outil Pinceau",
            Self::ToolEraser => "Outil Gomme",
            Self::ToolEyedropper => "Pipette",
            Self::ToolMove => "Déplacement",
            Self::ToolHand => "Main",
            Self::ToolZoom => "Zoom",
            Self::Undo => "Annuler",
            Self::Redo => "Rétablir",
            Self::DeleteLayer => "Supprimer le calque",
            Self::NewProject => "Nouveau projet",
            Self::Open => "Ouvrir",
            Self::Save => "Enregistrer",
            Self::SaveAs => "Enregistrer sous",
            Self::Export => "Exporter l'image",
            Self::ZoomIn => "Zoom avant",
            Self::ZoomOut => "Zoom arrière",
            Self::ZoomFit => "Ajuster à l'écran",
            Self::Zoom100 => "Zoom 100 %",
            Self::ToggleLayersPanel => "Panneau Calques",
            Self::ToggleToolsPanel => "Barre d'outils",
            Self::NewLayer => "Nouveau calque",
            Self::DuplicateLayer => "Dupliquer le calque",
            Self::OpenPreferences => "Préférences",
        }
    }

    /// Catégorie pour le groupement visuel.
    #[must_use]
    pub fn category(self) -> &'static str {
        match self {
            Self::ToolBrush
            | Self::ToolEraser
            | Self::ToolEyedropper
            | Self::ToolMove
            | Self::ToolHand
            | Self::ToolZoom => "Outils",
            Self::Undo | Self::Redo | Self::DeleteLayer => "Édition",
            Self::NewProject | Self::Open | Self::Save | Self::SaveAs | Self::Export => "Fichier",
            Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomFit
            | Self::Zoom100
            | Self::ToggleLayersPanel
            | Self::ToggleToolsPanel => "Affichage",
            Self::NewLayer | Self::DuplicateLayer => "Calques",
            Self::OpenPreferences => "Application",
        }
    }

    /// Retrouve l'action depuis son identifiant sérialisé.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.id() == id)
    }
}

/// Combinaison de touches normalisée (« Ctrl+Shift+S »).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    /// Touche principale NORMALISÉE EN MAJUSCULE (« S », « F7 », « Delete »)
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl std::fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::with_capacity(4);
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.as_str());
        write!(f, "{}", parts.join("+"))
    }
}

/// Résolveur : table combo → action, construite depuis les préférences.
#[derive(Debug, Default)]
pub struct KeybindingResolver {
    lookup: HashMap<KeyCombo, PhotoAction>,
}

impl KeybindingResolver {
    /// Construit le résolveur depuis la table id→combinaison.
    #[must_use]
    pub fn from_bindings(bindings: &HashMap<String, String>) -> Self {
        let mut resolver = Self::default();
        for (action_id, combo_str) in bindings {
            if let Some(action) = PhotoAction::from_id(action_id)
                && let Some(combo) = parse_combo(combo_str)
            {
                resolver.lookup.insert(combo, action);
            }
        }
        resolver
    }

    /// Action correspondant à cet événement clavier, s'il y en a une.
    #[must_use]
    pub fn resolve(&self, key: &Key, modifiers: Modifiers) -> Option<PhotoAction> {
        let combo = KeyCombo {
            key: key_to_string(key)?,
            ctrl: modifiers.control() || modifiers.command(),
            shift: modifiers.shift(),
            alt: modifiers.alt(),
        };
        self.lookup.get(&combo).copied()
    }

    /// Nombre de combinaisons actives (diagnostic).
    #[must_use]
    pub fn len(&self) -> usize {
        self.lookup.len()
    }

    /// Toujours vrai : un résolveur vide est valide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lookup.is_empty()
    }
}

/// Parse « Ctrl+Shift+S » / « F7 » / « Delete » en [`KeyCombo`] normalisé.
/// Tolère la casse et les alias (« Control », « Cmd », « Option »).
#[must_use]
pub fn parse_combo(s: &str) -> Option<KeyCombo> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut key = String::new();

    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "meta" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            other => key = other.to_uppercase(),
        }
    }

    if key.is_empty() {
        return None;
    }
    Some(KeyCombo {
        key,
        ctrl,
        shift,
        alt,
    })
}

/// Convertit une touche iced en sa représentation texte normalisée
/// (identique à celle utilisée par [`parse_combo`]).
#[must_use]
pub fn key_to_string(key: &Key) -> Option<String> {
    match key {
        Key::Character(c) => Some(c.to_uppercase()),
        Key::Named(named) => named_to_string(*named),
        Key::Unidentified => None,
    }
}

fn named_to_string(named: Named) -> Option<String> {
    let s = match named {
        Named::F1 => "F1",
        Named::F2 => "F2",
        Named::F3 => "F3",
        Named::F4 => "F4",
        Named::F5 => "F5",
        Named::F6 => "F6",
        Named::F7 => "F7",
        Named::F8 => "F8",
        Named::F9 => "F9",
        Named::F10 => "F10",
        Named::F11 => "F11",
        Named::F12 => "F12",
        Named::Space => "Space",
        Named::Enter => "Enter",
        Named::Escape => "Escape",
        Named::Delete => "Delete",
        Named::Backspace => "Backspace",
        Named::Tab => "Tab",
        Named::ArrowUp => "Up",
        Named::ArrowDown => "Down",
        Named::ArrowLeft => "Left",
        Named::ArrowRight => "Right",
        _ => return None,
    };
    Some(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_coherence_sur_toutes_les_actions() {
        for action in PhotoAction::ALL {
            assert_eq!(
                PhotoAction::from_id(action.id()),
                Some(action),
                "from_id(id()) doit être l'identité"
            );
            assert!(!action.label().is_empty());
            assert!(!action.category().is_empty());
        }
    }

    #[test]
    fn parsing_tolerant_a_la_casse_et_aux_alias() {
        let c = parse_combo("ctrl+shift+s").expect("parse");
        assert!(c.ctrl && c.shift && !c.alt);
        assert_eq!(c.key, "S");

        let c = parse_combo("Control + Minus").expect("parse");
        assert!(c.ctrl);
        assert_eq!(c.key, "MINUS");

        assert!(parse_combo("Ctrl+").is_none(), "touche vide rejetée");
    }

    #[test]
    fn display_reconstruit_la_combinaison() {
        let c = KeyCombo {
            key: "S".into(),
            ctrl: true,
            shift: true,
            alt: false,
        };
        assert_eq!(c.to_string(), "Ctrl+Shift+S");
    }

    #[test]
    fn resolve_trouve_les_raccourcis_par_defaut() {
        let defaults = crate::model::KeybindingPreferences::with_defaults();
        let resolver = KeybindingResolver::from_bindings(&defaults.bindings);

        // Ctrl+Z → Undo
        let z = Key::Character("z".into());
        let mods = Modifiers::CTRL;
        assert_eq!(resolver.resolve(&z, mods), Some(PhotoAction::Undo));

        // 'b' sans modificateur → ToolBrush
        let b = Key::Character("b".into());
        assert_eq!(
            resolver.resolve(&b, Modifiers::empty()),
            Some(PhotoAction::ToolBrush)
        );

        // Ctrl seul ne déclenche rien
        assert_eq!(resolver.resolve(&Key::Named(Named::Control), mods), None);

        // F7 → panneau calques
        assert_eq!(
            resolver.resolve(&Key::Named(Named::F7), Modifiers::empty()),
            Some(PhotoAction::ToggleLayersPanel)
        );
    }

    #[test]
    fn resolution_sensible_aux_modificateurs() {
        let defaults = crate::model::KeybindingPreferences::with_defaults();
        let resolver = KeybindingResolver::from_bindings(&defaults.bindings);
        // 's' SANS Ctrl ne doit PAS déclencher Enregistrer
        let s = Key::Character("s".into());
        assert_ne!(
            resolver.resolve(&s, Modifiers::empty()),
            Some(PhotoAction::Save)
        );
        assert_eq!(
            resolver.resolve(&s, Modifiers::CTRL),
            Some(PhotoAction::Save)
        );
    }
}
