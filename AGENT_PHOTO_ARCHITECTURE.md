# Instructions Agent IA — CreativeSuiteOpen / Photo

## 0. Mission

Tu travailles sur `CreativeSuiteOpen`, branche `photo`.

Ton objectif n'est PAS de réécrire le projet depuis zéro.
Ton objectif est de faire évoluer l'architecture existante pour qu'elle reste :

- simple à comprendre ;
- simple à modifier ;
- testable ;
- performante ;
- cohérente avec Rust et l'écosystème utilisé ;
- adaptée à un éditeur photo professionnel inspiré d'Affinity ;
- résistante à l'accumulation de code généré et d'abstractions inutiles.

Le projet utilise actuellement une architecture où :

- `apps/photo` contient l'application et l'interface `iced` ;
- `engines/photo-engine` contient le moteur photo et le modèle de document ;
- `core/datatypes` contient les types partagés ;
- `packages/ui-kit` contient les composants/styles UI communs.

Le modèle du document est un **LayerTree** hiérarchique. Il ne faut PAS le transformer en node graph utilisateur.

Les mécanismes nodaux restent pertinents comme implémentation interne de certains traitements, notamment les filtres non destructifs, mais l'utilisateur manipule principalement des **layers**.

---

# 1. Règles absolues

## 1.1 Ne pas réécrire ce qui fonctionne

Avant de modifier un module existant :

1. comprendre son rôle ;
2. identifier ses consommateurs ;
3. vérifier ses tests ;
4. vérifier si une API publique dépend de lui ;
5. rechercher les duplications existantes ;
6. rechercher les fonctionnalités équivalentes déjà fournies par une dépendance.

Une réécriture complète est interdite sauf si elle est explicitement demandée ou si l'architecture actuelle rend objectivement impossible l'évolution du projet.

Toute refactorisation doit privilégier les petits changements vérifiables.

---

## 1.2 Ne jamais ajouter une abstraction sans preuve

Avant de créer :

- un nouveau trait ;
- un nouveau wrapper ;
- une nouvelle structure de contexte ;
- un nouveau manager ;
- un nouveau cache ;
- un nouveau système d'événements ;
- une nouvelle collection ;
- une nouvelle couche d'abstraction ;
- une nouvelle utilité générique ;

répondre mentalement à ces quatre questions :

1. Quel problème concret cela résout-il ?
2. Pourquoi les abstractions existantes ne suffisent-elles pas ?
3. Une bibliothèque déjà utilisée fournit-elle cette fonctionnalité ?
4. Cette abstraction réduit-elle réellement la complexité globale ?

Si la réponse à la question 4 est non, ne pas créer l'abstraction.

---

## 1.3 Ne jamais dupliquer une abstraction existante

Avant d'implémenter une fonction ou un système, rechercher dans :

- `iced` ;
- `wgpu` ;
- `image` ;
- `rayon` ;
- `tokio` ;
- `glam` ou autre crate mathématique utilisée ;
- `uuid` ;
- `serde` / `serde_json` ;
- les crates déjà présentes dans le workspace.

Si une fonction standard ou de bibliothèque répond au besoin, utiliser cette solution plutôt que créer une version maison.

Exceptions :

- logique métier spécifique à l'éditeur photo ;
- représentation propre au document ;
- optimisation mesurée ;
- contrainte GPU/CPU spécifique ;
- comportement non fourni par les bibliothèques.

Toute exception doit être justifiable dans le code ou dans le compte rendu du changement.

---

# 2. Architecture cible

L'architecture cible doit rester proche de ceci :

```text
CreativeSuiteOpen
│
├── apps/
│   └── photo/
│       ├── UI iced
│       ├── application state
│       ├── messages
│       ├── outils utilisateur
│       └── orchestration
│
├── engines/
│   └── photo-engine/
│       ├── document/
│       ├── compositing/
│       ├── effects/
│       ├── renderer/
│       ├── history/
│       └── project/
│
├── core/
│   └── datatypes/
│
└── packages/
    └── ui-kit/
```

Principe fondamental :

```text
iced UI
   ↓
Application state / commands
   ↓
Photo Engine
   ↓
Document / rendering / effects
```

Le moteur ne doit pas dépendre de `iced` pour son modèle métier.

---

# 3. Modèle du document : priorité maximale

Le modèle utilisateur est un **LayerTree**.

Structure conceptuelle cible :

```text
Document
└── LayerTree
    ├── PixelLayer
    │   ├── source
    │   ├── transform
    │   ├── masks
    │   └── filters/effects
    │
    ├── GroupLayer
    │   └── children
    │
    └── AdjustmentLayer
        └── effects/parameters
```

## 3.1 Ne pas convertir LayerTree en graph utilisateur

Les layers doivent rester :

- hiérarchiques ;
- ordonnés ;
- sélectionnables ;
- déplaçables ;
- regroupables ;
- masquables ;
- compatibles avec l'ordre d'empilement.

Les mécanismes de type node peuvent exister en interne pour les filtres et traitements.

---

# 4. LayerNode / données communes

Éviter que `LayerNode` devienne une énorme façade avec des dizaines de méthodes forwarding.

Privilégier la factorisation des propriétés communes dans une structure du style :

```rust
struct LayerCommon {
    id: LayerId,
    name: String,
    visible: bool,
    opacity: f32,
    blend_mode: BlendMode,
    masks: Vec<LayerMask>,
}
```

Puis :

```rust
struct PixelLayer {
    common: LayerCommon,
    source: SourceId,
    transform: Transform2D,
    filters: Vec<Filter>,
}

struct GroupLayer {
    common: LayerCommon,
    children: Vec<LayerId>,
}
```

Ne pas appliquer mécaniquement cette structure si le modèle actuel propose une meilleure solution. Le but est de réduire la duplication, pas de déplacer la duplication.

---

# 5. FilterNode / FilterLayer / AdjustmentLayer

C'est une zone à auditer avec une priorité élevée.

Le projet possède actuellement plusieurs représentations conceptuellement proches :

```text
FilterNode
FilterLayer
AdjustmentLayer
PixelLayer filter chain
```

Avant d'ajouter un nouveau système, déterminer si ces types peuvent converger vers une représentation unique du style :

```rust
pub struct Filter {
    pub id: Uuid,
    pub type_id: FilterTypeId,
    pub params: Parameters,
    pub enabled: bool,
}
```

Une implémentation unique de l'instance de filtre est préférable à plusieurs structures représentant la même chose.

Important :

```text
LayerTree = structure du document visible par l'utilisateur
Filter/Effet = traitement interne
```

Ne pas recréer deux systèmes pour le même concept simplement pour des raisons historiques.

---

# 6. Registry des effets

Le registre doit avoir une source de vérité unique.

Éviter les chaînes du style :

```text
find()
  ↓
all_definitions()
  ↓
nodes::all()
  ↓
reconstruction de collections
```

Éviter aussi les allocations répétées pour des définitions statiques.

Préférer une définition statique ou une structure centralisée adaptée au nombre réel d'effets.

Ne pas créer un framework de plugins complexe tant qu'il n'existe pas de besoin réel.

---

# 7. Document et Renderer

Le `Document` ne doit pas devenir le propriétaire de toute l'infrastructure de rendu.

Éviter autant que possible :

```text
Document
  └── RefCell<Renderer>
```

Préférer la séparation :

```text
Document
    = état métier

RenderContext / Renderer
    = GPU
    = cache
    = workers
    = ressources temporaires
```

Le modèle de document doit rester utilisable indépendamment du pipeline de rendu.

Objectifs :

- plusieurs documents possibles ;
- plusieurs vues possibles ;
- tests du document sans GPU ;
- rendu remplaçable ;
- moins de couplage.

---

# 8. Cache de rendu

Le cache est légitime, mais il ne doit pas nécessiter plusieurs mécanismes d'invalidation concurrents sans justification.

Surveiller particulièrement l'accumulation de :

- `version` ;
- `generation` ;
- `dirty` ;
- `in_flight` ;
- `signature` ;
- `invalidate_*` ;
- `warm_cache_*` ;
- `sync_tree()` ;
- compteurs globaux atomiques.

Avant d'ajouter un nouveau flag d'invalidation, déterminer si une clé de cache déterministe peut déjà rendre l'état obsolète naturellement.

Approche préférée :

```rust
struct AppearanceKey {
    layer_id: LayerId,
    source_id: SourceId,
    parameters_hash: u64,
}
```

Ce n'est qu'une direction : ne pas refactorer vers cette forme si les mesures ou le fonctionnement actuel montrent qu'une autre stratégie est meilleure.

---

# 9. PhotoApp : éviter le God Object

`PhotoApp` ne doit pas accumuler toutes les responsabilités du programme.

Objectif de structure :

```rust
pub struct PhotoApp {
    pub document: DocumentState,
    pub canvas: CanvasState,
    pub tools: ToolState,
    pub workspace: WorkspaceState,
    pub rendering: RenderingState,
    pub windows: WindowState,
}
```

Les noms exacts peuvent être différents.

L'objectif est de regrouper l'état par responsabilité :

### DocumentState

- document ;
- sélection liée au document ;
- historique lié au document.

### CanvasState

- zoom ;
- pan ;
- viewport ;
- interaction de canvas.

### ToolState

- outil actif ;
- paramètres du brush ;
- transformation ;
- masque ;
- interaction outil.

### RenderingState

- cache de preview ;
- tâches de rendu ;
- fallback ;
- état GPU lié à l'UI.

### WorkspaceState

- `pane_grid` ;
- panneaux ;
- focus ;
- organisation du workspace.

### WindowState

- préférences ;
- fenêtres secondaires ;
- dialogues.

Ne pas créer de nouvelles structures juste pour réduire le nombre de lignes. Le découpage doit correspondre à une responsabilité réelle.

---

# 10. Messages iced

Si `message.rs` devient trop gros, découper par domaine.

Direction recommandée :

```text
message/
├── mod.rs
├── canvas.rs
├── layers.rs
├── document.rs
├── tools.rs
├── project.rs
├── jobs.rs
└── preferences.rs
```

Avec éventuellement :

```rust
pub enum Message {
    Canvas(CanvasMessage),
    Layers(LayerMessage),
    Document(DocumentMessage),
    Tools(ToolMessage),
    Project(ProjectMessage),
    Jobs(JobMessage),
}
```

Ne pas subdiviser arbitrairement si le nombre de messages est encore petit.

---

# 11. Tâches asynchrones / Jobs

Éviter de créer plusieurs systèmes parallèles du type :

```text
*_in_flight
*_generation
*_dirty
```

pour chaque opération.

Avant d'ajouter un nouveau mécanisme, vérifier s'il peut utiliser le système de tâches existant.

Si une abstraction commune devient réellement nécessaire, envisager un modèle simple :

```rust
JobId
JobKind
JobState
```

avec :

```text
start
→ execute
→ result(JobId, result)
→ finish
```

Ne pas créer un ordonnanceur maison complexe au-dessus de Tokio/Iced sans besoin mesuré.

---

# 12. Historique / Commands

Le système Command est utile pour :

- undo ;
- redo ;
- opérations atomiques ;
- mutations du document.

Mais ne pas y mélanger automatiquement :

- invalidation du renderer ;
- logique GPU ;
- logique UI ;
- scheduling des jobs.

Préférer conceptuellement :

```text
Edit / Command
    ↓
mutation du document

Edit
    ↓
invalidation déterminée

History
    ↓
undo / redo
```

Le détail exact peut rester simple tant que le projet reste petit.

---

# 13. Mathématiques

Auditer les utilitaires mathématiques maison.

Pour chaque type du style :

```text
Vec2
Vec3
Matrix4
Bezier
```

vérifier d'abord si une crate déjà présente ou une dépendance adaptée fournit le comportement voulu.

Une abstraction maison est acceptable lorsqu'elle exprime une sémantique métier, par exemple :

```text
Transform2D du document
```

mais il faut éviter de réimplémenter inutilement des opérations algébriques générales déjà fiables dans une bibliothèque spécialisée.

Toute suppression doit être accompagnée de tests de non-régression lorsque cela est pertinent.

---

# 14. UI Kit

`packages/ui-kit` est un système partagé de composants et de style.

Il doit rester générique.

Approprié :

```text
Panel
PropertyRow
IconButton
Slider
Section
Toolbar
```

À éviter :

```text
PhotoLayerInspector
PhotoDocumentController
PhotoRenderAwareWidget
```

Ces concepts appartiennent à `apps/photo`.

Ne pas transformer le UI kit en second framework UI.

---

# 15. iced / pane_grid

Utiliser les fonctionnalités existantes d'Iced avant d'envelopper ou de réimplémenter :

- layouts ;
- events ;
- canvas ;
- panes ;
- subscriptions ;
- tasks ;
- widgets ;
- styling.

Le `pane_grid` est le mécanisme privilégié pour le workspace de type application professionnelle tant qu'il répond au besoin.

Ne pas développer un système de docking maison sans preuve que `pane_grid` ne suffit pas.

---

# 16. Recherche systématique avant implémentation

Avant d'écrire une nouvelle fonction utilitaire, effectuer une recherche dans :

1. le workspace ;
2. les crates actuellement utilisées ;
3. la documentation officielle de ces crates lorsque le doute existe.

Exemples :

```text
"Cette transformation existe-t-elle déjà ?"
"Iced possède-t-il déjà cet événement ?"
"image fournit-il cette opération ?"
"rayon permet-il déjà ce parallélisme ?"
"wgpu possède-t-il déjà cette abstraction ?"
```

Ne pas considérer une abstraction maison comme justifiée simplement parce qu'elle est plus courte à écrire.

---

# 17. Règles de modification

Pour chaque changement non trivial :

## Étape 1 — Cartographie

Lister :

- fichiers touchés ;
- dépendances ;
- APIs utilisées ;
- tests existants ;
- code potentiellement dupliqué.

## Étape 2 — Simplification avant extension

Avant d'ajouter du code, vérifier si le nouveau besoin peut être résolu en :

- supprimant un helper ;
- fusionnant deux types ;
- réutilisant une API existante ;
- déplaçant une responsabilité ;
- supprimant un état redondant.

## Étape 3 — Modification minimale

Changer le moins de fichiers possible.

## Étape 4 — Compilation et tests

Après modification :

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

Puis lancer les tests spécifiques si le changement concerne un moteur précis.

## Étape 5 — Audit de duplication

Après avoir terminé :

- rechercher les fonctions similaires ;
- rechercher les conversions inutiles ;
- rechercher les wrappers ;
- rechercher les nouveaux flags ;
- rechercher les APIs désormais inutilisées.

## Étape 6 — Rapport

Le compte rendu doit signaler :

- ce qui a été modifié ;
- pourquoi ;
- ce qui a été supprimé ;
- quelles API existantes ont été réutilisées ;
- quels risques subsistent ;
- quels tests ont été exécutés.

---

# 18. Règles anti-code-slop

Le code suivant est suspect et doit être justifié :

### Suspect A — forwarding excessif

```rust
fn foo(&self) { self.inner.foo() }
fn bar(&self) { self.inner.bar() }
fn baz(&self) { self.inner.baz() }
```

### Suspect B — wrapper sans valeur métier

```rust
struct X<T> {
    inner: T,
}
```

si `X` ne fournit aucune invariance ou sémantique utile.

### Suspect C — duplication de fonctions

```text
render_chain()
render_nodes()
render_filters()
render_effects()
```

si plusieurs fonctions font essentiellement la même chose.

### Suspect D — booléens d'état qui se multiplient

```text
loading
in_flight
dirty
pending
processing
needs_update
```

Si plusieurs représentent une même machine à états, remplacer par un état explicite plus simple.

### Suspect E — conversion aller-retour

```text
A → B → A
```

surtout lorsque A et B représentent le même concept.

### Suspect F — cache pour contourner un modèle compliqué

Avant d'ajouter un cache, déterminer pourquoi les recalculs sont nécessaires.

---

# 19. Critère principal de qualité

Le meilleur code n'est pas celui qui contient le plus d'abstractions.

Le meilleur code est celui où un développeur peut répondre rapidement à :

```text
Où est l'état du document ?
Où sont les layers ?
Où est la sélection ?
Où sont les effets ?
Où est le rendu ?
Où est l'historique ?
Où est l'état UI ?
Où est la gestion des tâches ?
```

Chaque responsabilité doit avoir une maison claire.

---

# 20. Architecture souhaitée à terme

La cible conceptuelle est :

```text
                         ┌─────────────────────┐
                         │      Iced UI        │
                         │                     │
                         │ workspace           │
                         │ layers panel        │
                         │ properties          │
                         │ toolbar             │
                         │ canvas interaction  │
                         └──────────┬──────────┘
                                    │
                              Messages / Actions
                                    │
                         ┌──────────▼──────────┐
                         │   Application State │
                         │                     │
                         │ canvas              │
                         │ tools               │
                         │ workspace           │
                         │ rendering           │
                         │ windows             │
                         └──────────┬──────────┘
                                    │
                              Commands / Edits
                                    │
                         ┌──────────▼──────────┐
                         │     Photo Engine    │
                         │                     │
                         │ Document            │
                         │ LayerTree           │
                         │ Effects             │
                         │ History             │
                         │ Compositing         │
                         └──────────┬──────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
             ┌──────▼──────┐                ┌──────▼──────┐
             │ CPU / Image │                │ GPU / WGPU  │
             └─────────────┘                └─────────────┘
```

Le modèle `LayerTree` reste la structure utilisateur principale.

Les effets/filtres sont des traitements internes.

Le renderer ne devient pas propriétaire du document.

L'UI ne devient pas propriétaire du moteur.

---

# 21. Ce qu'il ne faut PAS faire

Ne pas :

- réécrire l'application entière pour "simplifier" ;
- remplacer le LayerTree par un node graph ;
- créer un nouveau système de docking sans nécessité ;
- créer un nouveau système de task scheduling alors qu'Iced/Tokio suffit ;
- créer un nouveau cache avant de comprendre l'ancien ;
- créer plusieurs types représentant le même filtre ;
- implémenter à la main une fonctionnalité clairement fournie par une dépendance ;
- déplacer tout le code dans de nouveaux modules uniquement pour réduire la taille des fichiers ;
- ajouter une abstraction uniquement parce qu'un agent considère le code plus "clean" ;
- supprimer une optimisation sans benchmark ou compréhension de son rôle ;
- modifier une API publique uniquement pour des raisons stylistiques.

---

# 22. Politique spéciale pour les agents IA

Un agent IA doit être conservateur avec ce projet.

Avant chaque modification importante, comparer :

```text
Complexité avant
Complexité après
```

Une modification est suspecte si :

```text
+200 lignes
+3 nouveaux types
+4 nouvelles fonctions helper
+2 nouveaux flags
```

pour une fonctionnalité relativement simple.

Dans ce cas, rechercher d'abord une solution plus directe.

Un agent ne doit pas considérer une compilation réussie comme une preuve que l'architecture est bonne.

La compilation vérifie la validité syntaxique et typée.
Elle ne prouve pas l'absence de :

- duplication ;
- overengineering ;
- mauvais découpage ;
- état redondant ;
- responsabilité mal placée ;
- abstraction inutile.

---

# 23. Definition of Done pour une refactorisation

Une refactorisation est considérée terminée lorsque :

- le comportement existant est conservé ;
- les tests passent ;
- la compilation passe ;
- aucune duplication évidente n'a été introduite ;
- aucune nouvelle abstraction inutile n'a été ajoutée ;
- les anciennes APIs devenues inutiles sont supprimées ;
- le nouveau découpage est plus facile à expliquer ;
- le code est plus facile à modifier localement ;
- les responsabilités sont plus clairement séparées.

Objectif final : **moins de code, moins d'état implicite, moins de duplication et des frontières plus claires**, sans sacrifier les capacités nécessaires à un éditeur photo professionnel.
