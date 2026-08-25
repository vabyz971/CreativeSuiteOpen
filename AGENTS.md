# AGENTS.md

Suite créative Rust : workspace Cargo, apps **Iced 0.14 + wgpu**, licence GPL-3.0. Docs : `README.md` (architecture, roadmap), `DESIGN.md` (tokens thème).

## Environnement
- Édition Rust 2024 → toolchain **1.85+** obligatoire.
- Linux : entrer dans `nix develop` (ou direnv via `.envrc`) — fournit cargo/clippy/rustfmt + Vulkan/Wayland avec `LD_LIBRARY_PATH` déjà réglé.
- Sans Nix : installer `pkg-config vulkan-loader libxkbcommon wayland`.

## Commandes
- App principale : `cargo run --release -p photo`. Autres apps (fondations) : `-p video`, `-p audio`.
- Vérification avant commit : `cargo fmt` puis `cargo clippy --workspace` (doit passer sans erreur). Pas de CI dans le repo — c'est le seul garde-fou.
- Tests unitaires inline (`#[cfg(test)]`, présents surtout dans `core/*`) : `cargo test -p photo-engine` (ou autre crate).

## Packages du workspace
Le nom de crate diffère parfois du dossier — utiliser `-p` avec le nom de crate :
| Dossier | Crate |
|---|---|
| `apps/photo` / `apps/video` / `apps/audio` | `photo` / `video` / `audio` (entrypoint `src/main.rs`) |
| `core/core` | `suite-core` (graphe nodal générique) |
| `core/shell` | `suite-shell` (layout, menus, fenêtre) |
| `core/datatypes` | `datatypes` (nœuds, sockets, Vec2 partagés) |
| `core/photo-engine` | `photo-engine` (document, compositing CPU/GPU) |
| `ui/` | `ui` (widgets iced réutilisables) |

## Règles d'architecture (strictes)
- Logique métier → `core/*` ; widgets → `ui/` ; apps = interface + orchestration uniquement. Pas de logique de rendu dans les apps.
- Les moteurs (`photo-engine`, etc.) sont indépendants de l'UI et doivent le rester.
- Modèle de rendu **« state-only »** : un réglage (opacité, position…) ne régénère jamais les pixels/textures ; il s'applique au draw GPU. Préserver ce modèle à tout prix.
- Rendu hybride : chemin rapide = 1 texture GPU par calque dessinée indépendamment ; fallback CPU rayon uniquement pour les modes de fusion nécessitant un vrai blending inter-calques.

## Conventions
- `unwrap()` interdit hors tests ; pas d'emoji dans le code ni les commits.
- Commits courts et préfixés par l'app : `photo: fix blend-mode offset jump`.
- Chaque fichier `.rs` commence par l'en-tête GPL v3 (copier celui de `apps/photo/src/main.rs`).
- Thème : palette/tokens définis dans `DESIGN.md`, implémentés dans `ui/src/theme.rs` ; police Hanken Grotesk et icônes Material chargées depuis `assets/fonts/`.
