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

use crate::theme::{colors, fonts, metrics};
use iced::widget::pane_grid;
use iced::widget::{container, row, text};
use iced::{Alignment, Element, Padding};
use iced_aw::ContextMenu;

/// Renders a pane-grid panel with title bar and content.
///
/// `close_menu` provides an optional context-menu action for closing the panel.
#[must_use]
pub fn render<'a, Message>(
    title: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
    is_focused: bool,
    close_menu: Option<Message>, // clic droit sur le titre → "Fermer le panneau"
) -> pane_grid::Content<'a, Message>
where
    Message: Clone + 'a,
{
    let title_row = row![
        text(title.into())
            .size(14)
            .font(fonts::SANS_SEMIBOLD)
            .style(move |_theme| iced::widget::text::Style {
                color: Some(if is_focused {
                    colors::TEXT_PRIMARY
                } else {
                    colors::TEXT_SECONDARY
                })
            }),
    ]
    .align_y(Alignment::Center)
    .spacing(5);

    let title_area: Element<'_, Message> = match close_menu {
        // Native ContextMenu: opens on right-click, positioned at cursor
        Some(msg) => ContextMenu::new(title_row, move || {
            crate::dropdown::menu_item("Fermer le panneau", "", msg.clone())
        })
        .into(),
        None => title_row.into(),
    };

    let title_bar = pane_grid::TitleBar::new(title_area)
        .padding(Padding::new(6.0).left(8.0).right(8.0))
        .style(if is_focused {
            style_title_focused
        } else {
            style_title_active
        });

    pane_grid::Content::new(content.into())
        .title_bar(title_bar)
        .style(if is_focused {
            style_pane_focused
        } else {
            style_pane_active
        })
}

// --- UNIFIED STYLES ---

fn style_title_active(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(colors::BG_TRANSPARENT.into()),
        ..Default::default()
    }
}

fn style_title_focused(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(colors::BG_TRANSPARENT.into()),
        ..Default::default()
    }
}

fn style_pane_active(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(colors::BG_PANEL.into()),
        border: iced::Border {
            width: metrics::BORDER_WIDTH_PANEL,
            color: colors::BORDER_PANEL,
            radius: metrics::RADIUS_PANEL.into(),
        },
        ..Default::default()
    }
}

fn style_pane_focused(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(colors::BG_PANEL.into()),
        border: iced::Border {
            width: metrics::BORDER_WIDTH_PANEL,
            color: colors::BORDER_FOCUSED,
            radius: metrics::RADIUS_PANEL.into(),
        },
        ..Default::default()
    }
}
