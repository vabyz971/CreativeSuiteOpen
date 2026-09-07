# apps/photo/src/update/mod.rs

- layers · module · L26-L26 — mod layers;
- misc · module · L27-L27 — mod misc;
- paint · module · L28-L28 — mod paint;
- panels · module · L29-L29 — mod panels;
- project · module · L30-L30 — mod project;
- update · function · L34-L40 — pub fn update(app: &mut PhotoApp, message: Message) -> Task<Message>
- dispatch · function · L42-L65 — fn dispatch(app: &mut PhotoApp, message: Message) -> Task<Message>
- tests · module · L69-L305 — mod tests
- solid_img · function · L74-L78 — fn solid_img(w: u32, h: u32) -> std::sync::Arc<image::DynamicImage>
- seed_layer · function · L84-L93 — fn seed_layer(app: &mut PhotoApp, w: u32, h: u32) -> uuid::Uuid
- cycle_calque_undo_redo · function · L96-L106 — fn cycle_calque_undo_redo()
- duplication_produit_nouvel_id · function · L109-L119 — fn duplication_produit_nouvel_id()
- suppression_dernier_calque_refusee · function · L122-L133 — fn suppression_dernier_calque_refusee()
- coalescing_opacite_en_un_undo · function · L136-L145 — fn coalescing_opacite_en_un_undo()
- drag_masque_zero_recomposite_par_mouvement · function · L152-L248 — fn drag_masque_zero_recomposite_par_mouvement()
- clic_selection_sans_mouvement_ne_lance_aucune_composite · function · L253-L304 — fn clic_selection_sans_mouvement_ne_lance_aucune_composite()
