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

//! Système de raccourcis clavier unifié pour toutes les applications.
//!
//! SOURCE DE VÉRITÉ UNIQUE : pour ajouter/modifier un raccourci par défaut,
//! il suffit d'éditer [`default_bindings`] ci-dessous — rien d'autre.
//!
//! - [`Shortcuts`] : table action → combinaison, modifiable à l'exécution
//! - [`Shortcuts::load`] / [`Shortcuts::save`] : persistance JSON utilisateur
//! - [`subscription`] : écoute clavier (résolution action + mode capture)
//!
//! Utilisable par les 3 apps : `shortcuts::subscription(&app.shortcuts, ...)`
//! puis un simple `match action -> Message` dans l'app.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Actions — identifiants sémantiques partagés par toutes les apps
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    // Fichier
    NewProject,
    Open,
    Save,
    SaveAs,
    Quit,
    // Édition
    Undo,
    Redo,
    Preferences,
    // Affichage
    ToggleTools,
    ToggleLayersPanel,
    TogglePropertiesPanel,
    // Calque
    LayerNew,
    LayerDuplicate,
    LayerDelete,
    LayerMoveUp,
    LayerMoveDown,
    Rotate90,
    Rotate180,
    RotateN90,
    RotateN180,
    ResetTransform,
    CropToSelection,
    // Vue
    ZoomIn,
    ZoomOut,
    FitToScreen,
}

impl Action {
    /// Identifiant de sérialisation stable
    pub fn as_str(self) -> &'static str {
        match self {
            Action::NewProject => "new_project",
            Action::Open => "open",
            Action::Save => "save",
            Action::SaveAs => "save_as",
            Action::Quit => "quit",
            Action::Undo => "undo",
            Action::Redo => "redo",
            Action::Preferences => "preferences",
            Action::ToggleTools => "toggle_tools",
            Action::ToggleLayersPanel => "toggle_layers_panel",
            Action::TogglePropertiesPanel => "toggle_properties_panel",
            Action::LayerNew => "layer_new",
            Action::LayerDuplicate => "layer_duplicate",
            Action::LayerDelete => "layer_delete",
            Action::LayerMoveUp => "layer_move_up",
            Action::LayerMoveDown => "layer_move_down",
            Action::Rotate90 => "rotate_90",
            Action::Rotate180 => "rotate_180",
            Action::RotateN90 => "rotate_n90",
            Action::RotateN180 => "rotate_n180",
            Action::ResetTransform => "reset_transform",
            Action::CropToSelection => "crop_to_selection",
            Action::ZoomIn => "zoom_in",
            Action::ZoomOut => "zoom_out",
            Action::FitToScreen => "fit_to_screen",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "new_project" => Action::NewProject,
            "open" => Action::Open,
            "save" => Action::Save,
            "save_as" => Action::SaveAs,
            "quit" => Action::Quit,
            "undo" => Action::Undo,
            "redo" => Action::Redo,
            "preferences" => Action::Preferences,
            "toggle_tools" => Action::ToggleTools,
            "toggle_layers_panel" => Action::ToggleLayersPanel,
            "toggle_properties_panel" => Action::TogglePropertiesPanel,
            "layer_new" => Action::LayerNew,
            "layer_duplicate" => Action::LayerDuplicate,
            "layer_delete" => Action::LayerDelete,
            "layer_move_up" => Action::LayerMoveUp,
            "layer_move_down" => Action::LayerMoveDown,
            "rotate_90" => Action::Rotate90,
            "rotate_180" => Action::Rotate180,
            "rotate_n90" => Action::RotateN90,
            "rotate_n180" => Action::RotateN180,
            "reset_transform" => Action::ResetTransform,
            "crop_to_selection" => Action::CropToSelection,
            "zoom_in" => Action::ZoomIn,
            "zoom_out" => Action::ZoomOut,
            "fit_to_screen" => Action::FitToScreen,
            _ => return None,
        })
    }

    /// Libellé français affiché dans les préférences
    pub fn label(self) -> &'static str {
        match self {
            Action::NewProject => "Nouveau projet",
            Action::Open => "Ouvrir",
            Action::Save => "Enregistrer",
            Action::SaveAs => "Enregistrer sous",
            Action::Quit => "Quitter",
            Action::Undo => "Annuler",
            Action::Redo => "Rétablir",
            Action::Preferences => "Préférences",
            Action::ToggleTools => "Barre d'outils",
            Action::ToggleLayersPanel => "Panneau Calques",
            Action::TogglePropertiesPanel => "Panneau Propriétés",
            Action::LayerNew => "Nouveau calque",
            Action::LayerDuplicate => "Dupliquer le calque",
            Action::LayerDelete => "Supprimer le calque",
            Action::LayerMoveUp => "Monter le calque",
            Action::LayerMoveDown => "Descendre le calque",
            Action::Rotate90 => "Rotation 90°",
            Action::Rotate180 => "Rotation 180°",
            Action::RotateN90 => "Rotation -90°",
            Action::RotateN180 => "Rotation -180°",
            Action::ResetTransform => "Réinitialiser transformation",
            Action::CropToSelection => "Rogner à la sélection",
            Action::ZoomIn => "Zoom avant",
            Action::ZoomOut => "Zoom arrière",
            Action::FitToScreen => "Ajuster à l'écran",
        }
    }

    /// Catégorie (groupement dans les préférences)
    pub fn category(self) -> &'static str {
        match self {
            Action::NewProject
            | Action::Open
            | Action::Save
            | Action::SaveAs
            | Action::Quit => "Fichier",
            Action::Undo | Action::Redo | Action::Preferences => "Édition",
            Action::ToggleTools | Action::ToggleLayersPanel | Action::TogglePropertiesPanel => {
                "Affichage"
            }
            Action::LayerNew
            | Action::LayerDuplicate
            | Action::LayerDelete
            | Action::LayerMoveUp
            | Action::LayerMoveDown
            | Action::Rotate90
            | Action::Rotate180
            | Action::RotateN90
            | Action::RotateN180
            | Action::ResetTransform
            | Action::CropToSelection => "Calque",
            Action::ZoomIn | Action::ZoomOut | Action::FitToScreen => "Vue",
        }
    }

    /// Toutes les actions, ordonnées par catégorie (pour l'UI préférences)
    pub fn all() -> Vec<Self> {
        let mut all: Vec<Self> = [
            Action::NewProject,
            Action::Open,
            Action::Save,
            Action::SaveAs,
            Action::Quit,
            Action::Undo,
            Action::Redo,
            Action::Preferences,
            Action::ToggleTools,
            Action::ToggleLayersPanel,
            Action::TogglePropertiesPanel,
            Action::LayerNew,
            Action::LayerDuplicate,
            Action::LayerDelete,
            Action::LayerMoveUp,
            Action::LayerMoveDown,
            Action::Rotate90,
            Action::Rotate180,
            Action::RotateN90,
            Action::RotateN180,
            Action::ResetTransform,
            Action::CropToSelection,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::FitToScreen,
        ]
        .into();
        all.sort_by_key(|a| (a.category(), a.label()));
        all
    }
}

// ---------------------------------------------------------------------------
// Binding — une combinaison de touches
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Binding {
    /// Touche normalisée minuscule : "n", "s", "+", "arrowup", "f7", "space"…
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Binding {
    pub fn new(key: &str, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key: key.to_lowercase(),
            ctrl,
            shift,
            alt,
        }
    }

    /// Libellé lisible : "Ctrl+Maj+N"
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.shift {
            parts.push("Maj".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        parts.push(display_key(&self.key));
        parts.join("+")
    }

    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.key == key && self.ctrl == ctrl && self.shift == shift && self.alt == alt
    }
}

fn display_key(key: &str) -> String {
    match key {
        "arrowup" => "↑".into(),
        "arrowdown" => "↓".into(),
        "arrowleft" => "←".into(),
        "arrowright" => "→".into(),
        "space" => "Espace".into(),
        "escape" => "Échap".into(),
        "enter" => "Entrée".into(),
        "tab" => "Tab".into(),
        "+" => "+".into(),
        "-" => "−".into(),
        k if k.len() == 1 => k.to_uppercase(),
        k if k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) => k.to_uppercase(),
        k => {
            let mut c = k.to_string();
            if let Some(first) = c.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            c
        }
    }
}

// ---------------------------------------------------------------------------
// Table par défaut — SOURCE DE VÉRITÉ UNIQUE
// Pour changer un raccourci par défaut : éditer UNE ligne ici.
// ---------------------------------------------------------------------------

fn default_bindings() -> Vec<(Action, Binding)> {
    use Action::*;
    vec![
        // Fichier
        (NewProject, Binding::new("n", true, false, false)),
        (Open, Binding::new("o", true, false, false)),
        (Save, Binding::new("s", true, false, false)),
        (SaveAs, Binding::new("s", true, true, false)),
        (Quit, Binding::new("q", true, false, false)),
        // Édition
        (Undo, Binding::new("z", true, false, false)),
        (Redo, Binding::new("y", true, false, false)),
        // Affichage
        (ToggleTools, Binding::new("tab", false, false, false)),
        (ToggleLayersPanel, Binding::new("f7", false, false, false)),
        (TogglePropertiesPanel, Binding::new("f8", false, false, false)),
        // Calque
        (LayerNew, Binding::new("n", true, true, false)),
        (LayerDuplicate, Binding::new("j", true, false, false)),
        // Vue
        (ZoomIn, Binding::new("+", true, false, false)),
        (ZoomOut, Binding::new("-", true, false, false)),
        (FitToScreen, Binding::new("0", true, false, false)),
        // Sans raccourci par défaut (dangereux ou rare) :
        // LayerDelete, LayerMoveUp/Down, Rotate*, ResetTransform,
        // CropToSelection, Preferences — assignables dans les préférences.
    ]
}

// ---------------------------------------------------------------------------
// Shortcuts — la table modifiable
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Shortcuts {
    map: HashMap<Action, Binding>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Shortcuts {
    /// Table par défaut (une seule source : `default_bindings`)
    pub fn defaults() -> Self {
        Self {
            map: default_bindings().into_iter().collect(),
        }
    }

    pub fn binding(&self, action: Action) -> Option<Binding> {
        self.map.get(&action).cloned()
    }

    /// Libellé lisible de l'action ("Ctrl+N") ou vide
    pub fn label(&self, action: Action) -> String {
        self.binding(action)
            .map(|b| b.label())
            .unwrap_or_default()
    }

    /// Assigne un raccourci. Vole la combinaison à toute autre action qui
    /// l'utilise déjà (comportement Photoshop).
    pub fn set(&mut self, action: Action, binding: Binding) {
        self.map.retain(|a, b| !(*a != action && *b == binding));
        self.map.insert(action, binding);
    }

    /// Remet le raccourci par défaut d'une action
    pub fn reset(&mut self, action: Action) {
        if let Some((_, b)) = default_bindings().into_iter().find(|(a, _)| *a == action) {
            self.map.insert(action, b);
        } else {
            self.map.remove(&action);
        }
    }

    /// Remet toute la table par défaut
    pub fn reset_all(&mut self) {
        self.map = default_bindings().into_iter().collect();
    }

    /// Résout une pression clavier → action
    pub fn find(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Action> {
        self.map
            .iter()
            .find(|(_, b)| b.matches(key, ctrl, shift, alt))
            .map(|(a, _)| *a)
    }

    // -- Persistance JSON (~/.config/creativesuite-open/shortcuts.json) --

    fn config_path() -> Option<std::path::PathBuf> {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))?;
        Some(
            std::path::PathBuf::from(base)
                .join("creativesuite-open")
                .join("shortcuts.json"),
        )
    }

    /// Charge la table utilisateur, complétée par les défauts
    pub fn load() -> Self {
        let mut table = Self::defaults();
        let Some(path) = Self::config_path() else {
            return table;
        };
        let Ok(json) = std::fs::read_to_string(path) else {
            return table;
        };
        #[derive(Deserialize)]
        struct FileBinding {
            key: String,
            #[serde(default)]
            ctrl: bool,
            #[serde(default)]
            shift: bool,
            #[serde(default)]
            alt: bool,
        }
        let Ok(entries) = serde_json::from_str::<HashMap<String, FileBinding>>(&json) else {
            return table;
        };
        for (name, fb) in entries {
            if let Some(action) = Action::from_str(&name) {
                table.map.insert(
                    action,
                    Binding {
                        key: fb.key.to_lowercase(),
                        ctrl: fb.ctrl,
                        shift: fb.shift,
                        alt: fb.alt,
                    },
                );
            }
        }
        table
    }

    /// Sauvegarde la table utilisateur (silencieux en cas d'échec disque)
    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        #[derive(Serialize)]
        struct FileBinding {
            key: String,
            ctrl: bool,
            shift: bool,
            alt: bool,
        }
        let entries: HashMap<String, FileBinding> = self
            .map
            .iter()
            .map(|(a, b)| {
                (
                    a.as_str().to_string(),
                    FileBinding {
                        key: b.key.clone(),
                        ctrl: b.ctrl,
                        shift: b.shift,
                        alt: b.alt,
                    },
                )
            })
            .collect();
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(&entries)
        {
            let _ = std::fs::write(path, json);
        }
    }
}

// ---------------------------------------------------------------------------
// Subscription clavier
// ---------------------------------------------------------------------------

/// Convertit un `iced::keyboard::Key` en clé normalisée de [`Binding`].
pub fn key_string(key: &iced::keyboard::Key) -> Option<String> {
    use iced::keyboard::{Key, key::Named};
    match key {
        Key::Character(c) => c.chars().next().map(|ch| ch.to_lowercase().to_string()),
        Key::Named(n) => {
            let s = match n {
                Named::Space => "space",
                Named::ArrowUp => "arrowup",
                Named::ArrowDown => "arrowdown",
                Named::ArrowLeft => "arrowleft",
                Named::ArrowRight => "arrowright",
                Named::Enter => "enter",
                Named::Tab => "tab",
                Named::Escape => "escape",
                Named::Backspace => "backspace",
                Named::Delete => "delete",
                Named::Home => "home",
                Named::End => "end",
                Named::PageUp => "pageup",
                Named::PageDown => "pagedown",
                Named::F1 => "f1",
                Named::F2 => "f2",
                Named::F3 => "f3",
                Named::F4 => "f4",
                Named::F5 => "f5",
                Named::F6 => "f6",
                Named::F7 => "f7",
                Named::F8 => "f8",
                Named::F9 => "f9",
                Named::F10 => "f10",
                Named::F11 => "f11",
                Named::F12 => "f12",
                other => return Some(format!("{other:?}").to_lowercase()),
            };
            Some(s.to_string())
        }
        Key::Unidentified => None,
    }
}

/// Écoute clavier global :
/// - mode capture (`capturing == true`) : la prochaine touche devient le
///   nouveau raccourci → `on_captured(Option<binding>)` (None = Échap/annule)
/// - sinon : résout l'action et appelle `on_action`
pub fn subscription<Message: Clone + Send + 'static>(
    shortcuts: &Shortcuts,
    capturing: bool,
    on_action: impl Fn(Action) -> Option<Message> + Copy + Send + 'static,
    on_captured: impl Fn(Option<Binding>) -> Message + Copy + Send + 'static,
) -> iced::Subscription<Message> {
    let table = shortcuts.clone();
    iced::keyboard::listen().filter_map(move |event| {
        let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let Some(key_str) = key_string(&key) else {
            return None;
        };
        // Ignore les touches modificateurs seules pendant la capture
        if matches!(
            key_str.as_str(),
            "control" | "shift" | "alt" | "meta" | "super"
        ) {
            return None;
        }
        let (ctrl, shift, alt) = (modifiers.control(), modifiers.shift(), modifiers.alt());
        if capturing {
            // Échap annule la capture
            if key_str == "escape" && !ctrl && !shift && !alt {
                return Some(on_captured(None));
            }
            return Some(on_captured(Some(Binding::new(&key_str, ctrl, shift, alt))));
        }
        table.find(&key_str, ctrl, shift, alt).and_then(on_action)
    })
}
