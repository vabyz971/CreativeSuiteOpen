# packages/math-utils/src/bezier.rs

- BezierSegment · struct · L22-L27 — pub struct BezierSegment
- new · function · L30-L32 — pub fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self
- evaluate · function · L34-L51 — pub fn evaluate(&self, t: f32) -> Vec2
- BezierCurve · struct · L55-L57 — pub struct BezierCurve
- new · function · L60-L62 — pub fn new() -> Self
- add_segment · function · L64-L66 — pub fn add_segment(&mut self, segment: BezierSegment)
- evaluate · function · L68-L77 — pub fn evaluate(&self, t: f32) -> Option<Vec2>
- tests · module · L81-L108 — mod tests
- linear · function · L84-L91 — fn linear(a: Vec2, b: Vec2) -> BezierSegment
- curve_samples_each_segment · function · L94-L107 — fn curve_samples_each_segment()
