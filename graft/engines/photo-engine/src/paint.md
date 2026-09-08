# engines/photo-engine/src/paint.rs

- StrokeMode · enum · L29-L34 — pub enum StrokeMode
- BrushParams · struct · L38-L47 — pub struct BrushParams
- paint_stroke_rgba · function · L52-L164 — pub fn paint_stroke_rgba(rgba: &mut [u8], w: u32, h: u32, points: &[(f32, f32)], b: &BrushParams)
- StrokeCommit · struct · L169-L178 — pub struct StrokeCommit
- fmt · function · L181-L187 — fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
- commit_stroke · function · L196-L204 — pub fn commit_stroke(
- commit_stroke_locked · function · L207-L279 — fn commit_stroke_locked(
- tests · module · L282-L398 — mod tests
- trait_opaque_sur_fond_transparent · function · L286-L308 — fn trait_opaque_sur_fond_transparent()
- opacite_50_sur_fond_blanc · function · L311-L330 — fn opacite_50_sur_fond_blanc()
- gomme_opaque_efface_le_centre_preserve_les_bords · function · L333-L354 — fn gomme_opaque_efface_le_centre_preserve_les_bords()
- gomme_50_reduit_alpha_de_moitie · function · L357-L376 — fn gomme_50_reduit_alpha_de_moitie()
- gomme_sur_pixel_deja_transparent_sans_effet_bord · function · L379-L397 — fn gomme_sur_pixel_deja_transparent_sans_effet_bord()
