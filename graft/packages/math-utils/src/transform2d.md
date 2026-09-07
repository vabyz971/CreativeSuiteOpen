# packages/math-utils/src/transform2d.rs

- Transform2D · struct · L36-L49 — pub struct Transform2D
- default · function · L53-L63 — fn default() -> Self
- deserialize · function · L67-L135 — fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
- Field · enum · L73-L83 — enum Field
- Transform2DVisitor · struct · L85-L85 — struct Transform2DVisitor;
- Value · type · L88-L88 — type Value = Transform2D;
- expecting · function · L90-L92 — fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
- visit_map · function · L94-L131 — fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
- local_to_doc · function · L145-L162 — pub fn local_to_doc(&self, w0: f32, h0: f32, x: f32, y: f32) -> (f32, f32)
- doc_corners · function · L166-L168 — pub fn doc_corners(&self, w0: f32, h0: f32) -> [(f32, f32); 4]
- shear_scale_matrix · function · L174-L183 — pub fn shear_scale_matrix(&self) -> (f32, f32, f32, f32)
- has_skew · function · L188-L190 — pub fn has_skew(&self) -> bool
