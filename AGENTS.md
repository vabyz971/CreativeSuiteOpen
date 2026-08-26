# AGENTS.md

Suite créative Rust : workspace Cargo, apps **Iced 0.14 + wgpu**, licence GPL-3.0. Docs : `README.md` (architecture, roadmap), `ARCHITECTURE.md` (règles de dépendances), `DESIGN.md` (tokens thème).

## Environnement
- Édition Rust 2024 → toolchain **1.85+** obligatoire.
- Linux : entrer dans `nix develop` (ou direnv via `.envrc`) — fournit cargo/clippy/rustfmt + Vulkan/Wayland avec `LD_LIBRARY_PATH` déjà réglé.
- Sans Nix : installer `pkg-config vulkan-loader libxkbcommon wayland`.

## Commandes
- App principale : `cargo run --release -p photo`. Autres apps (fondations) : `-p video`, `-p audio`.
- Vérification avant commit : `cargo fmt --all` puis `cargo clippy --workspace`. La CI GitHub (`.github/workflows/ci.yml`) rejoue fmt+clippy+tests sur chaque PR.
- Tests : `cargo test -p photo-engine` (compositing, historique, projet). Les tests « golden » de `document.rs` vérifient les modes de fusion pixel par pixel — ne pas les affaiblir pour faire passer un refactor.

## Packages du workspace
Le nom de crate diffère parfois du dossier — utiliser `-p` avec le nom de crate :
| Dossier | Crate |
|---|---|
| `apps/photo` / `apps/video` / `apps/audio` | `photo` / `video` / `audio` (entrypoint `src/main.rs`) |
| `core/core` | `suite-core` (graphe nodal générique) |
| `core/shell` | `suite-shell` (layout, menus, fenêtre) |
| `core/datatypes` | `datatypes` (nœuds, sockets, Vec2 partagés) |
| `engines/photo-engine` | `photo-engine` (document, compositing CPU/GPU, historique, projet) |
| `engines/audio-engine` / `engines/video-engine` | `audio-engine` / `video-engine` (fondations, purs) |
| `packages/ui-kit` | `ui-kit` (lib `ui_kit`, widgets iced réutilisables) |
| `packages/math-utils` | `math-utils` (Vec3, Matrix4, Bézier ; Vec2 canonique = datatypes) |
| `packages/file-utils` | `file-utils` (erreurs fichiers, drag & drop, dialogues) |

Dépendances autorisées : `packages/*` ← `core/*` ← `engines/*` ← `apps/*`. Les packages ne dépendent jamais des engines ni des apps ; pas de dépendances entre apps.

## Règles d'architecture (strictes)
- Logique métier → `engines/*` et `core/*` ; widgets → `packages/ui-kit` ; apps = interface + orchestration uniquement. Pas de logique de rendu dans les apps.
- **Les moteurs sont PURS : aucune dépendance UI.** `photo-engine` ne connaît ni iced ni ses types ; le modèle document (`Layer`) porte des buffers purs (`RgbaBuf`, `Arc<[u8]>`). Toute conversion vers une texture UI se fait côté app via l'adaptateur `apps/photo/src/ui_handles.rs`.
- Frontière moteur→UI : `PreviewCache` dérive les handles iced des `RgbaBuf` par identité d'Arc (zéro copie via `Bytes::from_owner`). Synchronisé à CHAQUE message au début de `update()` — point unique, ne pas créer de handles ailleurs sous peine de casser le cache de textures GPU de iced.
- Modèle de rendu **« state-only »** : un réglage (opacité, position…) ne régénère jamais les pixels/textures ; il s'applique au draw GPU. Préserver ce modèle à tout prix.
- Rendu hybride : chemin rapide = 1 texture GPU par calque dessinée indépendamment ; fallback CPU rayon uniquement pour les modes de fusion nécessitant un vrai blending inter-calques.

## Historique & persistance
- Undo/redo : snapshots complets du document (`engines/photo-engine/src/history.rs`) quasi gratuits grâce aux `Arc<DynamicImage>` partagés. Les gestes continus (sliders, renommage, drag) passent par `push_coalesced` — toujours pousser le snapshot PRÉ-mutation, jamais après.
- Format projet `.csophoto` (`engines/photo-engine/src/project.rs`) : JSON versionné, calques en PNG+base64. Toute évolution incompatible du modèle → incrémenter `FORMAT_VERSION` et gérer le refus proprement.

## Structure de l'app photo
Découpée par rôle (même schéma pour les futures apps) :
`message.rs` (enum Message + types partagés) · `state.rs` (PhotoApp + helpers) · `update.rs` (un handler par message) · `view.rs` (rendu + abonnements) · `menus.rs` · `ui_handles.rs`. Ne pas regrossir vers un main.rs monolithique.

## Architecture de `packages/ui-kit` (en couches, voir lib.rs)
1. **`theme`** = SEULE source des couleurs/tailles/rayons/ombres (tokens DESIGN.md : `colors`, `type_scale`, `metrics`, `spacing`, `shadows`).
2. **`style`** = styles canoniques par famille visuelle (`ghost`, `ghost_selected`, `menu_item`, `primary`, `chip`, `action_chip*`, `floating_card`, `inset_card`). Un composant n'écrit JAMAIS sa closure de style : il référence `ui_kit::style::*`.
3. **Primitives transverses** (`icon_button`, `spinner`, `dropdown`, `settings`, `shortcuts`) → 4. **Layouts** (`shell`, `menu`, `base_panel`) → 5. **Canvas domaine** (`image_canvas`, `layer_canvas`, `node_graph`, `timeline`, `piano_roll`).
- Les éléments spécifiques à une app restent dans `apps/<app>/src/components/`. Promotion vers `packages/ui-kit` seulement quand une 2e app en a besoin.
- **Interdit de coder une couleur en dur hors `theme.rs`** — y compris dans les canvas (la sélection utilise `SELECTION_*`, les nœuds `NODE_*`). Tailles de texte : passer par `type_scale`.

## Conventions
- `unwrap()`/`expect()` interdits hors tests ; pas d'emoji dans le code ni les commits.
- Commits courts et préfixés par l'app : `photo: fix blend-mode offset jump`.
- Chaque fichier `.rs` commence par l'en-tête GPL v3 (copier celui de `apps/photo/src/main.rs`).
- Thème : palette/tokens définis dans `DESIGN.md`, implémentés dans `packages/ui-kit/src/theme.rs` ; police Hanken Grotesk et icônes Material chargées depuis `assets/fonts/`.
