# engines/photo-engine/src/command.rs

- Command · enum · L42-L87 — pub enum Command
- target · function · L92-L103 — pub fn target(&self) -> Uuid
- inverse · function · L108-L171 — pub fn inverse(&self) -> Command
- merge_forward · function · L177-L239 — pub fn merge_forward(&mut self, next: &Command) -> bool
- affects_composite · function · L249-L257 — pub fn affects_composite(&self) -> bool
- RenderEvent · enum · L265-L270 — pub enum RenderEvent
- render_event · function · L275-L282 — pub fn render_event(&self) -> RenderEvent
- tests · module · L286-L386 — mod tests
- t · function · L289-L294 — fn t(x: f32) -> Transform2D
- inverse_echange_old_et_new · function · L297-L313 — fn inverse_echange_old_et_new()
- merge_garde_le_vieil_et_prend_le_nouveau_dernier · function · L316-L341 — fn merge_garde_le_vieil_et_prend_le_nouveau_dernier()
- merge_refuse_des_cibles_differentes · function · L344-L360 — fn merge_refuse_des_cibles_differentes()
- evenement_rendu_par_type · function · L363-L385 — fn evenement_rendu_par_type()
