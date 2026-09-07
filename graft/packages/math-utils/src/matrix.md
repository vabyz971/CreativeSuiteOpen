# packages/math-utils/src/matrix.rs

- Matrix4 · struct · L20-L22 — pub struct Matrix4
- IDENTITY · constant · L25-L32 — pub const IDENTITY: Self = Self
- identity · function · L34-L36 — pub const fn identity() -> Self
- translation · function · L38-L44 — pub fn translation(x: f32, y: f32, z: f32) -> Self
- scale · function · L46-L52 — pub fn scale(x: f32, y: f32, z: f32) -> Self
- transform_point · function · L54-L60 — pub fn transform_point(&self, p: [f32; 4]) -> [f32; 4]
- default · function · L64-L66 — fn default() -> Self
