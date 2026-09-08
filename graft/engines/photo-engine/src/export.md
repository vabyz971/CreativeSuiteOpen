# engines/photo-engine/src/export.rs

- DEFAULT_JPEG_QUALITY · constant · L28-L28 — pub const DEFAULT_JPEG_QUALITY: u8 = 90;
- ExportFormat · enum · L32-L37 — pub enum ExportFormat
- from_path · function · L44-L55 — pub fn from_path(path: &Path) -> Self
- flatten_on_white · function · L62-L72 — pub fn flatten_on_white(img: &DynamicImage) -> DynamicImage
- export_image · function · L78-L96 — pub fn export_image(img: &DynamicImage, path: &Path, format: ExportFormat) -> Result<(), String>
- tests · module · L99-L202 — mod tests
- sample_transparent · function · L102-L111 — fn sample_transparent() -> DynamicImage
- temp_path · function · L113-L122 — fn temp_path(tag: &str, ext: &str) -> std::path::PathBuf
- format_deduit_de_lextension_insensible_casse · function · L125-L155 — fn format_deduit_de_lextension_insensible_casse()
- png_conserve_la_transparence · function · L158-L168 — fn png_conserve_la_transparence()
- jpeg_aplatit_sur_blanc_et_perd_l_alpha · function · L171-L192 — fn jpeg_aplatit_sur_blanc_et_perd_l_alpha()
- image_deja_opaque_n_est_pas_recopiee_par_flatten · function · L195-L201 — fn image_deja_opaque_n_est_pas_recopiee_par_flatten()
