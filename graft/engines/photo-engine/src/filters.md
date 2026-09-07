# engines/photo-engine/src/filters.rs

- new_filter_layer · function · L36-L43 — pub fn new_filter_layer(type_id: &str) -> Option<FilterLayer>
- filterable_types · function · L47-L62 — pub fn filterable_types() -> Vec<datatypes::NodeDefinition>
- render_nodes · function · L67-L90 — pub(crate) fn render_nodes(
- render_chain · function · L99-L119 — pub fn render_chain(source: &Arc<DynamicImage>, layers: &[FilterLayer]) -> Arc<DynamicImage>
- tests · module · L122-L251 — mod tests
- grey · function · L128-L134 — fn grey(value: u8) -> Arc<DynamicImage>
- layer · function · L136-L138 — fn layer(type_id: &str) -> FilterLayer
- lum · function · L140-L145 — fn lum(value: f32) -> FilterLayer
- chaine_vide_renvoie_la_source_partagee · function · L148-L153 — fn chaine_vide_renvoie_la_source_partagee()
- filtre_desactive_est_transparent · function · L156-L164 — fn filtre_desactive_est_transparent()
- brightness_applique_le_parametre · function · L167-L182 — fn brightness_applique_le_parametre()
- chaine_sequentielle_compose_les_effets · function · L185-L197 — fn chaine_sequentielle_compose_les_effets()
- effet_inconnu_propage_son_entree · function · L200-L207 — fn effet_inconnu_propage_son_entree()
- opacite_sous_calque_mixe_lineairement · function · L210-L220 — fn opacite_sous_calque_mixe_lineairement()
- fusion_sous_calque_multiply · function · L223-L232 — fn fusion_sous_calque_multiply()
- passthrough_neutre_sans_cout · function · L235-L241 — fn passthrough_neutre_sans_cout()
- filterable_types_exclut_entrees_sorties · function · L244-L250 — fn filterable_types_exclut_entrees_sorties()
