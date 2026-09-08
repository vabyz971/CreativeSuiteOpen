# apps/photo/src/ui_handles.rs

- rgba_handle · function · L33-L39 — pub fn rgba_handle(buf: &photo_engine::RgbaBuf) -> iced::widget::image::Handle
- PreviewCache · struct · L42-L45 — pub struct PreviewCache
- Entry · struct · L47-L57 — struct Entry
- MaskThumb · struct · L60-L64 — struct MaskThumb
- arc_addr · function · L66-L68 — fn arc_addr(data: &Arc<[u8]>) -> usize
- img_arc_addr · function · L70-L72 — fn img_arc_addr(data: &Arc<image::RgbaImage>) -> usize
- mask_thumb_handle · function · L75-L83 — fn mask_thumb_handle(img: &image::RgbaImage) -> iced::widget::image::Handle
- sync · function · L88-L150 — pub fn sync(&mut self, doc: &Document)
- preview · function · L153-L155 — pub fn preview(&self, id: Uuid) -> Option<&iced::widget::image::Handle>
- thumb · function · L158-L160 — pub fn thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle>
- mask_thumb · function · L163-L165 — pub fn mask_thumb(&self, id: Uuid) -> Option<&iced::widget::image::Handle>
