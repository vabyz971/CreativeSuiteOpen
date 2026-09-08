# packages/ui-kit/src/menu.rs

- SLOT_WIDTH · constant · L26-L26 — pub const SLOT_WIDTH: f32 = 64.0;
- SLOT_GAP · constant · L28-L28 — pub const SLOT_GAP: f32 = 2.0;
- BAR_HEIGHT · constant · L30-L30 — pub const BAR_HEIGHT: f32 = 28.0;
- Item · enum · L33-L46 — pub enum Item<Message>
- action · function · L50-L57 — pub fn action(label: impl Into<String>, message: Message) -> Self
- Menu · struct · L61-L64 — pub struct Menu<Message>
- new · function · L67-L72 — pub fn new(label: impl Into<String>, items: Vec<Item<Message>>) -> Self
- bar · function · L79-L229 — pub fn bar<'a, Message: Clone + 'a>(menus: &[Menu<Message>]) -> Element<'a, Message>
