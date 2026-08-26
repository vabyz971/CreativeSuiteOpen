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

//! Math utils partagés pour la CreativeSuiteOpen
//!
//! `Vec2` n'est pas redéfini ici : la source canonique reste
//! `datatypes::Vec2`, réexportée pour la commodité des appelants.

pub mod bezier;
pub mod matrix;
pub mod vec3;

pub use datatypes::Vec2;
pub use matrix::Matrix4;
pub use vec3::Vec3;

pub use bezier::{BezierCurve, BezierSegment};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_at_endpoints() {
        let seg = BezierSegment::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(2.0, -1.0),
            Vec2::new(3.0, 0.0),
        );
        let start = seg.evaluate(0.0);
        let end = seg.evaluate(1.0);
        assert_eq!((start.x, start.y), (0.0, 0.0));
        assert_eq!((end.x, end.y), (3.0, 0.0));
    }

    #[test]
    fn identity_leaves_point_unchanged() {
        let m = Matrix4::identity();
        let p = [1.0, 2.0, 3.0, 1.0];
        let out = m.transform_point(p);
        assert_eq!(out, p);
    }

    #[test]
    fn translation_moves_point() {
        let m = Matrix4::translation(1.0, 2.0, 3.0);
        let out = m.transform_point([1.0, 1.0, 1.0, 1.0]);
        assert_eq!(out, [2.0, 3.0, 4.0, 1.0]);
    }

    #[test]
    fn vec3_cross_is_orthogonal() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = x.cross(y);
        assert_eq!(z, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(x.dot(y), 0.0);
    }
}
