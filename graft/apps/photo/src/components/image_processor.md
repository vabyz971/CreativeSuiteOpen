# apps/photo/src/components/image_processor.rs

- param_float · function · L35-L37 — fn param_float(node: &suite_core::Node, key: &str, default: f32) -> f32
- find_input_image · function · L39-L50 — fn find_input_image<'a>(
- eval_known · function · L55-L89 — fn eval_known(
- resolve_mix_fallback · function · L91-L104 — fn resolve_mix_fallback(
- evaluate · function · L112-L148 — pub fn evaluate(graph: &Graph, original: &DynamicImage) -> Option<DynamicImage>
- evaluate_incremental · function · L151-L194 — pub fn evaluate_incremental(
- evaluate_with_cache · function · L197-L243 — pub fn evaluate_with_cache(
- TILE · constant · L249-L249 — const TILE: u32 = 512;
- apply_brightness_contrast · function · L251-L271 — fn apply_brightness_contrast(img: &DynamicImage, brightness: f32, contrast: f32) -> DynamicImage
- apply_blur · function · L273-L283 — fn apply_blur(img: &DynamicImage, radius: f32) -> DynamicImage
- apply_mix · function · L285-L309 — fn apply_mix(a: &DynamicImage, b: &DynamicImage, factor: f32) -> DynamicImage
- apply_saturation · function · L311-L330 — fn apply_saturation(img: &DynamicImage, sat: f32) -> DynamicImage
- to_handle · function · L333-L337 — pub fn to_handle(img: &DynamicImage) -> iced::widget::image::Handle
