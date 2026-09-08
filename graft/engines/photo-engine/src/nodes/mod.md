# engines/photo-engine/src/nodes/mod.rs

- blur · module · L25-L25 — pub mod blur;
- brightness_contrast · module · L26-L26 — pub mod brightness_contrast;
- color_correct · module · L27-L27 — pub mod color_correct;
- input · module · L28-L28 — pub mod input;
- layer · module · L29-L29 — pub mod layer;
- mix · module · L30-L30 — pub mod mix;
- output · module · L31-L31 — pub mod output;
- NodeCtx · struct · L42-L49 — pub struct NodeCtx<'a>
- input · function · L53-L55 — pub fn input(&self) -> Option<&DynamicImage>
- param · function · L58-L63 — pub fn param(&self, key: &str, default: f32) -> f32
- Effect · struct · L67-L70 — pub struct Effect
- all · function · L73-L83 — pub fn all() -> Vec<Effect>
- find · function · L86-L88 — pub fn find(type_id: &str) -> Option<Effect>
- to_rgba8 · function · L95-L97 — pub fn to_rgba8(img: &DynamicImage) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>>
