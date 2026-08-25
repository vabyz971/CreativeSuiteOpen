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

//! Courbes de Bézier cubiques

use crate::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierSegment {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl BezierSegment {
    pub fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    pub fn evaluate(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        Vec2::new(
            mt3 * self.p0.x
                + 3.0 * mt2 * t * self.p1.x
                + 3.0 * mt * t2 * self.p2.x
                + t3 * self.p3.x,
            mt3 * self.p0.y
                + 3.0 * mt2 * t * self.p1.y
                + 3.0 * mt * t2 * self.p2.y
                + t3 * self.p3.y,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BezierCurve {
    pub segments: Vec<BezierSegment>,
}

impl BezierCurve {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_segment(&mut self, segment: BezierSegment) {
        self.segments.push(segment);
    }

    pub fn evaluate(&self, t: f32) -> Option<Vec2> {
        let count = self.segments.len();
        if count == 0 {
            return None;
        }
        let scaled = t.clamp(0.0, 1.0) * count as f32;
        let index = (scaled as usize).min(count - 1);
        let local_t = scaled - index as f32;
        Some(self.segments[index].evaluate(local_t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(a: Vec2, b: Vec2) -> BezierSegment {
        BezierSegment::new(
            a,
            Vec2::new(a.x + (b.x - a.x) / 3.0, a.y + (b.y - a.y) / 3.0),
            Vec2::new(a.x + 2.0 * (b.x - a.x) / 3.0, a.y + 2.0 * (b.y - a.y) / 3.0),
            b,
        )
    }

    #[test]
    fn curve_samples_each_segment() {
        let mut curve = BezierCurve::new();
        curve.add_segment(linear(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)));
        curve.add_segment(linear(Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)));
        assert_eq!(curve.segments.len(), 2);

        let mid = curve.evaluate(0.25).expect("curve has segments");
        assert!((mid.x - 0.5).abs() < 1e-5);
        assert!(mid.y.abs() < 1e-6);

        assert!(curve.evaluate(0.0).is_some());
        let empty = BezierCurve::new();
        assert!(empty.evaluate(0.5).is_none());
    }
}
