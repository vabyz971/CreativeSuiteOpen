# TODO — Migration wgpu 30 native quand `iced` passera à `wgpu 30`

> Contexte 2026-08-26 : `wgpu = 30.0.0` (`Cargo.toml:22`) vs `iced_wgpu 27.0.1` (`Cargo.lock:1834`) → 2 crates `wgpu` incompatibles. `photo-engine` crée son propre `Device` (`engines/photo-engine/src/gpu.rs:53`) → copie VRAM→RAM→VRAM obligatoire via `Handle::from_rgba` (`apps/photo/src/ui_handles.rs:33`). Pont async en place pour masquer le flicker (voir `gpu.rs:233` `run_compute`).

**Déclencheur :** `iced 0.15` (ou `iced_wgpu` dépendant de `wgpu 30.x`) publié. Vérifier avec `cargo tree | grep wgpu` → une seule version `30.x`.

## Checklist migration

- [ ] `Cargo.toml:22` — passer `wgpu = "30.0.0"` reste, vérifier `iced = "0.15"` et `Cargo.lock` n'a plus qu'un `wgpu 30.x`
- [ ] `engines/photo-engine/src/gpu.rs:41-112` — supprimer `GPU: OnceLock<Option<Arc<GpuContext>>>` + `try_new_async():53` (`Instance::new` + `request_adapter` + `request_device`). Remplacer par :
  ```rust
  static SHARED: OnceLock<Arc<GpuContext>> = OnceLock::new();
  impl GpuContext {
      pub fn init_from_iced(device: wgpu::Device, queue: wgpu::Queue, info: String) { ... }
      pub fn get() -> Option<Arc<Self>> { SHARED.get().cloned() }
  }
  ```
  Appelé depuis `packages/ui-kit/src/layer_canvas.rs:585` `CompositePipeline::new()` (premier `prepare():792`) qui reçoit déjà `device: &wgpu::Device, queue: &wgpu::Queue` d'`iced`.

- [ ] `engines/photo-engine/src/gpu.rs:811` — étendre `SHADER_BC_TEX` à tous les effets (`SHADER_SAT_TEX`, `SHADER_BLUR_TEX`, `SHADER_MIX`) en `texture_2d -> texture_storage_2d` et chaîner `input_tex -> tmp* -> final_tex` sans `image_to_floats:214` / `floats_to_image:224`. Supprimer le chemin `Storage Buffer` `run_compute:233` (floats 16o/px) — ne garder que `run_compute_banded` texture.

- [ ] `apps/photo/src/ui_handles.rs:33` `rgba_handle()` + `PreviewCache:99` — ne plus convertir `RgbaBuf` en `Handle::from_rgba(Bytes::from_owner)`. Stocker `wgpu::Texture` / `TextureView` et pousser dans `CompositePrimitive` (`layer_canvas.rs:553`) directement. Garder `RgbaBuf` uniquement pour `fallback_handle` CPU et export `project.rs:390`.

- [ ] `packages/ui-kit/src/layer_canvas.rs:792` `prepare()` — supprimer `queue.write_texture:829` par calque (upload CPU). Remplacer par `bind_group` direct vers `acc_tex` / `layer_textures` issues du `GpuContext` partagé.

- [ ] `apps/photo/src/components/image_processor.rs:31` + `engines/photo-engine/src/processor.rs:39` — supprimer le pont `Task<Message::GpuEffectDone>` async créé pour `wgpu30→iced27`. Revenir à appel synchrone texture→texture (reste en VRAM).

- [ ] `apps/photo/src/components/workspace.rs:244` `render_canvas_preview` — passer `Vec<wgpu::TextureView>` au lieu de `Vec<CanvasLayer{handle:Handle}>` (`image_canvas.rs:668`).

- [ ] `engines/photo-engine/src/gpu.rs:138` `detect_gpu_info_sync` — nettoyer texte `Backend: wgpu 30.0.0` / `Traitement nodal: GPU compute + fallback CPU` — n'afficher qu'un seul `Device limits`.

- [ ] Tests : `cargo test -p photo-engine` (62 tests) + `cargo clippy --workspace` 0 warning + `npx graft build` (74 fichiers) doivent rester verts. Vérifier drag multi-calques 60fps sans `poll(Wait)`.

## Références

- Pont actuel : `gpu.rs:233` / `gpu.rs:353` / `ui_handles.rs:33` / `layer_canvas.rs:792`
- State-only déjà OK : `workspace.rs:265` + `layer_canvas.rs:462` (`off_sel`/`pan_zoom`)
- Issue iced : https://github.com/iced-rs/iced/issues — attendre tag `wgpu 30`
