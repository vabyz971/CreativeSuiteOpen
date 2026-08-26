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

//! Fenêtre de préférences FLOTTANTE (non bloquante, façon Photoshop/Affinity).
//!
//! Affichée en surcouche `stack!` SANS scrim : les clics en dehors du
//! panneau atteignent le contenu principal — la fenêtre ne se ferme que
//! par son bouton Fermer ou Échap pendant une capture de touche.
//!
//! Sections : Général, Apparence, Rendu, Hardware, Raccourcis, À propos.
//! Les réglages vivent dans un BROUILLON ([`Preferences`]) : « Appliquer »
//! persiste sans fermer, « OK » persiste et ferme, « Fermer » jette les
//! modifications non appliquées.

use iced::widget::{button, column, container, pick_list, row, scrollable, slider, text, toggler};
use iced::{Alignment, Element, Length, Padding};

fn hspace() -> iced::widget::Space {
    iced::widget::Space::new()
        .width(Length::Fill)
        .height(Length::Fixed(0.0))
}
use preferences::{
    HardwareReport, PhotoAction, Preferences, RenderApi, RenderQuality, Theme, key_to_string,
};
use ui_kit::theme::colors;

/// Sections de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Appearance,
    Render,
    Hardware,
    Keybindings,
    About,
}

const SIDEBAR_WIDTH: f32 = 190.0;

/// Messages internes de la fenêtre de préférences.
#[derive(Debug, Clone)]
pub enum Message {
    Section(Section),
    SetLanguage(String),
    SetTheme(Theme),
    SetAutoSave(bool),
    SetAutoSaveInterval(u32),
    SetRenderApi(RenderApi),
    SetRenderQuality(RenderQuality),
    SetVsync(bool),
    SetGpuCacheLimit(u32),
    /// Démarre la capture d'une nouvelle combinaison pour l'action donnée
    StartCapture(String),
    /// Touche pressée pendant une capture (routée depuis l'abonnement global)
    KeyCaptured {
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    },
    CancelCapture,
    ResetBinding(String),
    ResetDefaults,
    Apply,
    SaveAndClose,
    Close,
}

pub struct PreferencesWindow {
    pub draft: Preferences,
    pub hardware: Option<HardwareReport>,
    section: Section,
    capturing: Option<String>,
    status: Option<String>,
}

impl PreferencesWindow {
    #[must_use]
    pub fn new(current: Preferences) -> Self {
        Self {
            draft: current,
            hardware: None,
            section: Section::General,
            capturing: None,
            status: None,
        }
    }

    /// Injecte le rapport matériel arrivé async.
    pub fn set_hardware(&mut self, report: HardwareReport) {
        self.hardware = Some(report);
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Section(s) => self.section = s,
            Message::SetLanguage(l) => self.draft.general.language = l,
            Message::SetTheme(t) => self.draft.general.theme = t,
            Message::SetAutoSave(b) => self.draft.general.auto_save = b,
            Message::SetAutoSaveInterval(v) => self.draft.general.auto_save_interval_secs = v,
            Message::SetRenderApi(a) => self.draft.render.api = a,
            Message::SetRenderQuality(q) => self.draft.render.quality = q,
            Message::SetVsync(b) => self.draft.render.vsync = b,
            Message::SetGpuCacheLimit(v) => self.draft.render.gpu_cache_limit_mb = v,

            Message::StartCapture(action_id) => {
                self.capturing = Some(action_id);
                self.status =
                    Some("Appuyez sur la nouvelle combinaison (Échap pour annuler)".into());
            }
            Message::KeyCaptured { key, modifiers } => {
                let Some(action_id) = self.capturing.take() else {
                    return;
                };
                // Échap annule la capture au lieu de s'affecter
                if matches!(
                    key,
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                ) {
                    self.status = Some("Capture annulée".into());
                    return;
                }
                let Some(key_str) = key_to_string(&key) else {
                    self.status = Some("Touche non assignable".into());
                    return;
                };
                let combo = format_combo(&key_str, modifiers);
                // Détection de conflit : la même combinaison sur autre action ?
                let conflict = PhotoAction::ALL.iter().find(|a| {
                    a.id() != action_id
                        && self.draft.keybindings.bindings.get(a.id()) == Some(&combo)
                });
                if let Some(other) = conflict {
                    self.status = Some(format!(
                        "Déjà utilisé par « {} » — combinaison ignorée",
                        other.label()
                    ));
                    return;
                }
                self.draft
                    .keybindings
                    .bindings
                    .insert(action_id.clone(), combo.clone());
                self.status = Some(format!("{action_id} → {combo}"));
            }
            Message::CancelCapture => {
                self.capturing = None;
                self.status = None;
            }
            Message::ResetDefaults => {
                self.draft = Preferences::default();
                self.status = Some("Valeurs par défaut restaurées".into());
            }
            Message::ResetBinding(action_id) => {
                let defaults = Preferences::default();
                if let Some(combo) = defaults.keybindings.bindings.get(&action_id) {
                    self.draft
                        .keybindings
                        .bindings
                        .insert(action_id, combo.clone());
                    self.status = Some(format!("Raccourci réinitialisé ({combo})"));
                }
            }
            Message::Apply => match self.draft.save("photo") {
                Ok(()) => self.status = Some("Préférences appliquées".into()),
                Err(e) => self.status = Some(format!("Échec : {e}")),
            },
            Message::SaveAndClose => {
                let _ = self.draft.save("photo");
            }
            Message::Close => {}
        }
    }

    /// La fenêtre consomme-t-elle les touches clavier actuellement ?
    /// Vrai pendant une capture : l'app doit router vers [`Self::key_event`].
    #[must_use]
    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some()
    }

    /// Route une touche vers la capture en cours. Retourne true si consommée.
    pub fn key_event(
        &mut self,
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> bool {
        if self.capturing.is_none() {
            return false;
        }
        // Ignore les relâchements : on ne traite que les pressions,
        // filtrées en amont par l'abonnement.
        self.update(Message::KeyCaptured { key, modifiers });
        true
    }

    // ------------------------------------------------------------------ view

    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = scrollable(container(self.view_content()).padding(16))
            .width(Length::Fill)
            .height(Length::Fill);

        let layout = row![
            sidebar,
            // Séparateur vertical (iced 0.14 n'a pas de Rule::vertical)
            container(iced::widget::Space::new().width(1).height(Length::Fill))
                .width(Length::Fixed(1.0))
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(colors::BORDER_PANEL.into()),
                    ..Default::default()
                }),
            container(content).width(Length::Fill).height(Length::Fill),
        ]
        .height(Length::Fill);

        container(column![layout, self.view_footer()].height(Length::Fill))
            .width(Length::Fixed(760.0))
            .height(Length::Fixed(560.0))
            .style(|_| {
                ui_kit::style::floating_card(
                    colors::SURFACE_CONTAINER_LOW,
                    ui_kit::theme::metrics::RADIUS_DROPDOWN,
                    ui_kit::theme::shadows::panel(),
                )
            })
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        const SECTIONS: [(Section, &str); 6] = [
            (Section::General, "Général"),
            (Section::Appearance, "Apparence"),
            (Section::Render, "Rendu"),
            (Section::Hardware, "Hardware"),
            (Section::Keybindings, "Raccourcis"),
            (Section::About, "À propos"),
        ];
        let mut col = column![].spacing(2).padding(8);
        for (section, label) in SECTIONS {
            col = col.push(
                button(text(label).size(13).color(if section == self.section {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                }))
                .width(Length::Fill)
                .padding(Padding::new(8.0).top(6.0).bottom(6.0))
                .style(move |_t, s| ui_kit::style::ghost_selected(section == self.section, s))
                .on_press(Message::Section(section)),
            );
        }
        container(col)
            .width(Length::Fixed(SIDEBAR_WIDTH))
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::BG_MENU_BAR.into()),
                border: iced::Border {
                    width: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        match self.section {
            Section::General => self.view_general(),
            Section::Appearance => self.view_appearance(),
            Section::Render => self.view_render(),
            Section::Hardware => self.view_hardware(),
            Section::Keybindings => self.view_keybindings(),
            Section::About => self.view_about(),
        }
    }

    fn title<'a>(label: &'a str) -> iced::widget::Text<'a> {
        text(label.to_string())
            .size(18)
            .font(ui_kit::theme::fonts::SANS_SEMIBOLD)
            .color(colors::TEXT_PRIMARY)
    }

    fn field_label(label: &str) -> iced::widget::Text<'_> {
        text(label.to_string()).size(12).color(colors::TEXT_MUTED)
    }

    fn view_general(&self) -> Element<'_, Message> {
        let languages = ["fr".to_string(), "en".to_string()];
        column![
            Self::title("Général"),
            Self::field_label("Langue de l'interface"),
            pick_list(languages, Some(self.draft.general.language.clone()), |l| {
                Message::SetLanguage(l)
            })
            .width(Length::Fixed(160.0)),
            row![
                text("Sauvegarde automatique")
                    .size(13)
                    .color(colors::TEXT_PRIMARY),
                hspace(),
                toggler(self.draft.general.auto_save).on_toggle(Message::SetAutoSave),
            ]
            .width(Length::Fill)
            .align_y(Alignment::Center),
            Self::field_label("Intervalle (secondes)"),
            slider(
                60..=1800,
                self.draft.general.auto_save_interval_secs,
                Message::SetAutoSaveInterval,
            )
            .step(60_u32),
            text(format!(
                "{} min",
                self.draft.general.auto_save_interval_secs / 60
            ))
            .size(11)
            .color(colors::TEXT_MUTED),
        ]
        .spacing(10)
        .into()
    }

    fn view_appearance(&self) -> Element<'_, Message> {
        column![
            Self::title("Apparence"),
            Self::field_label("Thème"),
            pick_list(
                Theme::ALL,
                Some(self.draft.general.theme),
                Message::SetTheme,
            )
            .width(Length::Fixed(160.0)),
            text("Seul le thème sombre est rendu aujourd'hui ; les autres sont mémorisés.")
                .size(11)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(10)
        .into()
    }

    fn view_render(&self) -> Element<'_, Message> {
        let apis: Vec<RenderApi> = RenderApi::ALL
            .iter()
            .copied()
            .filter(|api| api.is_available_on_current_platform())
            .collect();
        column![
            Self::title("Rendu"),
            Self::field_label("API graphique (nécessite un redémarrage)"),
            pick_list(apis, Some(self.draft.render.api), Message::SetRenderApi,)
                .width(Length::Fixed(200.0)),
            Self::field_label("Profil de qualité"),
            pick_list(
                RenderQuality::ALL,
                Some(self.draft.render.quality),
                Message::SetRenderQuality,
            )
            .width(Length::Fixed(200.0)),
            row![
                text("VSync").size(13).color(colors::TEXT_PRIMARY),
                hspace(),
                toggler(self.draft.render.vsync).on_toggle(Message::SetVsync),
            ]
            .width(Length::Fill)
            .align_y(Alignment::Center),
            Self::field_label("Limite du cache GPU (Mo)"),
            slider(
                256..=8192,
                self.draft.render.gpu_cache_limit_mb,
                Message::SetGpuCacheLimit,
            )
            .step(256_u32),
            text(format!("{} Mo", self.draft.render.gpu_cache_limit_mb))
                .size(11)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(10)
        .into()
    }

    fn view_hardware(&self) -> Element<'_, Message> {
        let mut col = column![Self::title("Hardware")].spacing(10);
        match &self.hardware {
            Some(report) => {
                if let Some(cpu) = &report.cpu {
                    col = col.push(field_row(
                        "CPU",
                        format!("{} · {} cœurs", cpu.name, cpu.cores),
                    ));
                }
                col = col.push(field_row(
                    "RAM",
                    if report.ram.total_mb > 0 {
                        format!("{} Mo", report.ram.total_mb)
                    } else {
                        "inconnue".to_string()
                    },
                ));
                col = col.push(Self::field_label("Adaptateurs GPU"));
                if report.gpus.is_empty() {
                    col = col.push(
                        text("Aucun adaptateur détecté")
                            .size(12)
                            .color(colors::TEXT_MUTED),
                    );
                }
                for gpu in &report.gpus {
                    col = col.push(
                        container(
                            column![
                                text(format!(
                                    "{}{}",
                                    if gpu.is_discrete { "[Dédicace] " } else { "" },
                                    gpu.name
                                ))
                                .size(13)
                                .color(colors::TEXT_PRIMARY),
                                text(format!("API {} · pilote {}", gpu.api, gpu.driver))
                                    .size(11)
                                    .color(colors::TEXT_MUTED),
                            ]
                            .spacing(2),
                        )
                        .padding(8)
                        .width(Length::Fill)
                        .style(|_| {
                            ui_kit::style::inset_card(
                                colors::SURFACE_CONTAINER_LOWEST,
                                ui_kit::theme::metrics::RADIUS_SM,
                            )
                        }),
                    );
                }
            }
            None => {
                col = col.push(
                    text("Détection en cours…")
                        .size(12)
                        .color(colors::TEXT_MUTED),
                );
            }
        }
        col.into()
    }

    fn view_keybindings(&self) -> Element<'_, Message> {
        let mut col = column![
            Self::title("Raccourcis clavier"),
            text("Cliquez sur une combinaison pour la modifier, puis appuyez sur les touches.")
                .size(11)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(6);

        // Groupement stable par catégorie
        let mut actions: Vec<PhotoAction> = PhotoAction::ALL.into();
        actions.sort_by_key(|a| (a.category(), a.label()));
        let mut current_category = "";
        for action in actions {
            if action.category() != current_category {
                current_category = action.category();
                col = col.push(
                    text(current_category.to_string())
                        .size(13)
                        .font(ui_kit::theme::fonts::SANS_SEMIBOLD)
                        .color(colors::ON_SURFACE),
                );
            }
            let combo = self
                .draft
                .keybindings
                .bindings
                .get(action.id())
                .cloned()
                .unwrap_or_else(|| "—".into());
            let is_capturing = self.capturing.as_deref() == Some(action.id());

            col = col.push(
                row![
                    text(action.label().to_string())
                        .size(12)
                        .color(colors::TEXT_PRIMARY)
                        .width(Length::Fixed(200.0)),
                    button(
                        text(if is_capturing {
                            "Appuyez sur une touche…".to_string()
                        } else {
                            combo
                        })
                        .size(12)
                        .color(if is_capturing {
                            colors::ACCENT
                        } else {
                            colors::TEXT_SECONDARY
                        }),
                    )
                    .width(Length::Fixed(220.0))
                    .padding(Padding::new(6.0).top(3.0).bottom(3.0))
                    .style(move |_t, s| ui_kit::style::ghost_selected(is_capturing, s))
                    .on_press(Message::StartCapture(action.id().to_string())),
                    button(text("Réinitialiser").size(11).color(colors::TEXT_MUTED),)
                        .padding(Padding::new(6.0).top(3.0).bottom(3.0))
                        .style(|_t, s| ui_kit::style::ghost(s))
                        .on_press(Message::ResetBinding(action.id().to_string())),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        col.into()
    }

    fn view_about(&self) -> Element<'_, Message> {
        column![
            Self::title("CreativeSuiteOpen — Photo"),
            field_row("Version", env!("CARGO_PKG_VERSION").to_string()),
            field_row("Licence", "GNU GPL v3".to_string()),
            field_row("Moteur", "Rust + Iced + wgpu".to_string()),
            text("https://github.com/vabyz971/CreativeSuiteOpen")
                .size(12)
                .color(colors::ACCENT),
        ]
        .spacing(10)
        .into()
    }

    fn view_footer(&self) -> Element<'_, Message> {
        let status = match (&self.status, &self.capturing) {
            (_, Some(_)) => text("Appuyez sur une touche…")
                .size(11)
                .color(colors::ACCENT),
            (Some(s), None) => text(s.clone()).size(11).color(colors::TEXT_MUTED),
            (None, None) => text("").size(11),
        };
        container(
            row![
                status,
                hspace(),
                footer_button("Réinitialiser", Message::ResetDefaults),
                footer_button("Fermer", Message::Close),
                footer_button("Appliquer", Message::Apply),
                footer_button_primary("OK", Message::SaveAndClose),
            ]
            .spacing(8),
        )
        .width(Length::Fill)
        .padding(Padding::new(10.0).left(14.0).right(14.0))
        .style(|_| container::Style {
            background: Some(colors::BG_MENU_BAR.into()),
            border: iced::Border {
                width: 1.0,
                color: colors::BORDER_PANEL,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    }
}

fn footer_button<'a>(label: &'static str, msg: Message) -> iced::widget::Button<'a, Message> {
    button(text(label.to_string()).size(12))
        .padding(Padding::new(10.0).top(5.0).bottom(5.0))
        .style(|_t, s| ui_kit::style::ghost(s))
        .on_press(msg)
}

fn footer_button_primary<'a>(
    label: &'static str,
    msg: Message,
) -> iced::widget::Button<'a, Message> {
    button(text(label.to_string()).size(12))
        .padding(Padding::new(12.0).top(5.0).bottom(5.0))
        .style(|_t, s| ui_kit::style::primary(s))
        .on_press(msg)
}

fn field_row(label: &str, value: String) -> Element<'_, Message> {
    row![
        text(format!("{label} :"))
            .size(12)
            .color(colors::TEXT_MUTED)
            .width(Length::Fixed(90.0)),
        text(value).size(12).color(colors::TEXT_PRIMARY),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// Construit « Ctrl+Shift+Alt+S » à partir des modificateurs iced.
fn format_combo(key: &str, modifiers: iced::keyboard::Modifiers) -> String {
    let mut parts = Vec::with_capacity(4);
    if modifiers.control() || modifiers.command() {
        parts.push("Ctrl");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    parts.push(key);
    parts.join("+")
}
