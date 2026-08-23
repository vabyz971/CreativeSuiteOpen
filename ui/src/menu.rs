//! Système de menus générique partagé par toutes les apps.
//! Utilise le widget natif [`iced_aw::DropDown`] : positionnement, ancrage
//! sous le bouton et fermeture au clic extérieur gérés automatiquement.
//!
//! Tous les types sont POSSÉDÉS (String) : l'app peut reconstruire ses menus
//! à chaque vue (labels dynamiques, coches d'état) sans souci de durées de vie.

use crate::theme::{colors, metrics};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};
use iced_aw::DropDown;
use iced_aw::drop_down;

/// Largeur d'un slot de bouton menu
pub const SLOT_WIDTH: f32 = 64.0;
/// Espace entre deux slots
pub const SLOT_GAP: f32 = 2.0;
/// Hauteur de la rangée de menus
pub const BAR_HEIGHT: f32 = 28.0;

/// Un item de menu déroulant
pub enum Item<Message> {
    Action {
        label: String,
        shortcut: String,
        /// État cochable optionnel (préfixe ✓ dans le libellé)
        checked: bool,
        message: Message,
    },
    Separator,
}

impl<Message> Item<Message> {
    /// Action simple sans coche ni raccourci
    pub fn action(label: impl Into<String>, message: Message) -> Self {
        Item::Action {
            label: label.into(),
            shortcut: String::new(),
            checked: false,
            message,
        }
    }
}

/// Un menu complet (bouton + contenu déroulant)
pub struct Menu<Message> {
    pub label: String,
    pub items: Vec<Item<Message>>,
}

impl<Message> Menu<Message> {
    pub fn new(label: impl Into<String>, items: Vec<Item<Message>>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

/// Rangée de boutons menus avec dropdowns natifs ancrés sous chaque bouton.
/// `open` = index du menu actuellement déplié (None = tous fermés).
/// L'élément retourné ne dépend PAS des données d'entrée (tout est possédé).
pub fn bar<'a, Message: Clone + 'a>(
    menus: &[Menu<Message>],
    open: Option<usize>,
    on_toggle: impl Fn(Option<usize>) -> Message + Copy,
) -> Element<'a, Message> {
    let btns: Vec<Element<'a, Message>> = menus
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let is_open = open == Some(i);
            let label = m.label.clone();

            let underlay = button(text(label).size(12).center())
                .width(Length::Fixed(SLOT_WIDTH))
                .height(Length::Fixed(BAR_HEIGHT))
                .style(move |_theme, status| {
                    let mut st = button::Style::default();
                    if is_open || status == button::Status::Hovered {
                        st.background = Some(colors::HOVER_OVERLAY.into());
                        st.text_color = colors::TEXT_PRIMARY;
                    } else {
                        st.background = Some(iced::Color::TRANSPARENT.into());
                        st.text_color = colors::ON_SURFACE;
                    }
                    st.border.radius = metrics::RADIUS_BUTTON.into();
                    st
                })
                .on_press(on_toggle(if is_open { None } else { Some(i) }));

            let items: Vec<Element<'a, Message>> = m
                .items
                .iter()
                .map(|item| match item {
                    Item::Action {
                        label,
                        shortcut,
                        checked,
                        message,
                    } => action_row(label.clone(), *checked, shortcut.clone(), message.clone()),
                    Item::Separator => separator(),
                })
                .collect();

            let overlay = scrollable(
                container(column(items).spacing(1).padding(4))
                    .width(Length::Fixed(240.0))
                    .style(|_theme| container::Style {
                        background: Some(colors::BG_DROPDOWN.into()),
                        border: iced::Border {
                            width: 1.0,
                            color: colors::BORDER_SUBTLE,
                            radius: metrics::RADIUS_DROPDOWN.into(),
                            ..Default::default()
                        },
                        shadow: crate::theme::shadows::dropdown(),
                        ..Default::default()
                    }),
            );

            DropDown::new(underlay, overlay, is_open)
                .width(Length::Fixed(240.0))
                .alignment(drop_down::Alignment::Bottom)
                .on_dismiss(on_toggle(None))
                .into()
        })
        .collect();

    container(row(btns).spacing(SLOT_GAP))
        .height(Length::Fixed(BAR_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

fn action_row<'a, Message: Clone + 'a>(
    mut label: String,
    checked: bool,
    shortcut: String,
    message: Message,
) -> Element<'a, Message> {
    // Coche d'état : préfixe ✓ aligné par espaces
    if checked {
        label.insert_str(0, "✓ ");
    } else {
        label.insert_str(0, "   ");
    }

    let content = row![
        text(label).size(13).color(colors::ON_SURFACE),
        Space::new().width(Length::Fill),
        text(shortcut)
            .size(11)
            .font(iced::Font::MONOSPACE)
            .color(colors::TEXT_MUTED),
    ]
    .align_y(Alignment::Center);

    button(content)
        .width(Length::Fill)
        .padding(iced::Padding::new(6.0).left(10.0).right(8.0))
        .style(move |_theme: &_, status| {
            let mut st = button::Style::default();
            let hovered = status == button::Status::Hovered;
            st.background = Some(if hovered {
                colors::ACCENT.into()
            } else {
                iced::Color::TRANSPARENT.into()
            });
            st.text_color = if hovered {
                colors::TEXT_ON_ACCENT
            } else {
                colors::ON_SURFACE
            };
            st.border.radius = metrics::RADIUS_BUTTON.into();
            st
        })
        .on_press(message)
        .into()
}

fn separator<'a, Message: 'a>() -> Element<'a, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .padding(iced::Padding::new(1.0).left(6.0).right(6.0))
        .style(|_| container::Style {
            background: Some(colors::BORDER_PANEL.into()),
            ..Default::default()
        })
        .into()
}
