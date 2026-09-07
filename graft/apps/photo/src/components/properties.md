# apps/photo/src/components/properties.rs

- render · function · L34-L214 — pub fn render<'a>(
- filter_editor · function · L218-L344 — fn filter_editor<'a>(
- mask_panel · function · L348-L431 — fn mask_panel<'a>(
- _action_btn · function · L433-L440 — fn _action_btn(label: &'static str, msg: Message, color: iced::Color) -> Element<'static, Message>
- source_info · function · L442-L474 — fn source_info(dims: (u32, u32), img: &image::DynamicImage) -> Element<'static, Message>
- add_filter_pick · function · L477-L505 — fn add_filter_pick(layer_id: uuid::Uuid) -> Element<'static, Message>
- Choice · struct · L480-L483 — struct Choice
- fmt · function · L485-L487 — fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
- filter_card · function · L509-L584 — fn filter_card<'a>(
- float_param_range · function · L587-L596 — fn float_param_range(key: &str) -> (f32, f32)
- blend_mode_buttons · function · L598-L639 — fn blend_mode_buttons(cur: Option<BlendMode>, id: uuid::Uuid) -> Element<'static, Message>
- offset_row · function · L641-L663 — fn offset_row<'a>(
- param_slider · function · L665-L697 — fn param_slider<'a>(
