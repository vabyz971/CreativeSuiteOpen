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

//! Activity indicators: circular spinner (rotating dots with
//! trail) and indeterminate progress bar. Animation is driven
//! by app via angle updated by subscription (`time::every`).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use crate::theme::colors;
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::{Element, Length, Point};

/// Circular spinner — 8 dots around a circle, decreasing opacity
/// behind current angle (Material-style trail).
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
            // The point at current angle is most opaque, trail behind
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

/// Animated circular spinner — `angle_deg` driven by app (tick ~30 fps).
#[must_use]
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

/// Indeterminate progress bar — segment bouncing on a track.
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
        // Track
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), colors::SURFACE_CONTAINER_HIGH);
        // Moving segment: sinusoidal position (smooth bounce)
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

/// Animated indeterminate progress bar.
#[must_use]
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
