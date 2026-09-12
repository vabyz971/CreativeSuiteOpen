---
covers: []
---
# CreativeSuiteOpen

**A professional, open-source creative suite — Linux-first, available everywhere.**

CreativeSuiteOpen is a creative suite (Photo, Video, Audio) built in **Rust** with **Iced** and **wgpu**. The project was born from a simple observation: Linux users have very few professional-grade creative applications. The major suites on the market (Adobe, Affinity) ignore Linux or remain closed. CreativeSuiteOpen aims to fill that gap with a 100% open-source foundation — fast and cross-platform.

- **Linux** first (Wayland/X11, Vulkan) — official development environment via Nix
- **Windows** and **macOS** supported natively thanks to Rust and wgpu (Vulkan / DX12 / Metal)

---

## Project status

| App | Status | Version |
|-----|--------|---------|
| **Photo** | Daily-usable — LayerTree, live filters, layer masks, hybrid history, GPU rendering, projects, export | `0.7.0` |
| **Video** | Foundations (UI shell) | `0.1.0` |
| **Audio** | Foundations (UI shell) | `0.1.0` |

Versions follow each crate's functional maturity: `0.1.0` = foundations, `0.2.0` = complete technical base, `0.3.0` = first real feature set, `0.5.0` = professional editing model (layer tree, non-destructive filters), `0.7.0` = layer masks and refined tooling.

---

## Features — Photo (`0.7.0`)

### Affinity-style layer tree
- **Hierarchical layer tree**: pixel layers, **groups** (collapsible, with their own opacity/blend) and **adjustment layers** that process everything beneath them
- **Domain-organized messages**: Message enum split into submodules (canvas, document, layers, jobs, project, system, tools, preferences) for cleaner architecture — all variants still accessible via `message::*` prefix (AGENT PR3)
- Ordered stack: **add, duplicate, delete, reorder (within parent), rename**, group/dissolve
- Live thumbnails, per-layer visibility toggle
- **Opacity and blend applied at draw time (GPU)** — sliders respond instantly: zero pixel regeneration, zero flicker
- **Blend modes**: Normal, Multiply, Screen, Overlay, Darken, Lighten
- **Per-layer real-time dragging** (60 fps, zero recomposite during the gesture)
- **Affine transforms** (Affinity-style): move via the box interior, scale (corner handles, uniform with `Ctrl`), rotate, non-uniform scale, skew — each gesture re-applies at draw time with zero pixel regeneration

### Layer masks (`0.7.0`)
- **Multiple raster masks per layer** (pixel layers, groups and even filter sub-layers), each with its own id so they can be selected, edited, reordered and deleted independently
- Paint masks directly with the **brush/eraser in black-or-white mode** (`X` toggles), or work on a mask via the layer panel
- **Masks baked into the layer's appearance signature**: a mask in *Normal* blend no longer forces the CPU fallback — the fast per-layer GPU path is preserved; masks on a scale non-unitaire layer paint at the correct size thanks to the document-space radius → layer-space ellipse conversion
- Rename, toggle, invert and reorder masks (dedicated context menu and panel FX stack) — settings done in a masked layer stay non-destructive
- **FX stack merged**: masks and filter sub-layers share a single collapsible list under each layer, with a unified context menu

### Live filters (non-destructive)
- Per-layer filter chains: brightness/contrast, blur, color correction… evaluated sequentially (each effect receives the previous one's output)
- Filters never alter the source image — edit parameters anytime, disable without losing settings
- **Per-layer appearance cache keyed by a signature of the filter chain + source identity**: editing layer N recomputes layer N only; neighbors keep their textures untouched

### Hybrid history & native project (`0.7.0`)
- **Hybrid undo/redo** (Ctrl+Z / Ctrl+Y, 50 steps): full snapshots for destructive/structural operations (paint, crop, reordering), lightweight commands for micro-editions (opacity, transforms, blend, renames, filter parameters) — near-zero memory cost, precise render invalidation
- Continuous gestures (sliders, renaming, drags) coalesced into a single restoration point within an 800 ms window; redo after a coalesced gesture restores the gesture's final value
- **Native project format `.csophoto` (v4)**: hierarchical tree saved as versioned JSON, source pixels stored as PNG so filters stay alive across sessions — Save (`Ctrl+S`), Save As (`Ctrl+Shift+S`); open projects or plain images from the same dialog (legacy `.csphoto` files are still detected, v3 projects migrate to filter sub-layers)
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

### Procedural effects
- Every effect (blur, color adjustments…) is also usable as a non-destructive live filter on any layer (work in progress: generated textures applied to layers)

### Tools
Hand, Zoom, Rectangle selection, Move, **Brush**, **Eraser** (destination-out, ring preview), **Eyedropper** — floating toolbar hideable with `Tab` (shortcuts `B` / `E`)
- **Brush/eraser opacity** adjustable from 0 to 100% in 1% steps, live size preview in document pixels
- Brush size is interpreted in **document space**: painting on a scaled or out-of-bounds layer matches the on-screen cursor exactly (stroke radius converted to a layer-space ellipse)
- **Pipette with loupe**: hovering with the eyedropper shows a ×4 magnifier patch under the cursor (Photoshop-style), sampled asynchronously from the full composite; click picks the color and returns to the previous tool
- Transform box now includes a dedicated **Scale handle** (square, 0.2× outside the bottom-right corner) for uniform resizing without holding `Ctrl`

### Interface
- Resizable panel layout (Layers, Properties, Generator)
- Full menus (File, Edit, Layer, View) with shortcuts (`Ctrl+O`, `Ctrl+J`, `F7`…)
- Unified design system: tokens in `packages/ui-kit/src/theme.rs` + shared canonical styles (`ui_kit::style`) — macOS-style tool palette
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
│   ├── datatypes/            # Shared types: nodes, sockets, parameters, Vec2
├── packages/                 # Reusable libraries (never depend on engines/apps)
│   ├── ui-kit/               # Iced widgets: theme.rs (SOLE source of tokens), style.rs,
│   │                         #   image_canvas.rs, layer_canvas.rs,
│   │                         #   menu.rs / dropdown.rs, timeline.rs / piano_roll.rs
│   ├── math-utils/           # Shared math: canonical affine `Transform2D`
│   │                         #   (canonical `Vec2` = datatypes)
│   ├── file-utils/           # I/O: drag & drop, file dialogs
│   └── preferences/          # Persistent preferences, hardware, keybindings
├── assets/fonts/             # Hanken Grotesk, Material Icons
├── flake.nix                 # NixOS dev environment (Vulkan, Wayland)
└── Cargo.toml                # Rust workspace
```

### Architecture philosophy
- **Strict modularity**: business logic lives in `engines/*` and `core/*`, never in the apps. An app = interface + orchestration.
- **Pure engines**: `photo-engine` (layer tree, compositing, history) has no UI dependency and can later serve the video module (titles, image compositing).
- **Layered dependencies**: `apps/*` may depend on `engines/*`, `core/*`, and `packages/*`; `engines/*` may depend on `core/*` and non-UI `packages/*`. Packages never depend on engines or apps.
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
- [x] Photo: layer masks, affine transforms, pipette loupe, refined tooling (`0.7.0`)
- [ ] Photo: vector shapes & text layers
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
