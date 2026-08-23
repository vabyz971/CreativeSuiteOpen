use crate::layers::Layer;
use datatypes::{NodeId, ParamValue};
use iced::widget::{image as iced_image, pane_grid};
use iced::{Element, Length, Point, Rectangle, Size, Task, Vector};
use std::sync::Arc;

mod components;
mod layers;

/// Menus applicatifs affichés dans le shell (Fichier / Édition / Affichage).
fn app_menus(tools_visible: bool) -> Vec<ui::menu::Menu<Message>> {
    vec![
        ui::menu::Menu::new(
            "Fichier",
            vec![
                ui::menu::Item::Action {
                    label: "Nouveau".into(),
                    shortcut: "Ctrl+N".to_string(),
                    checked: false,
                    message: Message::NewProject,
                },
                ui::menu::Item::Action {
                    label: "Ouvrir...".into(),
                    shortcut: "Ctrl+O".to_string(),
                    checked: false,
                    message: Message::OpenProject,
                },
                ui::menu::Item::Action {
                    label: "Ouvrir un élément récent".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Enregistrer".into(),
                    shortcut: "Ctrl+S".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Enregistrer sous...".into(),
                    shortcut: "Ctrl+Maj+S".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Quitter".into(),
                    shortcut: "Ctrl+Q".to_string(),
                    checked: false,
                    message: Message::Quit,
                },
            ],
        ),
        ui::menu::Menu::new(
            "Édition",
            vec![
                ui::menu::Item::Action {
                    label: "Annuler".into(),
                    shortcut: "Ctrl+Z".to_string(),
                    checked: false,
                    message: Message::Undo,
                },
                ui::menu::Item::Action {
                    label: "Rétablir".into(),
                    shortcut: "Ctrl+Y".to_string(),
                    checked: false,
                    message: Message::Redo,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Couper".into(),
                    shortcut: "Ctrl+X".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Copier".into(),
                    shortcut: "Ctrl+C".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Action {
                    label: "Coller".into(),
                    shortcut: "Ctrl+V".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Préférences...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenOptions,
                },
            ],
        ),
        ui::menu::Menu::new(
            "Calque",
            vec![
                ui::menu::Item::Action {
                    label: "Nouveau calque vide".into(),
                    shortcut: "Ctrl+Maj+N".to_string(),
                    checked: false,
                    message: Message::AddEmptyLayer,
                },
                ui::menu::Item::Action {
                    label: "Calque depuis une image...".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::OpenImage,
                },
                ui::menu::Item::Action {
                    label: "Dupliquer le calque".into(),
                    shortcut: "Ctrl+J".to_string(),
                    checked: false,
                    message: Message::DuplicateLayer(0),
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Supprimer le calque".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::DeleteLayer(0),
                },
            ],
        ),
        ui::menu::Menu::new(
            "Affichage",
            vec![
                ui::menu::Item::Action {
                    label: "Propriétés".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Properties),
                },
                ui::menu::Item::Action {
                    label: "Calques".into(),
                    shortcut: "F7".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Layers),
                },
                ui::menu::Item::Action {
                    label: "Générateur de textures".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::TogglePanel(PanelType::Generator),
                },
                ui::menu::Item::Action {
                    label: "Barre d'outils".into(),
                    shortcut: "Tab".to_string(),
                    checked: tools_visible,
                    message: Message::ToggleToolsPanel,
                },
                ui::menu::Item::Separator,
                ui::menu::Item::Action {
                    label: "Réinitialiser l'interface".into(),
                    shortcut: "".to_string(),
                    checked: false,
                    message: Message::MockAction,
                },
            ],
        ),
    ]
}

pub fn main() -> iced::Result {
    // Force rayon à utiliser tous les cœurs
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(cores)
        .thread_name(|i| format!("rayon-photo-{}", i))
        .build_global();
    // Warmup GPU en arrière-plan pour que le canvas principal intègre wgpu dès le démarrage
    std::thread::spawn(|| {
        let _ = crate::components::gpu::GpuContext::get();
    });
    iced::application(PhotoApp::default, update, view)
        .title("Creative Suite Open Photo")
        .font(include_bytes!(
            "../../../assets/fonts/MaterialIcons-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Regular.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-SemiBold.ttf"
        ))
        .font(include_bytes!(
            "../../../assets/fonts/HankenGrotesk-Bold.ttf"
        ))
        .default_font(ui::theme::fonts::SANS)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Hand,
    Zoom,
    Select,
    Eyedropper,
    Move,
}

struct PhotoApp {
    pub zoom_level: u32,
    pub active_menu: Option<usize>,
    pub panes: pane_grid::State<PanelType>,
    pub focus: Option<pane_grid::Pane>,
    // ---- Pile de calques (index 0 = bas de la pile) ----
    pub layers: Vec<Layer>,
    pub next_layer_id: u64,
    pub selected_layer: Option<u64>,
    /// Dimensions du document (fixées par la première image ouverte) — pour export/titre
    pub doc_size: Option<(u32, u32)>,
    /// Taille du composite fallback (modes de fusion non-Normal)
    pub fallback_size: Option<Size>,
    /// Composite CPU unique — UNIQUEMENT si un calque visible a un mode
    /// de fusion non-Normal (sinon chemin rapide par calque, zéro recomposite)
    pub fallback_handle: Option<iced_image::Handle>,
    pub image_path: Option<String>,
    pub image_error: Option<String>,
    // Canvas interaction (outil Main + pan/zoom)
    pub selected_tool: Tool,
    pub canvas_pan: Vector,
    pub color_profile: String,
    pub canvas_selection: Option<Rectangle>,
    /// Taille du viewport du canvas (publiée par le widget image_canvas)
    pub canvas_viewport: Size,
    /// Barre d'outils flottante visible ou masquée
    pub tools_visible: bool,
    /// Ancre de déplacement du calque sélectionné (outil Déplacer)
    pub move_anchor: Option<(u64, f32, f32)>,
    /// Throttle du recomposite fallback pendant un drag (30 fps max)
    pub last_fallback_refresh: Option<std::time::Instant>,
    // ---- Générateur de textures (graphe nodal — futur usage filtres/génération) ----
    pub gen_graph: suite_core::Graph,
    pub gen_selected_node: Option<NodeId>,
    pub gen_previews: std::collections::HashMap<NodeId, iced_image::Handle>,
    pub node_context_menu: Option<Point>,
    pub node_context_world: Option<datatypes::Vec2>,
    pub pending_connect: Option<(NodeId, String, datatypes::SocketType, bool)>,
    // Options / Hardware
    pub show_options: bool,
    pub gpu_info: Option<String>,
    pub gpu_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelType {
    Canvas,
    Properties,
    Layers,
    Generator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetAxis {
    X,
    Y,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Keyboard Actions
    ZoomInPressed,
    ZoomOutPressed,
    MockAction,

    // Panel Actions
    TogglePanel(PanelType),
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    PaneClicked(pane_grid::Pane),
    ClosePane(pane_grid::Pane),

    // UI Toolbar
    ToggleMenu(Option<usize>),
    NewProject,
    OpenProject,
    Quit,
    Undo,
    Redo,
    OpenOptions,
    CloseOptions,

    // Mouse & Context Menu Actions
    /// Ajuste le zoom/pan pour voir toute l'image dans le viewport
    CanvasFit,
    /// Affiche/masque la barre d'outils flottante
    ToggleToolsPanel,
    CloseNodeContextMenu,

    // Outils
    SelectTool(Tool),
    ImageCanvasEvent(ui::image_canvas::ImageCanvasEvent),

    // Calques
    SelectLayer(u64),
    ToggleLayerVisible(u64),
    SetLayerOpacity {
        id: u64,
        opacity: f32,
    },
    SetLayerBlend {
        id: u64,
        mode: String,
    },
    RenameLayer {
        id: u64,
        name: String,
    },
    SetLayerOffset {
        id: u64,
        axis: OffsetAxis,
        value: f32,
    },
    AddEmptyLayer,
    DuplicateLayer(u64),
    DeleteLayer(u64),
    MoveLayerUp(u64),
    MoveLayerDown(u64),

    // Image - utilise le picker natif via rfd
    OpenImage,
    ImagePicked(Option<std::path::PathBuf>),
    ImageLoaded(Result<(Vec<u8>, String), String>),

    // Générateur de textures (graphe nodal)
    NodeGraphEvent(ui::node_graph::NodeGraphEvent),
    UpdateParam {
        node: NodeId,
        key: String,
        value: ParamValue,
    },
    AddNodeAt {
        type_id: String,
        world_pos: datatypes::Vec2,
    },
    DeleteSelectedNode,

    // Hardware
    DetectGpu,
    GpuDetected(String),
}

impl Default for PhotoApp {
    fn default() -> Self {
        // Layout : Canvas à gauche, à droite Propriétés (haut) + Calques (bas)
        let (mut panes, canvas_pane) = pane_grid::State::new(PanelType::Canvas);

        let (right_pane, _split_canvas_right) = panes
            .split(pane_grid::Axis::Vertical, canvas_pane, PanelType::Layers)
            .expect("Erreur lors de l'ajout du panneau Calques");

        let (_props_pane, _split_right_panel) = panes
            .split(
                pane_grid::Axis::Horizontal,
                right_pane,
                PanelType::Properties,
            )
            .expect("Erreur lors de l'ajout des Propriétés");

        panes.resize(_split_canvas_right, 0.74);
        panes.resize(_split_right_panel, 0.55);

        Self {
            active_menu: None,
            zoom_level: 100,
            panes,
            focus: Some(canvas_pane),
            layers: Vec::new(),
            next_layer_id: 0,
            selected_layer: None,
            doc_size: None,
            fallback_size: None,
            fallback_handle: None,
            image_path: None,
            image_error: None,
            selected_tool: Tool::Hand,
            canvas_pan: Vector::new(0.0, 0.0),
            color_profile: "sRGB IEC61966-2.1".into(),
            canvas_selection: None,
            canvas_viewport: Size::new(800.0, 600.0),
            tools_visible: true,
            move_anchor: None,
            last_fallback_refresh: None,
            gen_graph: components::node_registry::create_empty_graph(),
            gen_selected_node: None,
            gen_previews: Default::default(),
            node_context_menu: None,
            node_context_world: None,
            pending_connect: None,
            show_options: false,
            gpu_info: None,
            gpu_available: components::gpu::GpuContext::is_available(),
        }
    }
}

impl PhotoApp {
    fn alloc_layer_id(&mut self) -> u64 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        id
    }

    fn layer_index(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    fn selected_layer_mut(&mut self) -> Option<&mut Layer> {
        let sel = self.selected_layer?;
        self.layer_index(sel).map(|i| &mut self.layers[i])
    }

    /// Un calque visible a-t-il un mode de fusion non-Normal ?
    /// (l'opacité est gérée au draw, elle ne force plus le fallback)
    fn needs_fallback(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.visible && l.blend_mode != "Normal")
    }

    /// Composite CPU — UNIQUEMENT pour les modes de fusion non-Normal
    /// (le chemin rapide par calque couvre Normal/opacité sans recomposite).
    fn refresh_fallback(&mut self) {
        use photo_engine::document::{LayerData, composite_preview};
        if !self.needs_fallback() {
            self.fallback_handle = None;
            self.fallback_size = None;
            return;
        }
        let data: Vec<LayerData> = self.layers.iter().map(LayerData::from).collect();
        let (doc_w, doc_h) = self.doc_size.unwrap_or((800, 600));
        if let Some(img) = composite_preview(&data, doc_w, doc_h) {
            let (w, h) = (img.width() as f32, img.height() as f32);
            let rgba = img.to_rgba8();
            self.fallback_size = Some(Size::new(w, h));
            self.fallback_handle = Some(iced_image::Handle::from_rgba(
                rgba.width(),
                rgba.height(),
                rgba.into_raw(),
            ));
        } else {
            self.fallback_size = None;
            self.fallback_handle = None;
        }
    }
}

fn update(app: &mut PhotoApp, message: Message) -> Task<Message> {
    match message {
        Message::ToggleMenu(menu) => {
            if app.active_menu == menu {
                app.active_menu = None;
            } else {
                app.active_menu = menu;
            }
        }
        Message::NewProject => {
            app.layers.clear();
            app.selected_layer = None;
            app.next_layer_id = 0;
            app.doc_size = None;
            app.gen_graph = components::node_registry::create_empty_graph();
            app.canvas_pan = Vector::new(0.0, 0.0);
            app.zoom_level = 100;
            app.fallback_size = None;
            app.fallback_handle = None;
            app.image_path = None;
            app.image_error = None;
            app.move_anchor = None;
        }
        Message::OpenProject => {
            app.active_menu = None;
            return pick_image_task(Message::ImagePicked);
        }
        Message::OpenImage => {
            app.active_menu = None;
            return pick_image_task(Message::ImagePicked);
        }
        Message::ImagePicked(path_opt) => {
            if let Some(path) = path_opt {
                return Task::perform(
                    async move {
                        let bytes =
                            std::fs::read(&path).map_err(|e| format!("Lecture échouée: {e}"))?;
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image")
                            .to_string();
                        Ok::<(Vec<u8>, String), String>((bytes, name))
                    },
                    Message::ImageLoaded,
                );
            }
        }
        Message::ImageLoaded(Ok((bytes, name))) => {
            app.image_path = Some(name.clone());
            app.image_error = None;
            match ::image::load_from_memory(&bytes) {
                Ok(dyn_img) => {
                    // Le document prend les dimensions de la première image
                    if app.doc_size.is_none() {
                        app.doc_size = Some((dyn_img.width(), dyn_img.height()));
                        app.canvas_pan = Vector::new(0.0, 0.0);
                        app.canvas_selection = None;
                        app.zoom_level = 100;
                    }
                    let id = app.alloc_layer_id();
                    app.layers.push(Layer::new(id, name, Arc::new(dyn_img)));
                    app.selected_layer = Some(id);
                    app.refresh_fallback();
                }
                Err(e) => {
                    app.image_error = Some(format!("Décodage échoué: {e}"));
                }
            }
        }
        Message::ImageLoaded(Err(e)) => {
            app.image_error = Some(e);
        }

        // ---- Calques ----
        Message::SelectLayer(id) => {
            app.selected_layer = Some(id);
            app.move_anchor = None;
        }
        Message::ToggleLayerVisible(id) => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].visible = !app.layers[i].visible;
                app.refresh_fallback();
            }
        }
        Message::SetLayerOpacity { id, opacity } => {
            // Simple changement d'état : l'opacité est appliquée au draw
            // (GPU) — zéro régénération de pixels, zéro clignotement
            if let Some(i) = app.layer_index(id) {
                app.layers[i].opacity = opacity;
            }
        }
        Message::SetLayerBlend { id, mode } => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].blend_mode = mode;
                // Bascule chemin rapide ↔ fallback selon le mode
                if app.needs_fallback() {
                    app.refresh_fallback();
                } else {
                    app.fallback_handle = None;
                    app.fallback_size = None;
                }
            }
        }
        Message::RenameLayer { id, name } => {
            if let Some(i) = app.layer_index(id) {
                app.layers[i].name = name;
            }
        }
        Message::SetLayerOffset { id, axis, value } => {
            if let Some(i) = app.layer_index(id) {
                match axis {
                    OffsetAxis::X => app.layers[i].offset_x = value,
                    OffsetAxis::Y => app.layers[i].offset_y = value,
                }
                app.refresh_fallback();
            }
        }
        Message::AddEmptyLayer => {
            app.active_menu = None;
            let (w, h) = app.doc_size.unwrap_or((800, 600));
            let transparent = ::image::DynamicImage::ImageRgba8(::image::ImageBuffer::from_pixel(
                w.max(1),
                h.max(1),
                ::image::Rgba([0, 0, 0, 0]),
            ));
            let id = app.alloc_layer_id();
            let idx = app.selected_layer.and_then(|s| app.layer_index(s));
            let layer = Layer::new(id, format!("Calque {}", id + 1), Arc::new(transparent));
            // Insère AU-DESSUS du calque sélectionné (sinon tout en haut)
            match idx {
                Some(i) => app.layers.insert(i + 1, layer),
                None => app.layers.push(layer),
            }
            app.selected_layer = Some(id);
            app.refresh_fallback();
        }
        Message::DuplicateLayer(id) => {
            app.active_menu = None;
            let src = id;
            let src = if app.layer_index(src).is_some() {
                src
            } else {
                app.selected_layer.unwrap_or(src)
            };
            if let Some(i) = app.layer_index(src) {
                let mut copy = Layer::new(
                    app.alloc_layer_id(),
                    format!("{} copie", app.layers[i].name),
                    app.layers[i].image.clone(),
                );
                copy.opacity = app.layers[i].opacity;
                copy.blend_mode = app.layers[i].blend_mode.clone();
                copy.visible = app.layers[i].visible;
                copy.offset_x = app.layers[i].offset_x;
                copy.offset_y = app.layers[i].offset_y;
                app.layers.insert(i + 1, copy);
                app.selected_layer = Some(app.layers[i + 1].id);
                app.refresh_fallback();
            }
        }
        Message::DeleteLayer(id) => {
            app.active_menu = None;
            let target = if app.layer_index(id).is_some() {
                Some(id)
            } else {
                app.selected_layer
            };
            if let Some(t) = target
                && app.layers.len() > 1
                && let Some(i) = app.layer_index(t)
            {
                app.layers.remove(i);
                app.selected_layer = app
                    .layers
                    .get(i.min(app.layers.len() - 1))
                    .or_else(|| app.layers.last())
                    .map(|l| l.id);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerUp(id) => {
            if let Some(i) = app.layer_index(id)
                && i + 1 < app.layers.len()
            {
                app.layers.swap(i, i + 1);
                app.refresh_fallback();
            }
        }
        Message::MoveLayerDown(id) => {
            if let Some(i) = app.layer_index(id)
                && i > 0
            {
                app.layers.swap(i, i - 1);
                app.refresh_fallback();
            }
        }

        Message::SelectTool(tool) => {
            app.selected_tool = tool;
            app.canvas_selection = None;
            app.move_anchor = None;
        }
        Message::ToggleToolsPanel => {
            app.tools_visible = !app.tools_visible;
        }
        Message::CanvasFit => {
            // Zoom pour voir toute l'image, centrée (pan nul)
            if let Some((iw, ih)) = app.doc_size.map(|(w, h)| (w as f32, h as f32)) {
                let vw = app.canvas_viewport.width.max(1.0);
                let vh = app.canvas_viewport.height.max(1.0);
                let fit = (vw / iw).min(vh / ih) * 0.95; // 5% de marge
                let zoom = fit.clamp(0.08, 6.0);
                app.zoom_level = (zoom * 100.0).round() as u32;
                app.canvas_pan = Vector::new(0.0, 0.0);
                app.canvas_selection = None;
            }
        }
        Message::ImageCanvasEvent(evt) => match evt {
            ui::image_canvas::ImageCanvasEvent::Viewport(size) => {
                app.canvas_viewport = size;
            }
            ui::image_canvas::ImageCanvasEvent::Pan(pan) => {
                if app.selected_tool == Tool::Hand {
                    app.canvas_pan = pan;
                }
            }
            ui::image_canvas::ImageCanvasEvent::ZoomPan { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::ZoomAt { zoom, pan } => {
                app.zoom_level = (zoom * 100.0) as u32;
                app.canvas_pan = pan;
            }
            ui::image_canvas::ImageCanvasEvent::SelectRect(rect) => {
                if app.selected_tool == Tool::Select || app.selected_tool == Tool::Zoom {
                    if app.selected_tool == Tool::Zoom {
                        // Zoom sur la zone sélectionnée
                        if let Some(r) = rect
                            && r.width > 10.0
                            && r.height > 10.0
                        {
                            let sx = 800.0 / r.width;
                            let sy = 600.0 / r.height;
                            let new_zoom =
                                (sx.min(sy) * app.zoom_level as f32 / 100.0).clamp(0.08, 6.0);
                            app.zoom_level = (new_zoom * 100.0) as u32;
                            let cx = r.x + r.width / 2.0 - 400.0;
                            let cy = r.y + r.height / 2.0 - 300.0;
                            app.canvas_pan = Vector::new(-cx, -cy);
                        }
                    } else {
                        app.canvas_selection = rect;
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerStart => {
                if app.selected_tool == Tool::Move
                    && let Some(layer) = app.selected_layer_mut()
                {
                    app.move_anchor = Some((layer.id, layer.offset_x, layer.offset_y));
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayer { dx, dy } => {
                if app.selected_tool == Tool::Move
                    && let Some((id, ax, ay)) = app.move_anchor
                    && Some(id) == app.selected_layer
                {
                    let zoom = app.zoom_level as f32 / 100.0;
                    if zoom > 0.001 {
                        // ZÉRO recomposite : l'offset change et le canvas
                        // redessine la texture à sa nouvelle position à la
                        // frame suivante (modèle Affinity — fluide à 60 fps)
                        let new_x = ax + dx / zoom;
                        let new_y = ay + dy / zoom;
                        if let Some(i) = app.layer_index(id) {
                            app.layers[i].offset_x = new_x;
                            app.layers[i].offset_y = new_y;
                        }
                        // Fallback fusion non-Normal seulement : recomposite
                        // throttlé (le chemin rapide n'en a pas besoin)
                        if app.needs_fallback() {
                            let now = std::time::Instant::now();
                            let should = app
                                .last_fallback_refresh
                                .map(|t| now.duration_since(t).as_millis() > 33)
                                .unwrap_or(true);
                            if should {
                                app.last_fallback_refresh = Some(now);
                                app.refresh_fallback();
                            }
                        }
                    }
                }
            }
            ui::image_canvas::ImageCanvasEvent::MoveLayerEnd => {
                app.move_anchor = None;
                app.last_fallback_refresh = None;
                if app.needs_fallback() {
                    app.refresh_fallback();
                }
            }
        },
        Message::Quit => {
            std::process::exit(0);
        }
        Message::Undo => {}
        Message::Redo => {}
        Message::ZoomInPressed => {
            app.zoom_level = (app.zoom_level + 10).clamp(5, 1600);
        }
        Message::ZoomOutPressed => {
            app.zoom_level = app.zoom_level.saturating_sub(10).max(5);
        }
        Message::MockAction => {
            // Keep silent to avoid spam from subscription
        }

        Message::TogglePanel(panel_type) => {
            let existing_pane = app
                .panes
                .iter()
                .find(|(_, p)| **p == panel_type)
                .map(|(pane, _)| *pane);

            if let Some(pane) = existing_pane {
                app.panes.close(pane);
            } else {
                let target_canvas_pane = app
                    .panes
                    .iter()
                    .find(|(_, p)| **p == PanelType::Canvas)
                    .map(|(p, _)| *p);

                if let Some(canvas_pane) = target_canvas_pane {
                    let axis = match panel_type {
                        PanelType::Generator => pane_grid::Axis::Horizontal,
                        _ => pane_grid::Axis::Vertical,
                    };
                    app.panes.split(axis, canvas_pane, panel_type);
                }
            }
        }
        Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
            app.panes.resize(split, ratio);
        }
        Message::PaneDragged(pane_grid::DragEvent::Dropped { pane, target }) => {
            app.panes.drop(pane, target);
        }
        Message::PaneDragged(_) => {}
        Message::PaneClicked(pane) => {
            app.focus = Some(pane);
        }
        Message::ClosePane(pane) => {
            app.panes.close(pane);
        }
        Message::CloseNodeContextMenu => {
            app.node_context_menu = None;
        }
        // ---- Générateur de textures (graphe nodal, usage futur filtres/génération) ----
        Message::NodeGraphEvent(evt) => match evt {
            ui::node_graph::NodeGraphEvent::NodeSelected(id) => {
                app.gen_selected_node = Some(id);
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::NodeMoved { id, position } => {
                app.gen_graph.move_node(id, position);
            }
            ui::node_graph::NodeGraphEvent::BackgroundClicked => {
                app.gen_selected_node = None;
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::RequestContextMenu(world, local) => {
                app.pending_connect = None;
                app.node_context_menu = Some(local);
                app.node_context_world = Some(world);
            }
            ui::node_graph::NodeGraphEvent::Connect {
                from,
                from_socket,
                to,
                to_socket,
            } => {
                let existing = app
                    .gen_graph
                    .connections
                    .iter()
                    .find(|c| c.to_node == to && c.to_socket == to_socket)
                    .cloned();
                if let Some(conn) = existing {
                    app.gen_graph.disconnect(&conn);
                }
                let from_ty = if from_socket == "factor" || to_socket == "factor" {
                    datatypes::SocketType::Float
                } else {
                    datatypes::SocketType::Image
                };
                let _ = app.gen_graph.connect(suite_core::Connection::new(
                    from,
                    from_socket.clone(),
                    to,
                    to_socket.clone(),
                    from_ty,
                ));
                app.node_context_menu = None;
            }
            ui::node_graph::NodeGraphEvent::Disconnect { node, socket } => {
                app.gen_graph.disconnect_input(node, &socket);
            }
            // Événements non utilisés par le générateur (ignorés silencieusement)
            _ => {}
        },
        Message::UpdateParam { node, key, value } => {
            app.gen_graph.update_param(node, key.clone(), value.clone());
        }
        Message::AddNodeAt { type_id, world_pos } => {
            let pos = datatypes::Vec2::new(
                world_pos.x.max(-2000.0).min(3000.0),
                world_pos.y.max(-2000.0).min(3000.0),
            );
            if let Some(id) =
                components::node_registry::create_node_for_type(&type_id, pos, &mut app.gen_graph)
            {
                app.gen_selected_node = Some(id);
            }
            app.node_context_menu = None;
            app.node_context_world = None;
        }
        Message::DeleteSelectedNode => {
            if let Some(id) = app.gen_selected_node {
                app.gen_graph.remove_node(id);
                app.gen_selected_node = None;
            }
        }

        Message::OpenOptions => {
            app.show_options = true;
            app.active_menu = None;
            // Détection matériel async
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::CloseOptions => {
            app.show_options = false;
        }
        Message::DetectGpu => {
            return Task::perform(
                async { components::gpu::detect_gpu_info().await },
                Message::GpuDetected,
            );
        }
        Message::GpuDetected(info) => {
            app.gpu_info = Some(info);
            app.gpu_available = true;
        }
    }
    Task::none()
}

fn pick_image_task(map: fn(Option<std::path::PathBuf>) -> Message) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter(
                    "Images",
                    &["png", "jpg", "jpeg", "bmp", "tiff", "webp", "gif"],
                )
                .set_title("Ouvrir une image")
                .pick_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        map,
    )
}

fn view(app: &PhotoApp) -> Element<'_, Message> {
    let doc_size = app.doc_size.map(|(w, h)| Size::new(w as f32, h as f32));
    // Contenu central : barre contextuelle (projet/zoom/export) + workspace
    let menus = app_menus(app.tools_visible);
    let menu_buttons = ui::menu::bar(&menus, app.active_menu, Message::ToggleMenu);

    let central = iced::widget::column![
        components::toolbar::context_bar(app.image_path.as_deref()),
        components::workspace::render(
            &app.panes,
            app.focus,
            &app.layers,
            app.selected_layer,
            doc_size,
            app.fallback_handle.clone(),
            app.fallback_size,
            app.image_path.clone(),
            app.image_error.clone(),
            app.selected_tool,
            app.tools_visible,
            app.canvas_pan,
            app.zoom_level,
            app.canvas_selection,
            app.color_profile.clone(),
            app.canvas_viewport,
            &app.gen_graph,
            app.gen_selected_node,
            &app.gen_previews,
            app.node_context_menu,
            app.node_context_world,
        )
    ];
    // Shell : menus intégrés à la top bar — outils Photo en flottant sur le canvas
    let base_layout =
        ui::shell::minimalist_layout_menus_only("Creative Suite Open Photo", menu_buttons, central);

    // Overlay Options (comme GIMP → Préférences)
    if app.show_options {
        let options_overlay = iced::widget::stack![
            // Fond semi-transparent
            iced::widget::container(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .style(|_| iced::widget::container::Style {
                background: Some(ui::theme::colors::CABLE_SHADOW.into()),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill),
            // Clic hors panel ferme
            iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fill)
            )
            .on_press(Message::CloseOptions),
            // Panel centré
            iced::widget::container(components::options::view(
                app.gpu_info.clone(),
                app.gpu_available,
                app.zoom_level
            ))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24),
        ];

        return iced::widget::stack![base_layout, options_overlay].into();
    }

    // Les dropdowns des menus sont gérés nativement par iced_aw::DropDown
    base_layout
}
