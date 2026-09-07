// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2026 vabyz971
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! État applicatif Photo : document LayerTree, canvas, outils, préférences.

use iced::widget::{image as iced_image, pane_grid};
use iced::{Color, Rectangle, Size, Task, Vector};
use uuid::Uuid;

use crate::components;
use crate::message::{Message, PanelType, PendingPaint, Tool};

/// Registre des traitements en arrière-plan.
///
/// Le spinner de la barre haute (`shell::task_indicator`) tourne tant que
/// ce registre n'est pas vide et le menu déroulant liste chaque libellé.
/// Les identifiants stables évitent les fuites (« créer un calque vide… »
/// resté affiché pour toujours) et les effacements par une tâche concurrente.
#[derive(Default)]
pub struct BackgroundTasks {
    next_id: u64,
    items: Vec<(u64, String)>,
}

impl BackgroundTasks {
    /// Pousse le libellé d'une nouvelle tâche et retourne son identifiant —
    /// à passer à [`Self::finish`] quand la tâche se termine, succès ou échec.
    pub fn start(&mut self, label: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.items.push((id, label.into()));
        id
    }

    /// Retire la tâche `id` (non-op si déjà retirée).
    pub fn finish(&mut self, id: u64) {
        self.items.retain(|(i, _)| *i != id);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Libellés des tâches en cours, pour l'affichage du menu.
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|(_, l)| l.as_str())
    }
}

pub struct PhotoApp {
    pub zoom_level: u32,
    pub panes: pane_grid::State<PanelType>,
    pub focus: Option<pane_grid::Pane>,
    // ---- Document (arbre de calques — index 0 racine = bas de la pile) ----
    pub doc: photo_engine::Document,
    pub selected_layer: Option<Uuid>,
    /// Taille du composite fallback (modes de fusion non-Normal)
    pub fallback_size: Option<Size>,
    /// Composite CPU unique — UNIQUEMENT si l'arbre exige du blending
    /// inter-calques (sinon chemin rapide par calque, zéro recomposite)
    pub fallback_handle: Option<iced_image::Handle>,
    // ---- Pipeline fallback ASYNCHRONE ----
    /// Composite de fond en cours / périmée : un seul état explicite au lieu
    /// des trois drapeaux (`generation` / `in_flight` / `dirty`) qui se
    /// désynchronisaient. Le compteur monotone vit dans la branche `Running`
    /// pour distinguer un résultat STALE d'un résultat à jour.
    pub(crate) fallback_job: FallbackJob,
    pub image_path: Option<String>,
    pub image_error: Option<String>,
    // Canvas interaction (outil Main + pan/zoom)
    pub selected_tool: Tool,
    /// Outil mémorisé avant la pipette : revenu automatique après l'échantillonnage.
    pub previous_tool: Option<Tool>,
    pub canvas_pan: Vector,
    pub color_profile: String,
    pub canvas_selection: Option<Rectangle>,
    /// Taille du viewport du canvas (publiée par le widget image_canvas)
    pub canvas_viewport: Size,
    /// Barre d'outils flottante visible ou masquée
    pub tools_visible: bool,
    /// Ancre de déplacement du calque sélectionné (outil Déplacer) :
    /// transform COMPLET au début du geste — sert à construire la commande
    /// SetTransform ancre→finale poussée au relâchement.
    pub move_anchor: Option<(Uuid, crate::layers::Transform2D)>,
    /// Ancre du geste de transformation en cours (poignées Affinity).
    /// `Some(anchor)` ⇔ un geste Resize/Rotate/Skew/Move est actif.
    pub(crate) transform_anchor: Option<TransformAnchor>,
    /// Fond composite PRÉ-CALCULÉ au début du drag (sans le calque déplacé).
    /// Pendant le drag : zéro recomposite — on dessine ce fond + le calque
    /// par-dessus. Le vrai blend est recalculé au relâchement.
    pub drag_background: Option<iced_image::Handle>,
    pub drag_background_size: Option<Size>,
    /// Composite du calque seul (avec son masque) pré-calculé HORS thread UI
    /// pour les drags en mode fallback. Évite de re-rendre le calque à chaque
    /// frame (60 fps) tout en préservant le rendu du masque — le buffer est
    /// calculé UNE fois au MoveLayerStart, puis réutilisé pour le geste.
    pub drag_layer_composite: Option<iced_image::Handle>,
    pub drag_layer_composite_size: Option<Size>,
    /// Pré-calcul du fond SANS le sous-arbre déplacé (drag en mode fallback).
    pub(crate) drag_bg_job: DragBgJob,
    /// Verrou anti-doublon pour [`Self::drag_layer_composite_task`] — un
    /// calque cible à la fois (le buffer est réutilisé pendant tout le geste).
    pub(crate) drag_layer_job: DragLayerJob,
    /// Traitements en arrière-plan (libellés affichés dans le menu du spinner).
    /// Registre par identifiant stable : chaque tâche asynchrone pousse son
    /// libellé à la création (`start`) et le retire à SON aboutissement ou à
    /// SON échec (`finish`) — aucun libellé ne fuit, quelle que soit la
    /// concurrence (un `.clear()` global effaçait les tâches concurrentes).
    pub background_tasks: BackgroundTasks,
    /// Menu des tâches ouvert (clic sur le spinner)
    pub task_menu_open: bool,
    /// Angle du spinner d'activité (animé par TickFrame)
    pub spinner_angle: f32,
    /// Résolveur de raccourcis construit depuis les préférences persistantes
    pub resolver: preferences::KeybindingResolver,
    /// Fenêtre principale — son Id (ouverte au boot)
    pub main_window: Option<iced::window::Id>,
    // ---- Historique (undo/redo) ----
    /// Historique du DOCUMENT (arbre + dimensions). Les états sont des
    /// snapshots bon marché : les pixels sont partagés via Arc.
    pub history: photo_engine::history::History,
    /// Chemin du projet .csophoto courant (None = jamais enregistré)
    pub project_path: Option<std::path::PathBuf>,
    // ---- Pinceau ----
    pub brush_color: Color,
    /// Diamètre du pinceau en pixels DOCUMENT
    pub brush_size: f32,
    /// Opacité globale du trait [0.05..1]
    pub brush_opacity: f32,
    pub color_picker_open: bool,
    /// Masque actuellement sélectionné pour édition/peinture, s'il y en a un.
    pub active_mask: Option<crate::message::MaskTarget>,
    /// Couleur du pinceau en mode masque : true = noir (masque), false = blanc (révèle).
    pub mask_brush_black: bool,
    /// Calques dont la liste de masques est dépliée dans le panneau Calques.
    pub expanded_masks: std::collections::HashSet<Uuid>,
    /// Calques pixels dont les sous-calques de filtres sont dépliés.
    pub expanded_filters: std::collections::HashSet<Uuid>,
    /// Menu d'ajout de filtre du panneau Calques ouvert/fermé.
    pub filter_menu_open: bool,
    /// Trait en cours : calque cible + polyligne en coordonnées DOCUMENT
    pub stroke_layer: Option<Uuid>,
    /// Commit lourd EN COURS hors thread UI — l'aperçu reste figé à l'écran
    /// jusqu'à l'application (aucun gel de l'interface).
    pub pending_paint: Option<PendingPaint>,

    // ---- Écran d'accueil (nouveau document) ----
    pub new_doc_w: String,
    pub new_doc_h: String,
    pub welcome_error: Option<String>,

    // ---- Redimensionnement document ----
    pub resize_dialog_open: bool,
    pub resize_w: String,
    pub resize_h: String,

    // ---- Drag & drop calques ----
    pub dragged_layer: Option<Uuid>,

    /// Id de la VRAIE fenêtre OS des préférences (multi-fenêtres daemon)
    pub preferences_window_id: Option<iced::window::Id>,
    /// État interne de la fenêtre (brouillon de préférences)
    pub preferences_window: Option<crate::preferences_window::PreferencesWindow>,
    /// Préférences persistantes chargées au démarrage
    pub preferences: preferences::Preferences,
    // Options / Hardware
    pub gpu_info: Option<String>,
    pub gpu_available: bool,

    /// Handles iced par calque (cache dérivé des buffers purs du moteur —
    /// voir `crate::ui_handles` ; purgé/reconstruit automatiquement).
    pub preview_cache: crate::ui_handles::PreviewCache,
}

impl PhotoApp {
    /// Boot daemon : le daemon n'ouvre AUCUNE fenêtre automatiquement —
    /// la fenêtre principale doit être créée ici via `window::open`
    /// (cf. iced examples/multi_window).
    // L'Id de fenêtre n'existe qu'APRÈS `Self::default()` (layout des
    // panneaux) : la réassignation est le pattern demandé par iced.
    #[allow(clippy::field_reassign_with_default)]
    pub fn new() -> (Self, Task<Message>) {
        let (main_id, open) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(960.0, 600.0)),
            ..iced::window::Settings::default()
        });
        let mut app = Self::default();
        app.main_window = Some(main_id);
        app.history.reset();
        (app, open.map(|_| Message::MockAction))
    }

    /// Dimensions du document si un document existe (sinon None).
    pub(crate) fn doc_dims(&self) -> Option<(u32, u32)> {
        (self.doc.width > 0 && self.doc.height > 0).then_some((self.doc.width, self.doc.height))
    }

    /// Snapshot complet du document pour l'historique (pixels partagés via Arc).
    pub(crate) fn snapshot(&self) -> photo_engine::history::Snapshot {
        self.doc.snapshot()
    }

    /// Cette fenêtre est-elle celle des préférences ?
    #[must_use]
    pub fn is_preferences_window(&self, window: iced::window::Id) -> bool {
        self.preferences_window_id == Some(window)
    }

    /// Ferme la fenêtre de préférences (état + surface OS) et retourne
    /// la tâche de fermeture à exécuter par le runtime.
    pub(crate) fn close_preferences_window(&mut self) -> Task<Message> {
        self.preferences_window = None;
        match self.preferences_window_id.take() {
            Some(id) => iced::window::close(id),
            None => Task::none(),
        }
    }

    /// L'arbre exige-t-il la composite CPU ? (groupes en mode non-Normal,
    /// calques d'ajustement actifs, calques non-Normal) — délégué moteur.
    pub(crate) fn needs_fallback(&self) -> bool {
        self.doc.needs_fallback()
    }

    /// Marque le fallback PÉRIMÉ. Zéro travail bloquant : la composite
    /// sera produite hors thread UI par [`Self::take_fallback_task`] au
    /// prochain passage de boucle. Si le chemin rapide suffit, on purge
    /// simplement les handles.
    pub(crate) fn invalidate_fallback(&mut self) {
        if self.needs_fallback() {
            self.fallback_job.invalidate();
        } else {
            self.fallback_job.reset_to_idle();
            self.fallback_handle = None;
            self.fallback_size = None;
        }
    }

    /// Si une composite est requise et aucune n'est en vol : lance le
    /// calcul HORS thread UI (jamais sur le thread interface). Le résultat
    /// revient par [`Message::FallbackComputed`] avec sa génération —
    /// un résultat périmé est jeté et une nouvelle tournée repart.
    pub(crate) fn take_fallback_task(&mut self) -> Option<Task<Message>> {
        if !self.needs_fallback() {
            return None;
        }
        let generation = self.fallback_job.start_new_run()?;

        let task_id = self.background_tasks.start("Composite de l'arbre...");

        let mut doc_copy = photo_engine::Document::new(self.doc.width, self.doc.height);
        doc_copy.restore_snapshot(self.doc.snapshot());
        // Réchauffe le cache d'apparences depuis le document vivant : la
        // composite de fond HIT les calques inchangés au lieu de re-exécuter
        // les chaînes de rendu (déjà calculées pour l'affichage).
        doc_copy.warm_cache_from(&self.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match doc_copy.composite_preview() {
                    Some(img) => Ok(Some((img.to_rgba8().into_raw(), img.width(), img.height()))),
                    None => Ok(None),
                })
                .await
                .map_err(|e| format!("Tâche annulée : {e}"))?
            },
            move |result| Message::FallbackComputed {
                task_id,
                generation,
                result,
            },
        ))
    }

    /// Pré-calcule le fond composite SANS le sous-arbre sur le point d'être
    /// déplacé — HORS thread UI également. Pendant les quelques millisecondes
    /// de calcul, le drag s'affiche déjà en dessin calque-par-calque
    /// (approximation), puis le fond exact remplace l'approximation.
    pub(crate) fn drag_background_task(&mut self, exclude_id: Uuid) -> Option<Task<Message>> {
        debug_assert!(self.needs_fallback());
        if !self.drag_bg_job.try_start(exclude_id) {
            return None;
        }
        let task_id = self.background_tasks.start("Fond de glissement...");

        let mut doc_copy = photo_engine::Document::new(self.doc.width, self.doc.height);
        doc_copy.restore_snapshot(self.doc.snapshot());
        doc_copy.warm_cache_from(&self.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    doc_copy
                        .composite_preview_without(exclude_id)
                        .map(|img| (img.to_rgba8().into_raw(), img.width(), img.height()))
                })
                .await
                .unwrap_or(None)
            },
            move |result| Message::DragBackgroundComputed {
                task_id,
                layer_id: exclude_id,
                result,
            },
        ))
    }

    /// Calcule EN ARRIÈRE-PLAN le composite du calque seul AVEC son masque
    /// appliqué (mode Normal uniquement — le blend final du calque dans le
    /// document est recalculé au relâchement via [`Self::invalidate_fallback`]).
    /// Lancé une seule fois au début du drag en mode fallback : le buffer
    /// est réutilisé pour toutes les frames suivantes, car le calque change
    /// de POSITION (pas de pixels) pendant le geste.
    pub(crate) fn drag_layer_composite_task(&mut self, layer_id: Uuid) -> Option<Task<Message>> {
        if !self.needs_fallback() {
            return None;
        }
        if !self.drag_layer_job.try_start() {
            return None;
        }
        let task_id = self.background_tasks.start("Rendu du calque déplacé...");

        let mut doc_copy = photo_engine::Document::new(self.doc.width, self.doc.height);
        doc_copy.restore_snapshot(self.doc.snapshot());
        doc_copy.warm_cache_from(&self.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    // clone() le calque est sans Arc donc peu coûteux (les
                    // buffers partagés ne sont pas dupliqués) ; on isole le
                    // calque dans un documentjeté pour utiliser le pipeline
                    // standard de compositing (prepare_top + combine_masks +
                    // blend_into sur fond transparent).
                    let mut tmp = photo_engine::Document::new(doc_copy.width, doc_copy.height);
                    // Le cache de doc_copy est chaud (réchauffé depuis le doc
                    // vivant) : on le partage avec tmp pour ne pas re-rendre
                    // l'apparence du calque déplacé.
                    tmp.warm_cache_from(&doc_copy);
                    if let Some(node) = doc_copy.find(layer_id).cloned() {
                        tmp.root.push(node);
                        tmp.composite_preview()
                            .map(|img| (img.to_rgba8().into_raw(), img.width(), img.height()))
                    } else {
                        None
                    }
                })
                .await
                .unwrap_or(None)
            },
            move |result| Message::DragLayerCompositeComputed {
                task_id,
                layer_id,
                result,
            },
        ))
    }
}

impl Default for PhotoApp {
    fn default() -> Self {
        // Layout : Canvas à gauche, à droite Propriétés (haut) + Calques (bas).
        // split() ne peut échouer que si le pane source n'existe pas ; en
        // cas d'imprvu on garde simplement le pane unique (pas de panic).
        let (mut panes, canvas_pane) = pane_grid::State::new(PanelType::Canvas);
        if let Some((right_pane, split_canvas_right)) = panes.split(
            pane_grid::Axis::Vertical,
            canvas_pane,
            PanelType::Properties,
        ) {
            panes.resize(split_canvas_right, 0.74);
            if let Some((_layers_pane, split_right_panel)) =
                panes.split(pane_grid::Axis::Horizontal, right_pane, PanelType::Layers)
            {
                panes.resize(split_right_panel, 0.55);
            }
        }

        Self {
            zoom_level: 100,
            panes,
            focus: Some(canvas_pane),
            doc: photo_engine::Document::new(0, 0),
            selected_layer: None,
            fallback_size: None,
            fallback_handle: None,
            fallback_job: FallbackJob::Idle,
            image_path: None,
            image_error: None,
            selected_tool: Tool::Hand,
            previous_tool: None,
            canvas_pan: Vector::new(0.0, 0.0),
            color_profile: "sRGB IEC61966-2.1".into(),
            canvas_selection: None,
            canvas_viewport: Size::new(800.0, 600.0),
            tools_visible: true,
            move_anchor: None,
            transform_anchor: None,
            drag_background: None,
            drag_background_size: None,
            drag_layer_composite: None,
            drag_layer_composite_size: None,
            drag_bg_job: DragBgJob::Idle,
            drag_layer_job: DragLayerJob::Idle,
            background_tasks: BackgroundTasks::default(),
            task_menu_open: false,
            spinner_angle: 0.0,
            resolver: preferences::KeybindingResolver::from_bindings(
                &preferences::Preferences::load("photo").keybindings.bindings,
            ),
            main_window: None,
            history: photo_engine::history::History::new(),
            project_path: None,
            brush_color: ui_kit::theme::colors::BRUSH_DEFAULT,
            brush_size: 12.0,
            brush_opacity: 1.0,
            color_picker_open: false,
            active_mask: None,
            mask_brush_black: true,
            expanded_masks: Default::default(),
            expanded_filters: Default::default(),
            filter_menu_open: false,
            stroke_layer: None,
            pending_paint: None,
            new_doc_w: "1920".to_string(),
            new_doc_h: "1080".to_string(),
            welcome_error: None,
            resize_dialog_open: false,
            resize_w: String::new(),
            resize_h: String::new(),
            dragged_layer: None,
            preferences_window_id: None,
            preferences_window: None,
            preferences: preferences::Preferences::load("photo"),
            gpu_info: None,
            gpu_available: components::gpu::GpuContext::is_available(),
            preview_cache: crate::ui_handles::PreviewCache::default(),
        }
    }
}

/// Ancre d'un geste de transformation en cours (poignées du visualiseur).
/// Le workflow est ancre→curseur→fin : `base` capture le transform complet au
/// début, `cursor_doc` la position document du press.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransformAnchor {
    pub layer_id: Uuid,
    pub kind: ui_kit::image_canvas::TransformHandle,
    /// Transform COMPLET au début du geste (pour la commande ancre→fin).
    pub base: crate::layers::Transform2D,
    /// Position curseur document au début du geste.
    pub cursor_doc: (f32, f32),
}

// ---------------------------------------------------------------------------
// États explicites des jobs de rendu asynchrones (AGENT §8, §11).
// Avant : 3 champs plats (`_in_flight`, `_dirty`, `_generation`) qui pouvaient
// se désynchroniser. Maintenant : un enum par job — la machine à états est
// dans le type, pas dans la tête du lecteur.
// ---------------------------------------------------------------------------

/// Composite de fond (blend inter-calques). Compteur monotone inclus pour
/// jeter un résultat calculé avec une génération antérieure (le document a
/// changé pendant que la tâche tournait).
#[derive(Default)]
pub(crate) enum FallbackJob {
    #[default]
    Idle,
    /// Tâche en vol ; un résultat qui reviendrait avec une `generation`
    /// différente serait périmé. La branche `Dirty` signale qu'une
    /// recomposite supplémentaire sera nécessaire au retour.
    Running { generation: u64, dirty: bool },
}

impl FallbackJob {
    /// Édition signalée — la composite affichée devient périmée.
    pub(crate) fn invalidate(&mut self) {
        match self {
            Self::Idle => {
                *self = Self::Running {
                    generation: 0,
                    dirty: true,
                }
            }
            Self::Running { dirty, .. } => *dirty = true,
        }
    }

    /// Purge l'état (mode rapide actif : pas de composite à refaire).
    pub(crate) fn reset_to_idle(&mut self) {
        *self = Self::Idle;
    }

    /// Lance un nouveau calcul IFF une invalidation est en attente. Retourne
    /// la génération attribuée, ou `None` si rien à faire (déjà en vol, ou
    /// rien d'invalidé). Appelé à chaque tour de boucle après un message.
    pub(crate) fn start_new_run(&mut self) -> Option<u64> {
        match self {
            Self::Idle => None, // rien d'invalidé, on n'invalide pas nous-mêmes
            Self::Running { dirty: false, .. } => None, // déjà en vol, pas périmé
            Self::Running { generation, .. } => {
                let next = generation.wrapping_add(1);
                *self = Self::Running {
                    generation: next,
                    dirty: false,
                };
                Some(next)
            }
        }
    }

    /// Le calcul est-il en vol (sans tenir compte de l'invalidation) ?
    #[allow(dead_code)] // observé par les tests d'invalidation du drag
    pub(crate) fn in_flight(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// Le calcul affiché est-il périmé (édition pendant le vol) ?
    #[allow(dead_code)] // observé par les tests d'invalidation du drag
    pub(crate) fn needs_recompute(&self) -> bool {
        match self {
            Self::Idle => false,
            Self::Running { dirty, .. } => *dirty,
        }
    }

    /// Tâche terminée. Retourne un verdict :
    /// - [`Finish::Applied`] : résultat à jour, l'appliquer.
    /// - [`Finish::Retry`] : résultat périmé (édition pendant le vol) — le
    ///   caller doit relancer via [`Self::start_new_run`].
    /// - [`Finish::Stale`] : callback obsolète (une nouvelle tâche a déjà
    ///   été lancée entre-temps), aucun travail.
    pub(crate) fn finish(&mut self, generation: u64) -> Finish {
        match self {
            Self::Running {
                generation: g,
                dirty,
            } if *g == generation => {
                if *dirty {
                    *self = Self::Running {
                        generation: *g,
                        dirty: false,
                    };
                    Finish::Retry
                } else {
                    *self = Self::Idle;
                    Finish::Applied
                }
            }
            _ => Finish::Stale,
        }
    }
}

/// Verdict de [`FallbackJob::finish`].
pub(crate) enum Finish {
    Applied,
    Retry,
    Stale,
}

/// Pré-calcul du fond SANS le sous-arbre déplacé (drag en mode fallback).
/// Un seul vol à la fois ; l'id du sous-arbre est conservé pour reconnaître
/// un résultat obsolète quand le geste change de cible.
#[derive(Default)]
pub(crate) enum DragBgJob {
    #[default]
    Idle,
    Running(Uuid),
}

impl DragBgJob {
    /// `true` si on a pu démarrer (aucune tâche en cours).
    pub(crate) fn try_start(&mut self, exclude_id: Uuid) -> bool {
        if matches!(self, Self::Idle) {
            *self = Self::Running(exclude_id);
            true
        } else {
            false
        }
    }

    pub(crate) fn finish(&mut self) {
        *self = Self::Idle;
    }

    pub(crate) fn is_running(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    /// Le calcul lancé pour cet id est-il toujours celui qu'on attend ?
    pub(crate) fn is_running_for(&self, id: Uuid) -> bool {
        matches!(self, Self::Running(x) if *x == id)
    }
}

/// Composite du calque seul AVEC masque — surimpression pendant le drag
/// en mode fallback. Un seul vol à la fois ; la pertinence du résultat est
/// vérifiée côté handler via `move_anchor` (le sous-arbre peut avoir changé).
#[derive(Default)]
pub(crate) enum DragLayerJob {
    #[default]
    Idle,
    Running,
}

impl DragLayerJob {
    pub(crate) fn try_start(&mut self) -> bool {
        if matches!(self, Self::Idle) {
            *self = Self::Running;
            true
        } else {
            false
        }
    }

    pub(crate) fn finish(&mut self) {
        *self = Self::Idle;
    }

    #[allow(dead_code)] // observé par les tests d'invalidation du drag
    pub(crate) fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}
