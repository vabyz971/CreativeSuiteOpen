# RENDERING.md — Contrat du pipeline de rendu (figé)

**Ce document est un CONTRAT, pas une suggestion.** Toute modification touchant `image_canvas.rs`, `layer_canvas.rs`, `gpu.rs`, `document/compositing.rs`, ou `state.rs::*fallback*`/`*drag*` doit être vérifiée contre les règles ci-dessous. Si une tâche semble exiger de violer une règle, **s'arrêter et demander confirmation** plutôt que de la contourner silencieusement.

Objectif de ce document : arrêter le pipeline de rendu à SA forme actuelle (elle est correcte) pour qu'aucun agent futur ne le réinvente à chaque session — cf. le cas vécu où `image_canvas.rs` a réimplémenté un calcul de rotation déjà présent dans `math-utils`.

---

## Les deux chemins — jamais un troisième

Il n'existe que DEUX façons pour un calque d'arriver à l'écran. Toute nouvelle fonctionnalité de rendu doit être un cas de l'un des deux, jamais un chemin parallèle.

### Chemin A — rapide (GPU, par calque, temps réel)
- Utilisé quand `Document::needs_fallback() == false`.
- Chaque calque est dessiné comme SA PROPRE texture GPU, positionnée/tournée/mise à l'échelle par le shader (`layer_canvas.rs`) au moment du draw.
- **Aucun recalcul CPU par frame.** Les textures sont préparées une fois (au chargement/à la modification de la source) et lues depuis `PreviewCache`.
- Limite stricte : ce chemin ne sait dessiner QUE des calques individuels sans interaction complexe entre eux (pas de blend mode non-Normal, pas de masque actif, pas de groupe à opacité <100%).

### Chemin B — fallback (CPU, composite complet, async)
- Déclenché quand `Document::needs_fallback() == true` (défini dans `document/compositing.rs::needs_fallback_in` — SEUL endroit qui décide de cette bascule).
- Composite l'intégralité du document (ou une variante `_without(id)` pendant un drag) via `Document::composite_preview()`.
- **Toujours** via `tokio::task::spawn_blocking`, **jamais** appelé de façon synchrone dans `update()` ou `view()`.
- Le résultat est un `image::Handle` unique affiché à la place du dessin par-calque, jusqu'à ce que le prochain résultat (tag de génération) arrive.

### Règle absolue
Si une fonctionnalité future (ex. : effet de calque, nouveau mode de fusion, ombre portée) ne peut PAS être exprimée par le chemin A tel quel, elle DOIT ajouter une condition à `needs_fallback_in()` et se reposer sur le chemin B — jamais écrire un troisième chemin de rendu ad hoc. `needs_fallback_in()` est la liste exhaustive et unique des raisons de basculer en fallback ; toute nouvelle raison s'y ajoute, nulle part ailleurs.

---

## Invariants non négociables

1. **Aucune fonction de `document/compositing.rs` (`composite`, `composite_preview`, `blend_into`, `apply_layer_mask`, `prepare_top*`) n'est appelée en dehors de `spawn_blocking` ou des tests.** Si un grep de `\.composite(` ou `\.composite_preview(` remonte un appel direct dans `update()`/`view()`, c'est une violation du contrat.
2. **`view()` ne fait jamais de calcul d'image.** Il ne fait que lire des `Handle` déjà en cache (`PreviewCache`) et les positionner. Toute donnée qu'il affiche doit avoir été calculée AVANT, dans `update()` ou une tâche async.
3. **Toute géométrie (rotation, échelle, skew, transformation de point) passe par `packages/math-utils`.** Aucun fichier hors de `math-utils` ne réimplémente `cos`/`sin`/multiplication de matrice pour transformer un point — importer et utiliser les types existants. Si `math-utils` ne couvre pas un cas nécessaire, l'étendre LÀ, jamais dupliquer ailleurs.
4. **Le pool `rayon` utilisé par le compositing CPU est le pool dédié et plafonné** (voir `DIAGNOSTIC_FREEZE_MASQUE.md` §3) — jamais le pool `rayon` global par défaut, pour ne pas saturer les threads de rendu wgpu/iced pendant une composite fallback.
5. **Toute tâche async de plus de quelques millisecondes pousse un libellé dans `app.background_tasks`** (voir `DIAGNOSTIC_FREEZE_MASQUE.md` §1) — l'indicateur de la barre du haut doit refléter TOUT traitement en arrière-plan, sans exception, dès son introduction.

---

## Checklist avant de toucher au rendu

Avant d'écrire une ligne de code touchant l'affichage :
- [ ] Ai-je vérifié si `packages/math-utils` couvre déjà le calcul géométrique dont j'ai besoin ?
- [ ] Ma fonctionnalité peut-elle être rendue par le chemin A tel quel (pas d'interaction complexe entre calques) ?
- [ ] Si non, ai-je ajouté ma condition à `needs_fallback_in()` plutôt que d'écrire un chemin de rendu séparé ?
- [ ] Mon nouveau code de composite est-il appelé UNIQUEMENT via `spawn_blocking` (ou dans les tests) ?
- [ ] `view()` reste-t-il une fonction de lecture pure, sans calcul ?
- [ ] Ai-je câblé `background_tasks` si l'opération est asynchrone et non instantanée ?

Si une seule case est cochée « non » sans justification écrite dans le commit, la modification ne respecte pas ce contrat.
