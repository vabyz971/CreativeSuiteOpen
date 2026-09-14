---
covers: []
---
# Architecture de Cygnus

## Vue d'ensemble

Cygnus est une suite créative professionnelle composée de trois applications
indépendantes (Photo, Vidéo, Audio) qui partagent un socle commun : moteurs métier,
graphe nodal générique, widgets et bibliothèques utilitaires.

## Structure des dossiers

```
apps/       Applications finales (binaires indépendants)
engines/    Moteurs métier PURS — zéro dépendance UI
core/       Socle commun : datatypes
packages/   Bibliothèques réutilisables : ui-kit, math-utils, file-utils
assets/     Ressources partagées (polices)
```

### packages/
Bibliothèques partagées réutilisables entre toutes les applications.
- `ui-kit` (crate `ui_kit`) : design system egui en couches — `theme`
  (seule source des couleurs/tailles, tokens dans `theme/`), `widgets`
  génériques (dont `CygnusIcon`, `ReorderableList`), `panels`,
  `viewport` pan/zoom générique, `dialogs`. Strictement
  domain-agnostic : aucun type métier (vérifié par
  `scripts/check_uikit_domain_agnostic.sh`).
- `math-utils` : transformation affine 2D canonique (`Transform2D`) ;
  le `Vec2` canonique reste `datatypes::Vec2`, réexporté.
- `file-utils` : erreurs fichiers, types drag & drop et dialogues.

Ces packages ne doivent JAMAIS dépendre des engines ni des apps.

### engines/
Moteurs métier spécifiques à chaque domaine, strictement purs :
aucune connaissance d'egui ou de ses types. Les buffers portés par le modèle
document restent purs (`RgbaBuf`, `Arc<[u8]>`) ; les apps envoient des
commandes via `mpsc` à un worker propriétaire du `Document` et reçoivent
snapshots + aperçu composite (conversion texture côté app).
- `photo-engine` : document, compositing CPU/GPU, historique, projet `.cygp`.
- `video-engine`, `audio-engine` : fondations.

Ils peuvent dépendre de `core/*` et de `packages/*` (hors UI).

### core/
Socle transverse : `datatypes` (nœuds, sockets, `Vec2`).

### apps/
Applications finales qui combinent packages, core et engines. Découpage par rôle
(`main.rs` boot eframe, `app.rs` état + channels, `layout.rs` disposition
propre, `ui/` widgets métier). Chaque app est un binaire indépendant ;
photo est complète, video/audio sont des bases en attendant leurs moteurs.

## Règles de dépendances

1. `packages/` ne dépend JAMAIS de `engines/` ni de `apps/`
2. `engines/` peut dépendre de `core/` et des `packages/` non-UI ; `datatypes` ne dépend que de `serde`
3. `apps/` peuvent dépendre de `core/`, `engines/` et `packages/`
4. Pas de dépendances circulaires
5. Pas de dépendances entre apps

Vérification : `cargo tree -p <crate> --depth 1`.

## Modèle de rendu

- **State-only** : un réglage (opacité, position…) ne régénère jamais les pixels ;
  il s'applique au draw GPU.
- **Rendu** : aperçu composite CPU calculé côté worker (thread background,
  taille plafonnée) et téléversé en texture egui côté app ; chemin natif
  wgpu zéro-copie (`register_native_texture`) prévu.
- **Frontière moteur→UI** : le worker répond `LayersChanged { layers,
  preview, can_undo, can_redo }`, pollé en non bloquant (`try_recv`) à
  chaque frame — point unique de conversion.

## Compilation

```bash
cargo build --workspace --release   # tout le workspace
cargo build -p photo --release      # une seule app
cargo run -p photo                  # lancer une app
cargo test -p photo-engine          # tests du moteur photo (golden compositing)
```
