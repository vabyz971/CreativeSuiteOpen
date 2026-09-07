# packages/math-utils/src/vec3.rs

- Vec3 · struct · L22-L26 — pub struct Vec3
- ZERO · constant · L29-L29 — pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
- new · function · L31-L33 — pub const fn new(x: f32, y: f32, z: f32) -> Self
- length · function · L35-L37 — pub fn length(self) -> f32
- normalized · function · L39-L46 — pub fn normalized(self) -> Self
- dot · function · L48-L50 — pub fn dot(self, other: Self) -> f32
- cross · function · L52-L58 — pub fn cross(self, other: Self) -> Self
- Output · type · L62-L62 — type Output = Self;
- add · function · L64-L66 — fn add(self, rhs: Self) -> Self
- Output · type · L70-L70 — type Output = Self;
- sub · function · L72-L74 — fn sub(self, rhs: Self) -> Self
- Output · type · L78-L78 — type Output = Self;
- mul · function · L80-L82 — fn mul(self, scalar: f32) -> Self
