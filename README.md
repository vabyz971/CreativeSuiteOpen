# CreativeSuiteOpen

**Une suite créative professionnelle, open source, pensée pour Linux d'abord — disponible partout.**

CreativeSuiteOpen est une suite créative (Photo, Vidéo, Audio) construite en **Rust** avec **Iced** et **wgpu**. Le projet est né d'un constat simple : les utilisateurs de Linux disposent de très peu de logiciels créatifs de niveau professionnel. Les grandes suites du marché (Adobe, Affinity) ignorent Linux ou restent fermées. CreativeSuiteOpen vise à combler ce vide avec une base 100 % open source, performante et multiplateforme.

- **Linux** en priorité (Wayland/X11, Vulkan) — environnement de développement officiel via Nix
- **Windows** et **macOS** supportés nativement grâce à Rust et wgpu (Vulkan / DX12 / Metal)

---

## État du projet

| App | État | Version |
|-----|------|---------|
| **Photo** | Utilisable au quotidien — calques temps réel, GPU, undo/redo, projets | `0.4.0` |
| **Vidéo** | Fondations (interface) | `0.1.0` |
| **Audio** | Fondations (interface) | `0.1.0` |

Les versions suivent la maturité fonctionnelle de chaque crate : `0.1.0` = fondations, `0.2.0` = socle technique complet, `0.3.0` = premier jeu de fonctionnalités réelles.

---

## Fonctionnalités — Photo (`0.4.0`)

### Système de calques façon Photoshop / Affinity
- Pile de calques ordonnée : **ajouter, dupliquer, supprimer, réordonner, renommer**
- Miniatures live, œil de visibilité par calque
- **Opacité appliquée au draw (GPU)** — le slider répond instantanément, zéro régénération de pixels, zéro clignotement
- **Modes de fusion** : Normal, Multiply, Screen, Overlay, Darken, Lighten
- Déplacement **par calque en temps réel** (60 fps, zéro recomposite pendant le drag)

### Historique & projet natif (`0.4.0`)
- **Undo/redo** complet (Ctrl+Z / Ctrl+Y, 50 pas) — snapshots quasi gratuits grâce au partage de pixels, gestes continus (sliders, renommage, drag) coalescés en un seul point de restauration
- **Format projet `.csphoto`** : enregistrement/ouverture du document (calques, fusion, transforms) — Enregistrer (`Ctrl+S`), Enregistrer sous (`Ctrl+Maj+S`), ouverture projet ou image depuis la même boîte

### Plan de travail infini
- Aucun crop : les images peuvent dépasser le document, comme sur les plans de travail pro
- Repère document dessiné **dans l'espace monde** (insensible au zoom, jamais déformé)
- Pan/zoom fluide : molette, outil Main, zoom sur sélection, ajuster à l'écran

### Rendu hybride CPU/GPU
- Chemin rapide : chaque calque = une texture GPU dessinée indépendamment (déplacement/opacité sans aucun recalcul)
- Fallback CPU rayon pour les modes de fusion nécessitant un vrai blending inter-calques
- Détection GPU (Vulkan/DX12/Metal) et informations matériel dans les préférences

### Générateur de textures (graphe nodal)
- Éditeur nodal intégré (panneau dédié) — destiné à la génération de textures et aux filtres appliquables aux calques (en développement)

### Outils
Main, Zoom, Sélection rectangulaire, Déplacement, **Pinceau**, **Gomme** (destination-out, aperçu en anneau), Pipette — barre d'outils flottante masquable (`Tab`, raccourcis `B` / `E`)

### Interface
- Layout à panneaux redimensionnables (Calques, Propriétés, Générateur)
- Menus complets (Fichier, Édition, Calque, Affichage) avec raccourcis (`Ctrl+O`, `Ctrl+J`, `F7`…)
- Design system unifié : tokens DESIGN.md + styles canoniques partagés (`ui_kit::style`) — palette d'outils façon macOS
- Thème sombre cohérent, police Hanken Grotesk, icônes Material

### Interface
- Layout à panneaux redimensionnables (Calques, Propriétés, Générateur)
- Menus complets (Fichier, Édition, Calque, Affichage) avec raccourcis (`Ctrl+O`, `Ctrl+J`, `F7`…)
- Thème sombre cohérent, police Hanken Grotesk, icônes Material

---

## Structure du projet

```
CreativeSuiteOpen/
├── apps/                     # Applications utilisateur
│   ├── photo/                # Éditeur photo (calques, outils, canvas)
│   ├── video/                # Éditeur vidéo (fondations)
│   └── audio/                # Station audio (fondations)
├── engines/                  # Moteurs métier PURS (zéro dépendance UI)
│   ├── photo-engine/         # Moteur photo : modèle document, compositing CPU/GPU, modes de fusion
│   ├── video-engine/         # Moteur vidéo (à venir)
│   └── audio-engine/         # Moteur audio (à venir)
├── core/                     # Socle commun réutilisable entre apps
│   ├── core/                 # suite-core : graphe de nœuds générique (évaluation, connexions)
│   ├── datatypes/            # Types partagés : nœuds, sockets, paramètres, Vec2
│   └── shell/                # Shell commun : layout, barre de menus, fenêtre
├── packages/                 # Bibliothèques réutilisables (ne dépendent jamais des engines/apps)
│   ├── ui-kit/               # Widgets iced : theme.rs (SEULE source des tokens), style.rs,
│   │                         #   node_graph.rs, image_canvas.rs, layer_canvas.rs,
│   │                         #   menu.rs / dropdown.rs, timeline.rs / piano_roll.rs
│   ├── math-utils/           # Math partagées : Vec3, Matrix4, Bézier (Vec2 canonique = datatypes)
│   └── file-utils/           # I/O : drag & drop, dialogues de fichiers
├── assets/fonts/             # Hanken Grotesk, Material Icons
├── flake.nix                 # Environnement de dev NixOS (Vulkan, Wayland)
└── Cargo.toml                # Workspace Rust
```

### Philosophie d'architecture
- **Modularité stricte** : la logique métier vit dans `engines/*` et `core/*`, jamais dans les apps. Une app = interface + orchestration.
- **Moteurs purs** : `photo-engine` (modèle document, compositing) est indépendant de l'UI et pourra servir au module vidéo (titres, compositing d'images).
- **Dépendances en couches** : `packages` ← `core`/`engines` ← `apps`. Les packages ne dépendent jamais des engines ni des apps.
- **Rust + Iced + wgpu** : un seul code source, rendu GPU natif sur les trois plateformes.

---

## Compilation

### Prérequis
- Rust 1.85+ (édition 2024)
- Dépendances système (Linux) : `pkg-config`, `vulkan-loader`, `libxkbcommon`, `wayland` (+ `libx11` si X11)

### Linux (NixOS / Nix)
```bash
nix develop        # shell de dev prêt à l'emploi
cargo build --release -p photo
```

### Linux (distros classiques)
```bash
cargo build --release -p photo
./target/release/photo
```

### Windows / macOS
```bash
cargo build --release -p photo
```
Aucune configuration spécifique : wgpu sélectionne automatiquement DX12 (Windows) ou Metal (macOS).

### Lancer les autres apps
```bash
cargo run -p video
cargo run -p audio
```

---

## Contribuer

Les contributions sont **très bienvenues** — c'est un projet jeune, chaque apport compte, du fix de bug au moteur de rendu.

### Bonnes pratiques
1. **Ouvre une issue avant** les changements importants (architecture, nouveaux moteurs) pour en discuter.
2. **Respecte la modularité** : logique métier → `engines/*` / `core/*`, widgets → `packages/ui-kit`, orchestration → `apps/*`. Pas de logique de rendu dans les apps.
3. **Performance** : le modèle de rendu est « state-only » (les réglages ne régénèrent jamais les textures). Préserve-le — profile avant d'optimiser (`cargo flamegraph`), mesure avant/après.
4. **Qualité** : `cargo clippy --workspace` sans erreur, `cargo fmt` avant tout commit. Les `unwrap()` sont interdits hors tests.
5. **Pas d'emoji** dans le code ni les commits ; messages de commit courts et descriptifs (`photo: fix blend-mode offset jump`).

### Idées de contribution
- Photo : masques de calque, formes/sélection, export (PNG/JPEG)
- Générateur nodal : brancher l'évaluation du graphe sur des textures de calque
- Video / Audio : faire passer les fondations au stade `0.2.0` (timeline fonctionnelle)
- Packaging : Flatpak, AppImage, AUR, brew
- Traductions, tests, documentation

---

## Feuille de route

- [x] Photo : système de calques temps réel (`0.3.0`)
- [x] Photo : pinceau + gomme, historique undo/redo, projet `.csphoto` (`0.4.0`)
- [ ] Photo : masques de calque, formes, export (PNG/JPEG)
- [ ] Générateur nodal : textures générées appliquées aux calques
- [ ] Vidéo : timeline, montage, preview (`0.2.0` → `0.3.0`)
- [ ] Audio : mixer, pistes, piano roll
- [ ] Packaging Linux (Flatpak/AppImage) + releases Windows/macOS

---

## Licence

CreativeSuiteOpen est un logiciel libre distribué sous licence **GNU GPL v3** — voir le fichier [LICENSE](LICENSE).

- Vous êtes libre d'utiliser, d'étudier, de modifier et de redistribuer ce logiciel
- Toute version dérivée doit rester open source sous la même licence (copyleft)
- Chaque fichier source commence par l'en-tête GPL standard

```
CreativeSuiteOpen — Suite créative professionnelle open source
Copyright (C) 2026 vabyz971

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.
```

---

*Fait avec Rust, Iced et wgpu — pour les créateurs sur Linux.*
