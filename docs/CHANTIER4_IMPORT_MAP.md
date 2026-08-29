# Chantier 4 — Mapping complet des imports (document.rs)

> Objectif : valider le découpage `document.rs` (2315 lignes) → `model` / `tree` / `compositing` / `tests` sans casser l'API publique.
> Ce document est le filet de sécurité demandé : carte exhaustive des symboles, dépendances et ré-exports.

## 1. API publique stable (ne doit pas changer pour `apps/photo`, `project.rs`, `renderer.rs`)

```rust
// Re-exportés depuis crate::document (via document/mod.rs)
pub use model::{
    AdjustmentLayer, Appearance, BlendMode, FilterNode, GroupLayer, LayerMask,
    LayerNode, PixelLayer, RgbaBuf, Transform2D, next_appearance_version,
};
pub use tree::Document;
pub use compositing::{
    preview_buf, thumb_buf, // seuls helpers publics utilisés hors crate (ui_handles)
    // internes mais exposés pub(crate) pour tests / project
    // needs_fallback_in, blend_into, prepare_top, prepare_mask, fold_scope, scope_half_extents, apply_adjustment
};
```

Vérification : `grep -r "crate::document::\|photo_engine::document::\|Document::\|LayerNode::" apps/ engines/ --include="*.rs" | cut -d: -f2 | sort -u` doit rester vert après split.

## 2. Frontières

| Fichier | Lignes originales | Contenu | Dépendances internes |
|---------|-------------------|---------|----------------------|
| `model.rs` | 1–533 | `RgbaBuf`, `BlendMode`, `Transform2D`, `FilterNode`, `LayerMask`, `PixelLayer`, `GroupLayer`, `AdjustmentLayer`, `LayerNode`, `Appearance`, `APPEARANCE_VERSION`, `next_appearance_version()`, `regenerate_ids()` | `std`, `image`, `serde`, `uuid`, `datatypes`, `rayon` (à retirer, inutilisé) |
| `tree.rs` | 534–1170 | `Document` + `find_in`, `find_in_mut`, `find_owner_list`, `hide_subtree`, `collect_pixels`, `needs_fallback_in` (déplacé depuis compositing pour cohérence arbre), `Document::group/ungroup/duplicate/move_up/down/reorder/crop/flip/set_source_image/add_filter.../apply_command` | `super::model::*`, `super::RgbaBuf`, `crate::renderer::Renderer`, `crate::command::Command`, `crate::history::Snapshot` |
| `compositing.rs` | 1171–1675 | `preview_buf`, `thumb_buf`, `blend_pixel`, `blend_into`, `DrawItem`, `prepare_top`, `prepare_mask`, `scope_half_extents`, `extents_visit`, `fold_scope`, `apply_adjustment` | `super::model::{BlendMode, LayerMask, Transform2D, LayerNode, RgbaBuf}`, `image`, `rayon` |
| `tests.rs` | 1677–2315 | `#[cfg(test)] mod tests` (golden + masques) | `super::model::*`, `super::tree::Document`, `super::compositing::*`, `crate::history::Snapshot` |

## 3. Imports détaillés par sous-module

### model.rs
```rust
use std::cell::RefCell; // non, uniquement tree
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
// aucun import crate interne hors datatypes
```

Spécifique :
- `APPEARANCE_VERSION: AtomicU64` → `pub(crate) static`
- `next_appearance_version()` → `pub fn` (ré-exporté) ou `pub(crate)` + `pub use` dans mod.rs doit être `pub` (pas `pub(crate)` sinon E0364)
- `LayerNode::regenerate_ids()` → `pub(crate)` (utilisé par `tree.rs::duplicate`)
- `Transform2D::transformed_extents()` → `pub(crate)` (utilisé par `compositing.rs::extents_visit`)

### tree.rs
```rust
use std::cell::RefCell;
use std::sync::Arc;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use uuid::Uuid;
use super::model::{
    AdjustmentLayer, Appearance, BlendMode, FilterNode, GroupLayer, LayerMask,
    LayerNode, PixelLayer, RgbaBuf, Transform2D, next_appearance_version,
};
use crate::command::Command;
use crate::history::Snapshot;
use crate::renderer::Renderer;
```

Fonctions à déplacer et rendre `pub(crate)` :
- `find_in`, `find_in_mut`, `find_owner_list`, `hide_subtree`, `collect_pixels` → `pub(crate)` (utilisés par tests et `compositing.rs::fold_scope` via `tree::find_in`)

### compositing.rs
```rust
use std::sync::Arc;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use rayon::prelude::*;
use uuid::Uuid;
use super::model::{BlendMode, LayerMask, LayerNode, RgbaBuf, Transform2D};
use super::tree::Document; // pour type Resolver si besoin
```

Fonctions :
- `needs_fallback_in` → `pub` (ré-exporté pour `Document::needs_fallback`)
- `preview_buf`, `thumb_buf` → `pub`
- `blend_pixel` → `pub(crate)` (interne)
- `blend_into`, `prepare_top`, `prepare_mask`, `fold_scope`, `scope_half_extents`, `apply_adjustment` → `pub` ou `pub(crate)` selon usage externe (`project.rs` n'en a pas besoin, mais `mod.rs` les ré-exportait en `pub`)

### tests.rs
```rust
use super::model::{BlendMode, FilterNode, GroupLayer, LayerMask, LayerNode, PixelLayer, Transform2D};
use super::tree::Document;
use crate::history::Snapshot;
use datatypes::ParamValue;
use image::{DynamicImage, ImageBuffer, Rgba};
use std::sync::Arc;

fn solid(...) -> DynamicImage { ... }
fn arc(...) -> Arc<DynamicImage> { ... }
fn pixel_node(...) -> LayerNode { ... }
fn masked_node(...) -> LayerNode { ... } // masques
fn doc_of(...) -> Document { ... }
fn px(...) -> [u8;4] { ... }
fn assert_close(...) { ... }
```

## 4. `document/mod.rs` (façade)

```rust
//! Document LayerTree — facade re-exporting submodules.
pub mod compositing;
pub mod model;
pub mod tree;
#[cfg(test)] mod tests;

pub use compositing::{preview_buf, thumb_buf};
pub use model::{
    AdjustmentLayer, Appearance, BlendMode, FilterNode, GroupLayer, LayerMask,
    LayerNode, PixelLayer, RgbaBuf, Transform2D, next_appearance_version,
};
pub use tree::Document;

// Compat : helpers internes restent accessibles en pub(crate) sans ré-export pub
pub(crate) use compositing::{blend_into, fold_scope, needs_fallback_in, prepare_mask, prepare_top};
```

**Règle** : ne pas `pub use` des `pub(crate)` hors crate (E0364) — les garder `pub(crate) use` ou ne pas ré-exporter.

## 5. Dépendances externes par crate

- `photo-engine` → `datatypes`, `suite-core`, `image`, `rayon`, `uuid`, `serde`
- `apps/photo` → `photo_engine::document::{Document, LayerNode, BlendMode, ...}` (inchangé)
- `engines/photo-engine/src/project.rs` → `crate::document::{Document, LayerNode, PixelLayer, GroupLayer, LayerMask, Transform2D, BlendMode}` → doit passer par `crate::document::model::*` ou `crate::document::*` (via mod.rs) — vérifier `use crate::document::{...}` reste valide grâce au `pub use`
- `engines/photo-engine/src/renderer.rs` → `crate::document::{Appearance, PixelLayer, Document}`

## 6. Étapes de validation

```bash
cp engines/photo-engine/src/document.rs /tmp/doc.bak
# créer les 4 fichiers + mod.rs selon mapping ci-dessus
cargo check -p photo-engine          # 0 erreur
cargo check --workspace              # 0 erreur (apps/photo n'a pas changé d'import)
cargo test -p photo-engine           # 69 verts (golden + masques)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
# diff API publique
cargo public-api --crate photo-engine 2>/dev/null | diff /tmp/api_avant.txt -
```

## 7. Pièges identifiés lors de la tentative précédente

1. `next_appearance_version` en `pub(crate)` → E0364 au `pub use` — le rendre `pub`
2. `regenerate_ids` privé → E0624 depuis `tree.rs` — `pub(crate)`
3. `transformed_extents` privé → E0624 depuis `compositing.rs` — `pub(crate)`
4. `RgbaBuf`/`Appearance` manquants dans `pub use model` → E0432 — ajouter
5. `use super::RgbaBuf` dans `tree.rs` → introuvable (ordre des `pub use`) — utiliser `super::model::RgbaBuf`
6. Double `use super::RgbaBuf;` + `use super::model::RgbaBuf;` → conflit — garder un seul

## 8. Prochaine PR

Branche `chantier-4-split-document` depuis `photo` (actuel `a50242b`), appliquer ce mapping, `cargo test --workspace` vert, puis PR indépendante (ne pas mélanger avec chantier 5 déjà livré).
