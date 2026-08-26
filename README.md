# CreativeSuiteOpen

**A professional, open-source creative suite — Linux-first, available everywhere.**

CreativeSuiteOpen is a creative suite (Photo, Video, Audio) built in **Rust** with **Iced** and **wgpu**. The project was born from a simple observation: Linux users have very few professional-grade creative applications. The major suites on the market (Adobe, Affinity) ignore Linux or remain closed. CreativeSuiteOpen aims to fill that gap with a 100% open-source foundation — fast and cross-platform.

- **Linux** first (Wayland/X11, Vulkan) — official development environment via Nix
- **Windows** and **macOS** supported natively thanks to Rust and wgpu (Vulkan / DX12 / Metal)

---

## Project status

| App | Status | Version |
|-----|--------|---------|
| **Photo** | Daily-usable — LayerTree, live filters, hybrid history, GPU rendering, projects, export | `0.5.0` |
| **Video** | Foundations (UI shell) | `0.1.0` |
| **Audio** | Foundations (UI shell) | `0.1.0` |

Versions follow each crate's functional maturity: `0.1.0` = foundations, `0.2.0` = complete technical base, `0.3.0` = first real feature set, `0.5.0` = professional editing model (layer tree, non-destructive filters).

---

## Features — Photo (`0.5.0`)

### Affinity-style layer tree
- **Hierarchical layer tree**: pixel layers, **groups** (collapsible, with their own opacity/blend) and **adjustment layers** that process everything beneath them
- Ordered stack: **add, duplicate, delete, reorder (within parent), rename**, group/dissolve
- Live thumbnails, per-layer visibility toggle
- **Opacity and blend applied at draw time (GPU)** — sliders respond instantly: zero pixel regeneration, zero flicker
- **Blend modes**: Normal, Multiply, Screen, Overlay, Darken, Lighten
- **Per-layer real-time dragging** (60 fps, zero recomposite during the gesture)

### Live filters (non-destructive)
- Per-layer filter chains: brightness/contrast, blur, color correction… evaluated through an internal node-graph engine
- Filters never alter the source image — edit parameters anytime, disable without losing settings
- **Per-layer appearance cache keyed by a signature of the filter chain + source identity**: editing layer N recomputes layer N only; neighbors keep their textures untouched

### Hybrid history & native project (`0.5.0`)
- **Hybrid undo/redo** (Ctrl+Z / Ctrl+Y, 50 steps): full snapshots for destructive/structural operations (paint, crop, reordering), lightweight commands for micro-editions (opacity, transforms, blend, renames, filter parameters) — near-zero memory cost, precise render invalidation
- Continuous gestures (sliders, renaming, drags) coalesced into a single restoration point within an 800 ms window; redo after a coalesced gesture restores the gesture's final value
- **Native project format `.csophoto` (v2)**: hierarchical tree saved as versioned JSON, source pixels stored as PNG so filters stay alive across sessions — Save (`Ctrl+S`), Save As (`Ctrl+Shift+S`); open projects or plain images from the same dialog (legacy `.csphoto` files are still detected)
- **PNG/JPEG export**: exports the full composite (`Ctrl+Shift+E`) — transparency preserved in PNG, alpha flattened onto white in JPEG (quality 90)

### Infinite canvas
- No cropping: images may extend past the document, like professional artboards
- Document outline drawn **in world space** (zoom-independent, never distorted)
- Smooth pan/zoom: mouse wheel, Hand tool, zoom to selection, fit to screen

### Hybrid CPU/GPU rendering
- Fast path: each layer = one independently drawn GPU texture (move/opacity with no recomputation)
- CPU rayon fallback for blending that requires true inter-layer compositing (groups in non-Normal modes, adjustment layers)
- Compute-shader filters (brightness/contrast, blur…) with graceful CPU fallback when no adapter is present
- GPU detection (Vulkan/DX12/Metal) and hardware info in preferences

### Nodal texture generator
- Built-in node editor (dedicated panel) — intended for texture generation and filters applicable to layers (work in progress)

### Tools
Hand, Zoom, Rectangle selection, Move, **Brush**, **Eraser** (destination-out, ring preview), Eyedropper — floating toolbar hideable with `Tab` (shortcuts `B` / `E`)

### Interface
- Resizable panel layout (Layers, Properties, Generator)
- Full menus (File, Edit, Layer, View) with shortcuts (`Ctrl+O`, `Ctrl+J`, `F7`…)
- Unified design system: DESIGN.md tokens + shared canonical styles (`ui_kit::style`) — macOS-style tool palette
- Consistent dark theme, Hanken Grotesk typeface, Material icons

---

## Project structure

```
CreativeSuiteOpen/
├── apps/                     # User-facing applications
│   ├── photo/                # Photo editor (layers, tools, canvas)
│   ├── video/                # Video editor (foundations)
│   └── audio/                # Audio station (foundations)
├── engines/                  # PURE business engines (zero UI dependencies)
│   ├── photo-engine/         # Photo engine: layer tree, compositing, live filters,
│   │                         #   hybrid history, GPU compute, project format, export
│   ├── video-engine/         # Video engine (upcoming)
│   └── audio-engine/         # Audio engine (upcoming)
├── core/                     # Shared foundation reused across apps
│   ├── core/                 # suite-core: generic node graph (evaluation, connections)
│   ├── datatypes/            # Shared types: nodes, sockets, parameters, Vec2
│   └── shell/                # Common shell: layout, menu bar, window
├── packages/                 # Reusable libraries (never depend on engines/apps)
│   ├── ui-kit/               # Iced widgets: theme.rs (SOLE source of tokens), style.rs,
│   │                         #   node_graph.rs, image_canvas.rs, layer_canvas.rs,
│   │                         #   menu.rs / dropdown.rs, timeline.rs / piano_roll.rs
│   ├── math-utils/           # Shared math: Vec3, Matrix4, Bézier (canonical Vec2 = datatypes)
│   └── file-utils/           # I/O: drag & drop, file dialogs
├── assets/fonts/             # Hanken Grotesk, Material Icons
├── flake.nix                 # NixOS dev environment (Vulkan, Wayland)
└── Cargo.toml                # Rust workspace
```

### Architecture philosophy
- **Strict modularity**: business logic lives in `engines/*` and `core/*`, never in the apps. An app = interface + orchestration.
- **Pure engines**: `photo-engine` (layer tree, compositing, history) has no UI dependency and can later serve the video module (titles, image compositing).
- **Layered dependencies**: `packages` ← `core`/`engines` ← `apps`. Packages never depend on engines or apps.
- **State-only rendering**: settings (opacity, transform, blend) never regenerate pixels — they apply at draw time. This invariant is what makes the UI feel instant.
- **Rust + Iced + wgpu**: one codebase, native GPU rendering on all three platforms.

---

## Building

### Prerequisites
- Rust 1.85+ (2024 edition)
- System dependencies (Linux): `pkg-config`, `vulkan-loader`, `libxkbcommon`, `wayland` (+ `libx11` for X11)

### Linux (NixOS / Nix)
```bash
nix develop        # ready-to-use dev shell
cargo build --release -p photo
```

### Linux (classic distros)
```bash
cargo build --release -p photo
./target/release/photo
```

### Windows / macOS
```bash
cargo build --release -p photo
```
No special configuration: wgpu automatically selects DX12 (Windows) or Metal (macOS).

### Running the other apps
```bash
cargo run -p video
cargo run -p audio
```

---

## Contributing

Contributions are **very welcome** — this is a young project and every contribution counts, from bug fixes to the rendering engine.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide: development setup, architecture rules, code style and the pull-request checklist.

Quick summary:
1. **Open an issue first** for significant changes (architecture, new engines).
2. **Respect modularity**: business logic → `engines/*` / `core/*`, widgets → `packages/ui-kit`, orchestration → `apps/*`.
3. **Preserve the state-only rendering model** — profile before optimizing.
4. **Quality gate**: `cargo clippy --workspace` clean, `cargo fmt` before every commit.
5. Short, descriptive commit messages prefixed by the app (`photo: fix blend-mode offset jump`).

---

## Roadmap

- [x] Photo: real-time layer system (`0.3.0`)
- [x] Photo: brush + eraser, undo/redo, `.csophoto` project (`0.4.0`)
- [x] Photo: LayerTree (groups, adjustment layers), live filters, hybrid history, PNG/JPEG export (`0.5.0`)
- [ ] Photo: layer masks, vector shapes
- [ ] Photo: zero-readback GPU pipeline integrated with the UI renderer
- [ ] Nodal generator: generated textures applied to layers
- [ ] Video: timeline, editing, preview (`0.2.0` → `0.3.0`)
- [ ] Audio: mixer, tracks, piano roll
- [ ] Linux packaging (Flatpak/AppImage) + Windows/macOS releases

---

## License

CreativeSuiteOpen is free software licensed under the **GNU GPL v3** — see the [LICENSE](LICENSE) file.

- You are free to use, study, modify and redistribute this software
- Any derivative version must remain open source under the same license (copyleft)
- Every source file starts with the standard GPL header

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

*Built with Rust, Iced and wgpu — for creators on Linux.*
