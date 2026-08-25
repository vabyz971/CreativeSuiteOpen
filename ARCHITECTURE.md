# Architecture de CreativeSuiteOpen

## Vue d'ensemble

CreativeSuiteOpen est une suite créative professionnelle composée de trois applications
indépendantes (Photo, Vidéo, Audio) qui partagent un socle commun : moteurs métier,
graphe nodal générique, widgets et bibliothèques utilitaires.

## Structure des dossiers

```
apps/       Applications finales (binaires indépendants)
engines/    Moteurs métier PURS — zéro dépendance UI
core/       Socle commun : suite-core (graphe nodal), datatypes, shell
packages/   Bibliothèques réutilisables : ui-kit, math-utils, file-utils
assets/     Ressources partagées (polices)
```

### packages/
Bibliothèques partagées réutilisables entre toutes les applications.
- `ui-kit` (crate `ui_kit`) : widgets iced en couches — `theme` (seule source des
  couleurs/tailles, tokens DESIGN.md), `style` (styles canoniques), primitives
  transverses, layouts, canvas domaine (`image_canvas`, `node_graph`, `timeline`,
  `piano_roll`).
- `math-utils` : mathématiques communes (`Vec3`, `Matrix4`, courbes de Bézier) ;
  le `Vec2` canonique reste `datatypes::Vec2`, réexporté.
- `file-utils` : erreurs fichiers, types drag & drop et dialogues.

Ces packages ne doivent JAMAIS dépendre des engines ni des apps.

### engines/
Moteurs métier spécifiques à chaque domaine, strictement purs :
aucune connaissance d'iced ou de ses types. Les buffers portés par le modèle
document restent purs (`RgbaBuf`, `Arc<[u8]>`) ; toute conversion vers une
texture UI se fait côté app.
- `photo-engine` : document, compositing CPU/GPU, historique, projet `.csphoto`.
- `video-engine`, `audio-engine` : fondations.

Ils peuvent dépendre de `core/*` et de `packages/*` (hors UI).

### core/
Socle transverse : `suite-core` (graphe nodal générique), `datatypes`
(nœuds, sockets, `Vec2`), `suite-shell` (layout, menus, fenêtre).

### apps/
Applications finales qui combinent packages, core et engines. Découpage par rôle :
`message.rs`, `state.rs`, `update.rs`, `view.rs`, `menus.rs`, plus les modules
d'adaptation moteur→UI (`ui_handles.rs`). Chaque app est un binaire indépendant.

## Règles de dépendances

1. `packages/` ne dépend JAMAIS de `engines/` ni de `apps/`
2. `core/` ne dépend que de lui-même ; `engines/` peut dépendre de `core/` et `packages/` (hors UI)
3. `apps/` peuvent dépendre de tout le reste
4. Pas de dépendances circulaires
5. Pas de dépendances entre apps

Vérification : `cargo tree -p <crate> --depth 1`.

## Modèle de rendu

- **State-only** : un réglage (opacité, position…) ne régénère jamais les pixels ;
  il s'applique au draw GPU.
- **Rendu hybride** : chemin rapide = une texture GPU par calque dessinée
  indépendamment ; fallback CPU (rayon) pour les fusions nécessitant un vrai
  blending inter-calques.
- **Frontière moteur→UI** : `PreviewCache` dérive les handles iced des `RgbaBuf`
  par identité d'Arc (zéro copie), synchronisé au début de chaque message dans
  `update()` — point unique de conversion.

## Compilation

```bash
cargo build --workspace --release   # tout le workspace
cargo build -p photo --release      # une seule app
cargo run -p photo                  # lancer une app
cargo test -p photo-engine          # tests du moteur photo (golden compositing)
```
