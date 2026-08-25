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

//! Menus applicatifs affichés dans le shell (Fichier / Édition / Calque / Affichage).

use crate::message::{Message, PanelType};

/// Menus applicatifs affichés dans le shell (Fichier / Édition / Affichage).
pub fn app_menus(
    tools_visible: bool,
    selected_layer: Option<u64>,
) -> Vec<ui_kit::menu::Menu<Message>> {
    vec![
        ui_kit::menu::Menu::new(
            "Fichier",
            vec![
                ui_kit::menu::Item::Action {
                    label: "Nouveau".into(),
                    shortcut: "Ctrl+N".to_string(),
                    checked: false,
                    message: Message::NewProject,
                },
                ui_kit::menu::Item::Action {
                    label: "Ouvrir...".into(),
                    shortcut: "Ctrl+O".to_string(),
                    checked: false,
                    message: Message::OpenProject,
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::Action {
                    label: "Enregistrer".into(),
                    shortcut: "Ctrl+S".to_string(),
                    checked: false,
                    message: Message::SaveProject,
                },
                ui_kit::menu::Item::Action {
                    label: "Enregistrer sous...".into(),
                    shortcut: "Ctrl+Maj+S".to_string(),
                    checked: false,
                    message: Message::SaveProjectAs,
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::Action {
                    label: "Quitter".into(),
                    shortcut: "Ctrl+Q".to_string(),
                    checked: false,
                    message: Message::Quit,
                },
            ],
        ),
        ui_kit::menu::Menu::new(
            "Édition",
            vec![
                ui_kit::menu::Item::Action {
                    label: "Annuler".into(),
                    shortcut: "Ctrl+Z".to_string(),
                    checked: false,
                    message: Message::Undo,
                },
                ui_kit::menu::Item::Action {
                    label: "Rétablir".into(),
                    shortcut: "Ctrl+Y".to_string(),
                    checked: false,
                    message: Message::Redo,
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::Action {
                    label: "Préférences...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenPreferences,
                },
            ],
        ),
        ui_kit::menu::Menu::new(
            "Calque",
            vec![
                ui_kit::menu::Item::Action {
                    label: "Nouveau calque vide".into(),
                    shortcut: "Ctrl+Maj+N".to_string(),
                    checked: false,
                    message: Message::AddEmptyLayer,
                },
                ui_kit::menu::Item::Action {
                    label: "Calque depuis une image...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenImage,
                },
                ui_kit::menu::Item::Action {
                    label: "Dupliquer le calque".into(),
                    shortcut: "Ctrl+J".to_string(),
                    checked: false,
                    message: Message::DuplicateLayer(selected_layer.unwrap_or(u64::MAX)),
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::Action {
                    label: "Supprimer le calque".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::DeleteLayer(selected_layer.unwrap_or(u64::MAX)),
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::SubMenu {
                    label: "Transformation".into(),
                    items: vec![
                        ui_kit::menu::Item::Action {
                            label: "Rotation 90°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer {
                                id: selected_layer.unwrap_or(u64::MAX),
                                delta: 90.0,
                            },
                        },
                        ui_kit::menu::Item::Action {
                            label: "Rotation 180°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer {
                                id: selected_layer.unwrap_or(u64::MAX),
                                delta: 180.0,
                            },
                        },
                        ui_kit::menu::Item::Action {
                            label: "Rotation -90°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer {
                                id: selected_layer.unwrap_or(u64::MAX),
                                delta: -90.0,
                            },
                        },
                        ui_kit::menu::Item::Action {
                            label: "Rotation -180°".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::RotateLayer {
                                id: selected_layer.unwrap_or(u64::MAX),
                                delta: -180.0,
                            },
                        },
                        ui_kit::menu::Item::Separator,
                        ui_kit::menu::Item::Action {
                            label: "Réinitialiser transformation".into(),
                            shortcut: "".to_string(),
                            checked: false,
                            message: Message::ResetLayerTransform(
                                selected_layer.unwrap_or(u64::MAX),
                            ),
                        },
                    ],
                },
            ],
        ),
        ui_kit::menu::Menu::new(
            "Affichage",
            vec![
                ui_kit::menu::Item::Action {
                    label: "Propriétés".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Properties),
                },
                ui_kit::menu::Item::Action {
                    label: "Calques".into(),
                    shortcut: "F7".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Layers),
                },
                ui_kit::menu::Item::Action {
                    label: "Générateur de textures".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Generator),
                },
                ui_kit::menu::Item::Action {
                    label: "Barre d'outils".into(),
                    shortcut: "Tab".to_string(),
                    checked: tools_visible,
                    message: Message::ToggleToolsPanel,
                },
                ui_kit::menu::Item::Separator,
                ui_kit::menu::Item::Action {
                    label: "Réinitialiser l'interface".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
            ],
        ),
    ]
}
