# apps/photo/src/components/options_bar.rs

- ICON_ROTATE_LEFT · constant · L33-L33 — const ICON_ROTATE_LEFT: &str = "\u{e419}";
- ICON_ROTATE_RIGHT · constant · L34-L34 — const ICON_ROTATE_RIGHT: &str = "\u{e41a}";
- ICON_FLIP · constant · L35-L35 — const ICON_FLIP: &str = "\u{e3e8}"; // flip
- ICON_CROP · constant · L36-L36 — const ICON_CROP: &str = "\u{e3be}";
- ICON_RESET · constant · L37-L37 — const ICON_RESET: &str = "\u{e166}"; // restart_alt
- render · function · L42-L85 — pub fn render<'a>(
- tool_controls · function · L91-L116 — pub fn tool_controls<'a>(
- brush_section · function · L122-L158 — fn brush_section<'a>(
- eraser_section · function · L164-L193 — fn eraser_section<'a>(brush_size: f32, brush_opacity: f32) -> Element<'a, Message>
- move_section · function · L199-L318 — fn move_section<'a>(
- field_label · function · L324-L330 — fn field_label(s: &'static str) -> Element<'static, Message>
- value_label · function · L332-L334 — fn value_label(s: String) -> Element<'static, Message>
- separator · function · L336-L347 — fn separator<'a>() -> Element<'a, Message>
