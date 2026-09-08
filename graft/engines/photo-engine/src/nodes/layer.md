# engines/photo-engine/src/nodes/layer.rs

- definition · function · L28-L39 — pub fn definition() -> NodeDefinition
- mix_socket · function · L42-L44 — pub fn mix_socket(i: usize) -> String
- MIX_MAX_INPUTS · constant · L47-L47 — pub const MIX_MAX_INPUTS: usize = 6;
- mode_id · function · L49-L58 — fn mode_id(mode: &str) -> u32
- blend_pixel · function · L62-L104 — fn blend_pixel(b: [f32; 4], t: [f32; 4], mode: u32) -> [f32; 4]
- apply_effect · function · L106-L177 — pub fn apply_effect(
- blend_mode_of · function · L179-L184 — fn blend_mode_of<'a>(ctx: &'a NodeCtx<'_>) -> &'a str
- apply · function · L186-L207 — fn apply(ctx: &NodeCtx) -> Option<DynamicImage>
- effect · function · L209-L214 — pub fn effect() -> Effect
