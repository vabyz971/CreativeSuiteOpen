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

//! Indicateurs d'activité : spinner circulaire (points en rotation avec
//! traînée) et barre de progression indéterminée. L'animation est pilotée
//! par l'app via un angle mis à jour par abonnement (`time::every`).

use crate::theme::colors;
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::{Element, Length, Point};

/// Spinner circulaire — 8 points autour d'un cercle, opacité décroissante
/// derrière l'angle courant (traînée façon Material).
struct Spinner {
    angle_deg: f32,
    size: f32,
    dot_count: usize,
}

impl<Message> canvas::Program<Message> for Spinner {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let r = (self.size / 2.0 - 3.0).max(2.0);
        let n = self.dot_count as f32;
        for i in 0..self.dot_count {
            let a = (self.angle_deg + i as f32 * 360.0 / n).to_radians();
            let p = Point::new(c.x + r * a.cos(), c.y + r * a.sin());
            // Le point à l'angle courant est le plus opaque, traînée derrière
            let alpha = (i as f32 + 1.0) / n;
            let color = iced::Color {
                a: alpha.clamp(0.15, 1.0),
                ..colors::ACCENT
            };
            frame.fill(&Path::circle(p, 2.4), color);
        }
        vec![frame.into_geometry()]
    }
}

/// Spinner circulaire animé — `angle_deg` piloté par l'app (tick ~30 fps).
pub fn circle<Message: 'static>(angle_deg: f32, size: f32) -> Element<'static, Message> {
    iced::widget::canvas(Spinner {
        angle_deg,
        size,
        dot_count: 8,
    })
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .into()
}

/// Barre de progression indéterminée — segment en va-et-vient sur une piste.
struct IndeterminateBar {
    angle_deg: f32,
}

impl<Message> canvas::Program<Message> for IndeterminateBar {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        // Piste
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), colors::SURFACE_CONTAINER_HIGH);
        // Segment mobile : position sinusoïdale (va-et-vient fluide)
        let t = (self.angle_deg.to_radians().sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let seg_w = (bounds.width * 0.35).max(24.0);
        let max_x = (bounds.width - seg_w).max(0.0);
        let x = t * max_x;
        frame.fill_rectangle(
            Point::new(x, 0.0),
            iced::Size::new(seg_w, bounds.height),
            colors::ACCENT,
        );
        vec![frame.into_geometry()]
    }
}

/// Barre de progression indéterminée animée.
pub fn progress_bar<Message: 'static>(
    angle_deg: f32,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    iced::widget::canvas(IndeterminateBar { angle_deg })
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}
