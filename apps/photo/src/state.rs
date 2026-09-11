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

//! État applicatif Photo : regroupé par responsabilité (AGENT §9) :
//! - [`DocumentState`] : arbre de calques, sélection, historique, projet
//! - [`CanvasState`] : zoom, pan, viewport, état d'image, barre d'outils
//! - [`ToolState`] : outil actif, pinceau, transformation, masques, dialogues
//! - [`RenderingState`] : cache de preview, tâches de rendu, fallback, GPU
//! - [`WorkspaceState`] : `pane_grid` et focus
//! - [`WindowState`] : fenêtres secondaires, préférences, raccourcis
//!
//! [`PhotoApp`] reste l'entrée unique pour iced — ses méthodes orchestrent
//! les sous-états sans devenir un god object (un domaine = un struct).

use iced::widget::{image as iced_image, pane_grid};
use iced::{Color, Rectangle, Size, Task, Vector};
use uuid::Uuid;

use crate::components;
use crate::message::{Message, PanelType, PendingPaint, Tool};

// ---------------------------------------------------------------------------
// Sous-états (AGENT §9) — un struct par responsabilité réelle.
// ---------------------------------------------------------------------------

/// État du document : l'arbre de calques, la sélection, l'historique,
/// le chemin du projet. Tout ce qui survit à un changement d'outil.
pub struct DocumentState {
    /// Arbre de calques (index 0 = bas de la pile).
    pub doc: photo_engine::Document,
    pub selected_layer: Option<Uuid>,
    /// Historique hybride (snapshots + commandes légères).
    pub history: photo_engine::history::History,
    pub project_path: Option<std::path::PathBuf>,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            doc: photo_engine::Document::new(0, 0),
            selected_layer: None,
            history: photo_engine::history::History::new(),
            project_path: None,
        }
    }
}

/// État du canvas : géométrie d'affichage, état d'image, navigation,
/// visibilité de la barre d'outils flottante.
pub struct CanvasState {
    pub zoom_level: u32,
    pub canvas_pan: Vector,
    /// Publiée par le widget image_canvas.
    pub canvas_viewport: Size,
    pub canvas_selection: Option<Rectangle>,
    pub color_profile: String,
    pub image_path: Option<String>,
    pub image_error: Option<String>,
    pub tools_visible: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            zoom_level: 100,
            canvas_pan: Vector::new(0.0, 0.0),
            canvas_viewport: Size::new(800.0, 600.0),
            canvas_selection: None,
            color_profile: "sRGB IEC61966-2.1".into(),
            image_path: None,
            image_error: None,
            tools_visible: true,
        }
    }
}

/// État de l'outil actif : outil sélectionné, pinceau, transformation en
/// cours, masque actif, dialogues de création/édition de document.
pub struct ToolState {
    pub selected_tool: Tool,
    /// Outil mémorisé avant la pipette — revenu automatique après l'échantillon.
    pub previous_tool: Option<Tool>,
    /// Transform COMPLET au début du geste Déplacer — sert à construire la
    /// commande `SetTransform` ancre→finale poussée au relâchement.
    pub move_anchor: Option<(Uuid, crate::layers::Transform2D)>,
    /// Ancre du geste de transformation (poignées Affinity).
    pub(crate) transform_anchor: Option<TransformAnchor>,
    pub brush_color: Color,
    pub brush_size: f32,
    pub brush_opacity: f32,
    /// Cran de rotation aimantée (degrés) — outil Sélection (Ctrl).
    pub rotation_step: f32,
    /// Grille du déplacement aimanté (outil Sélection) — `false` = libre.
    pub move_grid_enabled: bool,
    /// Taille de la grille d'aimantation (px document).
    pub move_grid_size: f32,
    pub color_picker_open: bool,
    pub active_mask: Option<crate::message::MaskTarget>,
    /// `true` = noir (masque), `false` = blanc (révèle).
    pub mask_brush_black: bool,
    /// Pile FX dépliée par porteur (calque) — masques + sous-calques de
    /// filtres partagent désormais une seule liste commune sous chaque
    /// calque, d'où un seul `HashSet` pour la mémoisation du dépliage.
    pub expanded_fx_stack: std::collections::HashSet<Uuid>,
    pub filter_menu_open: bool,
    pub stroke_layer: Option<Uuid>,
    pub pending_paint: Option<PendingPaint>,
    pub dragged_layer: Option<Uuid>,
    // Écran d'accueil
    pub new_doc_w: String,
    pub new_doc_h: String,
    pub welcome_error: Option<String>,
    // Redimensionnement
    pub resize_dialog_open: bool,
    pub resize_w: String,
    pub resize_h: String,
    /// État du menu contextuel sur le calque (clic droit dans panneau calques).
    pub context_menu_open: Option<Uuid>, // calque ciblé, None = fermer
    pub context_menu_pos: (f32, f32), // position souris
    /// Texture de LOUPE courante de la pipette (patch grossi au curseur).
    pub pick_loupe: Option<ui_kit::image_canvas::LoupeTex>,
    /// Un échantillonnage de patch est en vol (garde anti-empilement).
    pub loupe_sample_pending: bool,
    /// Identifiant du dernier patch demandé — filtre les arrivages périmés.
    pub loupe_last_resp: Option<u64>,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            selected_tool: Tool::Hand,
            previous_tool: None,
            move_anchor: None,
            transform_anchor: None,
            brush_color: ui_kit::theme::colors::BRUSH_DEFAULT,
            brush_size: 12.0,
            brush_opacity: 1.0,
            rotation_step: 5.0,
            move_grid_enabled: false,
            move_grid_size: 10.0,
            color_picker_open: false,
            active_mask: None,
            mask_brush_black: true,
            expanded_fx_stack: std::collections::HashSet::new(),
            filter_menu_open: false,
            stroke_layer: None,
            pending_paint: None,
            dragged_layer: None,
            new_doc_w: "1920".to_string(),
            new_doc_h: "1080".to_string(),
            welcome_error: None,
            resize_dialog_open: false,
            resize_w: String::new(),
            resize_h: String::new(),
            context_menu_open: None,
            context_menu_pos: (0.0, 0.0),
            pick_loupe: None,
            loupe_sample_pending: false,
            loupe_last_resp: None,
        }
    }
}

/// État du pipeline de rendu : cache UI, jobs asynchrones, GPU détecté,
/// indicateurs d'activité (spinner, menu tâches).
pub struct RenderingState {
    /// Taille du composite fallback (modes de fusion non-Normal).
    pub fallback_size: Option<Size>,
    /// Composite CPU unique — UNIQUEMENT si l'arbre exige du blending
    /// inter-calques (sinon chemin rapide par calque, zéro recomposite).
    pub fallback_handle: Option<iced_image::Handle>,
    /// Fond composite PRÉ-CALCULÉ au début du drag (sans le calque déplacé).
    /// Pendant le drag : zéro recomposite — on dessine ce fond + le calque
    /// par-dessus. Le vrai blend est recalculé au relâchement.
    pub drag_background: Option<iced_image::Handle>,
    pub drag_background_size: Option<Size>,
    /// Composite du calque seul (avec son masque) pré-calculé HORS thread UI
    /// pour les drags en mode fallback.
    pub drag_layer_composite: Option<iced_image::Handle>,
    pub drag_layer_composite_size: Option<Size>,
    /// Pipeline asynchrone (états explicites — voir PR1).
    pub fallback_job: FallbackJob,
    pub drag_bg_job: DragBgJob,
    pub drag_layer_job: DragLayerJob,
    /// Handles iced par calque (cache dérivé des buffers purs du moteur).
    pub preview_cache: crate::ui_handles::PreviewCache,
    pub gpu_info: Option<String>,
    pub gpu_available: bool,
    pub spinner_angle: f32,
    pub task_menu_open: bool,
    pub background_tasks: BackgroundTasks,
}

impl Default for RenderingState {
    fn default() -> Self {
        Self {
            fallback_size: None,
            fallback_handle: None,
            drag_background: None,
            drag_background_size: None,
            drag_layer_composite: None,
            drag_layer_composite_size: None,
            fallback_job: FallbackJob::Idle,
            drag_bg_job: DragBgJob::Idle,
            drag_layer_job: DragLayerJob::Idle,
            preview_cache: crate::ui_handles::PreviewCache::default(),
            gpu_info: None,
            gpu_available: components::gpu::GpuContext::is_available(),
            spinner_angle: 0.0,
            task_menu_open: false,
            background_tasks: BackgroundTasks::default(),
        }
    }
}

/// État du workspace : layout `pane_grid` et focus du panneau actif.
pub struct WorkspaceState {
    pub panes: pane_grid::State<PanelType>,
    pub focus: Option<pane_grid::Pane>,
}

/// État des fenêtres OS et des préférences multi-fenêtres.
pub struct WindowState {
    pub main_window: Option<iced::window::Id>,
    pub preferences_window_id: Option<iced::window::Id>,
    pub preferences_window: Option<crate::preferences_window::PreferencesWindow>,
    pub preferences: preferences::Preferences,
    pub resolver: preferences::KeybindingResolver,
}

// ---------------------------------------------------------------------------
// PhotoApp — façade mince qui orchestre les sous-états.
// ---------------------------------------------------------------------------

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

/// Résolution de vue des buffers de scène (fallback / fond de drag) — alignée
/// sur la preview 2048 des calques du chemin rapide. Une composite pleine
/// scène peut dépasser largement (bornes du plan infini, clamp 16384 du
/// moteur) : l'afficher sans downscale change la résolution perçue et charge
/// la VRAM. L'export et `sample_color` restent sur la résolution pleine.
const SCENE_DISPLAY_MAX: u32 = 2048;

/// Plafonne un buffer RGBA d'affichage à [`SCENE_DISPLAY_MAX`] de côté
/// (échantillonnage Triangle, centré). Buffer déjà dans les limites → 1:1.
fn fit_scene_display(rgba: Vec<u8>, w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    if w.max(h) <= SCENE_DISPLAY_MAX {
        return (rgba, w, h);
    }
    let Some(img) = image::RgbaImage::from_vec(w, h, rgba) else {
        return (Vec::new(), 0, 0);
    };
    let nw = ((w as f32 * (SCENE_DISPLAY_MAX as f32 / w.max(h) as f32)).round() as u32).max(1);
    let nh = ((h as f32 * (SCENE_DISPLAY_MAX as f32 / w.max(h) as f32)).round() as u32).max(1);
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    (resized.into_raw(), nw, nh)
}

pub struct PhotoApp {
    pub document: DocumentState,
    pub canvas: CanvasState,
    pub tools: ToolState,
    pub rendering: RenderingState,
    pub workspace: WorkspaceState,
    pub windows: WindowState,
}

impl PhotoApp {
    /// Boot daemon : le daemon n'ouvre AUCUNE fenêtre automatiquement —
    /// la fenêtre principale doit être créée ici via `window::open`
    /// (cf. iced examples/multi_window).
    #[allow(clippy::field_reassign_with_default)]
    pub fn new() -> (Self, Task<Message>) {
        let (main_id, open) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(960.0, 600.0)),
            ..iced::window::Settings::default()
        });
        let mut app = Self::default();
        app.windows.main_window = Some(main_id);
        app.document.history.reset();
        (app, open.map(|_| Message::MockAction))
    }

    /// Dimensions du document si un document existe (sinon None).
    pub(crate) fn doc_dims(&self) -> Option<(u32, u32)> {
        let w = self.document.doc.width;
        let h = self.document.doc.height;
        (w > 0 && h > 0).then_some((w, h))
    }

    /// Snapshot complet du document pour l'historique (pixels partagés via Arc).
    pub(crate) fn snapshot(&self) -> photo_engine::history::Snapshot {
        self.document.doc.snapshot()
    }

    /// Cette fenêtre est-elle celle des préférences ?
    #[must_use]
    pub fn is_preferences_window(&self, window: iced::window::Id) -> bool {
        self.windows.preferences_window_id == Some(window)
    }

    /// Ferme la fenêtre de préférences (état + surface OS) et retourne
    /// la tâche de fermeture à exécuter par le runtime.
    pub(crate) fn close_preferences_window(&mut self) -> Task<Message> {
        self.windows.preferences_window = None;
        match self.windows.preferences_window_id.take() {
            Some(id) => iced::window::close(id),
            None => Task::none(),
        }
    }

    /// L'arbre exige-t-il la composite CPU ? (groupes en mode non-Normal,
    /// calques d'ajustement actifs, calques non-Normal) — délégué moteur.
    pub(crate) fn needs_fallback(&self) -> bool {
        self.document.doc.needs_fallback()
    }

    /// Marque le fallback PÉRIMÉ. Zéro travail bloquant : la composite
    /// sera produite hors thread UI par [`Self::take_fallback_task`] au
    /// prochain passage de boucle. Si le chemin rapide suffit, on purge
    /// simplement les handles.
    pub(crate) fn invalidate_fallback(&mut self) {
        if self.needs_fallback() {
            self.rendering.fallback_job.invalidate();
        } else {
            self.rendering.fallback_job.reset_to_idle();
            self.rendering.fallback_handle = None;
            self.rendering.fallback_size = None;
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
        let generation = self.rendering.fallback_job.start_new_run()?;

        let task_id = self
            .rendering
            .background_tasks
            .start("Composite de l'arbre...");

        let mut doc_copy =
            photo_engine::Document::new(self.document.doc.width, self.document.doc.height);
        doc_copy.restore_snapshot(self.document.doc.snapshot());
        doc_copy.warm_cache_from(&self.document.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match doc_copy.composite_preview() {
                    Some(img) => {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let (data, w2, h2) = fit_scene_display(rgba.into_raw(), w, h);
                        Ok(Some((data, w2, h2)))
                    }
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
    /// déplacé — HORS thread UI également.
    pub(crate) fn drag_background_task(&mut self, exclude_id: Uuid) -> Option<Task<Message>> {
        debug_assert!(self.needs_fallback());
        if !self.rendering.drag_bg_job.try_start(exclude_id) {
            return None;
        }
        let task_id = self
            .rendering
            .background_tasks
            .start("Fond de glissement...");

        let mut doc_copy =
            photo_engine::Document::new(self.document.doc.width, self.document.doc.height);
        doc_copy.restore_snapshot(self.document.doc.snapshot());
        doc_copy.warm_cache_from(&self.document.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    doc_copy.composite_preview_without(exclude_id).map(|img| {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let (data, w2, h2) = fit_scene_display(rgba.into_raw(), w, h);
                        (data, w2, h2)
                    })
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
    pub(crate) fn drag_layer_composite_task(&mut self, layer_id: Uuid) -> Option<Task<Message>> {
        if !self.needs_fallback() {
            return None;
        }
        if !self.rendering.drag_layer_job.try_start() {
            return None;
        }
        let task_id = self
            .rendering
            .background_tasks
            .start("Rendu du calque déplacé...");

        let mut doc_copy =
            photo_engine::Document::new(self.document.doc.width, self.document.doc.height);
        doc_copy.restore_snapshot(self.document.doc.snapshot());
        doc_copy.warm_cache_from(&self.document.doc);

        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut tmp = photo_engine::Document::new(doc_copy.width, doc_copy.height);
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

        let prefs = preferences::Preferences::load("photo");
        let resolver = preferences::KeybindingResolver::from_bindings(&prefs.keybindings.bindings);

        Self {
            document: DocumentState::default(),
            canvas: CanvasState::default(),
            tools: ToolState::default(),
            rendering: RenderingState::default(),
            workspace: WorkspaceState {
                panes,
                focus: Some(canvas_pane),
            },
            windows: WindowState {
                main_window: None,
                preferences_window_id: None,
                preferences_window: None,
                preferences: prefs,
                resolver,
            },
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
// États explicites des jobs de rendu asynchrones (chantier 8 / 11).
// ---------------------------------------------------------------------------

/// Composite de fond (blend inter-calques). Compteur monotone inclus pour
/// jeter un résultat calculé avec une génération antérieure (le document a
/// changé pendant que la tâche tournait).
#[derive(Default)]
pub enum FallbackJob {
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
    /// la génération attribuée, ou `None` si rien à faire.
    pub(crate) fn start_new_run(&mut self) -> Option<u64> {
        match self {
            Self::Idle => None,
            Self::Running { dirty: false, .. } => None,
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
    #[allow(dead_code)]
    pub(crate) fn in_flight(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// Le calcul affiché est-il périmé (édition pendant le vol) ?
    #[allow(dead_code)]
    pub(crate) fn needs_recompute(&self) -> bool {
        match self {
            Self::Idle => false,
            Self::Running { dirty, .. } => *dirty,
        }
    }

    /// Tâche terminée.
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
#[derive(Default)]
pub enum DragBgJob {
    #[default]
    Idle,
    Running(Uuid),
}

impl DragBgJob {
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

    pub(crate) fn is_running_for(&self, id: Uuid) -> bool {
        matches!(self, Self::Running(x) if *x == id)
    }
}

/// Composite du calque seul AVEC masque — surimpression pendant le drag.
#[derive(Default)]
pub enum DragLayerJob {
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

    #[allow(dead_code)]
    pub(crate) fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}
