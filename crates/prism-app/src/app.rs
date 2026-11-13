//! The window and event loop. Kept thin: anything testable lives in other modules.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::chrome::Chrome;
use crate::frame_stats::FrameStats;
use crate::gpu::Gpu;
use crate::input_log::{InputEvent, InputLog, Modifiers, WheelUnit};
use crate::overlay::{self, DebugInfo, Demo};
use crate::settings::Settings;
use crate::viewport::Viewport;

/// How many input events the overlay keeps.
const LOG_CAPACITY: usize = 14;

/// Start the app. Natively this blocks until the window closes. On the web it
/// returns once the event loop is handed to the browser.
pub fn run() {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("could not create the event loop");
    // One running Prism (brief 11): a later start hands its files to the
    // first and quits; macOS sends double-clicked files as Apple events.
    if crate::instance::claim(&start_files()) {
        return;
    }
    // What an earlier update left behind (brief 20).
    #[cfg(not(target_arch = "wasm32"))]
    crate::update::clean_up();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let proxy = event_loop.create_proxy();
        crate::instance::set_wake(move || {
            let _ = proxy.send_event(UserEvent::Files);
        });
    }
    let app = App::new(&event_loop);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop failed");
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}

/// Files named on the command line, as full paths (`prism model.prism`,
/// how Windows passes a double-clicked file). macOS adds a -psn_ argument
/// when Finder starts an app.
fn start_files() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with("-psn"))
            .map(|a| {
                std::fs::canonicalize(&a)
                    .map(|p| p.display().to_string())
                    .unwrap_or(a)
            })
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}

/// What can run now, from the document and the selection.
pub(crate) fn rules(viewport: &Viewport, recent: usize) -> crate::actions::Context {
    crate::actions::Context {
        can_undo: viewport.can_undo(),
        can_redo: viewport.can_redo(),
        has_selection: !viewport.counts().is_empty(),
        has_groups: viewport.counts().groups > 0,
        recent,
    }
}

/// Send the actions that ran (menus, keys, the assistant) where they go.
pub(crate) fn dispatch(
    chrome: &mut Chrome,
    viewport: &mut Viewport,
    files: &mut crate::files::Files,
    tab_actions: &mut Vec<isize>,
) {
    for e in std::mem::take(&mut chrome.events) {
        if e.id == "win.nexttab" || e.id == "win.prevtab" {
            tab_actions.push(if e.id == "win.nexttab" { 1 } else { -1 });
        } else if e.id.starts_with("file.") || e.id.starts_with("exp.") {
            files.action(e.id, &mut viewport.session, chrome);
        } else {
            viewport.action(e.id, chrome);
        }
    }
}

/// What other threads and the browser tell the event loop.
pub enum UserEvent {
    /// The GPU is ready (the web sets it up in the background).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Gpu(Box<Gpu>),
    /// Files arrived from another start or from the system (native only).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Files,
    /// The MCP server has a tool call waiting (brief 21).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Mcp,
}

struct App {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    log: InputLog,
    stats: FrameStats,
    start: web_time::Instant,
    cursor: Option<(f64, f64)>,
    modifiers: Modifiers,
    settings: Settings,
    demo: Demo,
    aurora: prism_ui::Aurora,
    /// What the system says about reducing motion, read at start and on focus.
    system_reduce_motion: Option<bool>,
    /// When egui next wants a frame, if it asked for one later.
    next_redraw: Option<web_time::Instant>,
    /// Menus, dock, search and the shortcut and feature windows.
    chrome: Chrome,
    /// The modifier keys held now, as the window reports them.
    modifiers_state: winit::keyboard::ModifiersState,
    /// The model, the camera, hover and selection.
    viewport: Viewport,
    /// The tool picked last frame, to notice changes.
    last_tool: &'static str,
    /// The model version the GPU has.
    uploaded: Option<u64>,
    /// The document's file: save, open, recovery (brief 08).
    files: crate::files::Files,
    /// The other open documents (brief 11).
    tabs: crate::tabs::Tabs,
    /// Next and previous tab, asked for this frame.
    tab_actions: Vec<isize>,
    /// A file named on the command line, opened on the first frame.
    start_file: Option<String>,
    /// The model version Model Health last checked.
    health_version: Option<u64>,
    /// Screenshots the assistant asked for, taken after the next frame.
    shots: Vec<crate::mcp::Call>,
}

impl App {
    fn new(event_loop: &EventLoop<UserEvent>) -> Self {
        let egui_ctx = egui::Context::default();
        prism_ui::theme::install(&egui_ctx);
        let mut viewport = Viewport::new();
        let mut files = crate::files::Files::new(&viewport.session, 1);
        let mut tabs = crate::tabs::Tabs::new(1);
        // Work a crash left behind comes back, one tab each (natively it is
        // read now; on the web it arrives in later frames).
        tabs.offer(crate::recovery::left_behind(), &mut viewport, &mut files);
        // `prism model.prism` (how Windows passes a double-clicked file).
        // macOS adds a -psn_ argument when Finder starts an app.
        let start_file = start_files().into_iter().next();
        let mut chrome = Chrome::new(crate::platform::keys());
        // The MCP server, if the user turned it on (brief 21).
        let proxy = event_loop.create_proxy();
        chrome.mcp.set_wake(move || {
            let _ = proxy.send_event(UserEvent::Mcp);
        });
        chrome.mcp.apply();
        Self {
            proxy: event_loop.create_proxy(),
            window: None,
            gpu: None,
            egui_ctx,
            egui_state: None,
            log: InputLog::new(LOG_CAPACITY),
            stats: FrameStats::new(),
            start: web_time::Instant::now(),
            cursor: None,
            modifiers: Modifiers::default(),
            settings: Settings::default(),
            demo: Demo::default(),
            aurora: prism_ui::Aurora::new(),
            system_reduce_motion: crate::platform::system_reduces_motion(),
            next_redraw: None,
            chrome,
            modifiers_state: winit::keyboard::ModifiersState::empty(),
            viewport,
            last_tool: "tool.select",
            uploaded: None,
            files,
            tabs,
            tab_actions: Vec::new(),
            start_file,
            health_version: None,
            shots: Vec::new(),
        }
    }

    /// Run the assistant's waiting tool calls (brief 21).
    fn serve_mcp(&mut self) {
        for call in self.chrome.mcp.take_calls() {
            if call.name == "screenshot" {
                self.chrome.mcp.note("screenshot".into());
                self.shots.push(call);
                continue;
            }
            let mut host = crate::mcp::host::AppHost {
                chrome: &mut self.chrome,
                viewport: &mut self.viewport,
                files: &mut self.files,
                tab_actions: &mut self.tab_actions,
                last_tool: &mut self.last_tool,
            };
            let result = crate::mcp::protocol::Host::call(&mut host, &call.name, &call.args);
            let what = call
                .args
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map_or(call.name.clone(), |id| format!("{} {id}", call.name));
            self.chrome.mcp.note(format!(
                "{what}{}",
                if result.is_ok() { "" } else { " (refused)" }
            ));
            let _ = call.reply.send(result);
        }
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Turn a winit event into a loggable one, if it is input we care about.
    fn input_event(&mut self, event: &WindowEvent) -> Option<InputEvent> {
        match event {
            WindowEvent::KeyboardInput { event, .. } => Some(InputEvent::Key {
                code: match event.physical_key {
                    PhysicalKey::Code(code) => format!("{code:?}"),
                    PhysicalKey::Unidentified(native) => format!("{native:?}"),
                },
                text: event.text.as_ref().map(|t| t.to_string()),
                pressed: event.state == ElementState::Pressed,
                repeat: event.repeat,
                modifiers: self.modifiers,
            }),
            WindowEvent::MouseInput { state, button, .. } => {
                let (x, y) = self.cursor.unwrap_or_default();
                Some(InputEvent::Button {
                    button: button_name(*button),
                    pressed: *state == ElementState::Pressed,
                    x,
                    y,
                    modifiers: self.modifiers,
                })
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy, unit) = match *delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (f64::from(x), f64::from(y), WheelUnit::Lines)
                    }
                    MouseScrollDelta::PixelDelta(p) => (p.x, p.y, WheelUnit::Pixels),
                };
                Some(InputEvent::Wheel {
                    dx,
                    dy,
                    unit,
                    modifiers: self.modifiers,
                })
            }
            WindowEvent::PinchGesture { delta, .. } => Some(InputEvent::Pinch { delta: *delta }),
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.modifiers_state = s;
                self.modifiers = Modifiers {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    logo: s.super_key(),
                };
                Some(InputEvent::ModifiersChanged(self.modifiers))
            }
            _ => None,
        }
    }

    /// Tab requests, clicks and closes after a frame. Returns whether the
    /// active document changed (then the GPU gets the new one).
    fn tab_work(&mut self) -> bool {
        let (vp, f, chrome) = (&mut self.viewport, &mut self.files, &mut self.chrome);
        self.tabs.new_units = chrome.prefs.units;
        let mut changed = self.tabs.handle(vp, f, chrome);
        match chrome.doc_event.take() {
            Some(crate::chrome::DocEvent::Switch(i)) => changed |= self.tabs.switch(i, vp, f),
            Some(crate::chrome::DocEvent::Close(i)) => changed |= self.tabs.close(i, vp, f, chrome),
            Some(crate::chrome::DocEvent::New) => {
                f.requests.push(crate::files::Request::NewTab);
                changed |= self.tabs.handle(vp, f, chrome);
            }
            None => {}
        }
        for e in std::mem::take(&mut self.tab_actions) {
            changed |= self.tabs.step(e, vp, f);
        }
        changed |= self.tabs.offer(crate::recovery::take_found(), vp, f);
        changed |= self.tabs.after_frame(vp, f, chrome);
        if changed {
            // The GPU uploads the new tab's scene; the dock shows its tool.
            self.uploaded = None;
            if let Some(id) = chrome.tool_id(vp.session.tool()) {
                chrome.active_tool = id;
                self.last_tool = id;
            }
            vp.session.mark_all_dirty();
        }
        changed
    }

    fn redraw(&mut self) {
        let (Some(window), Some(gpu), Some(egui_state)) =
            (&self.window, &mut self.gpu, &mut self.egui_state)
        else {
            return;
        };

        self.stats.frame(self.start.elapsed().as_secs_f64());

        let raw_input = egui_state.take_egui_input(window);
        let size = window.inner_size();
        let motion = self.settings.motion(self.system_reduce_motion);
        let info = DebugInfo {
            stats: &self.stats,
            log: &self.log,
            cursor: self.cursor,
            modifiers: self.modifiers,
            size: [size.width, size.height],
            scale: window.scale_factor(),
            adapter: gpu.adapter_summary(),
            reduce_motion: motion.reduce_motion,
            aurora_redraws: self.aurora.redraws(),
        };
        let tabs_labels = self.tabs.labels(&self.viewport, &self.files);
        let active_tab = self.tabs.active();
        let (settings, demo, aurora, chrome, viewport, last_tool, files) = (
            &mut self.settings,
            &mut self.demo,
            &mut self.aurora,
            &mut self.chrome,
            &mut self.viewport,
            &mut self.last_tool,
            &mut self.files,
        );
        let start_file = self.start_file.take();
        let mut tab_actions: Vec<isize> = Vec::new();
        let mut health_version = self.health_version;
        chrome.viewport_texture = gpu.viewport_texture();
        viewport.session.reduce_motion = motion.reduce_motion;
        let mut viewport_px = [1u32, 1u32];
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx();
            prism_ui::set_motion(ctx, motion);
            aurora.prepare(ctx);
            // What can run depends on the document and the selection.
            if let Some(p) = &start_file {
                files.open_at_start(p, &mut viewport.session, chrome);
            }
            chrome.docs = tabs_labels.clone();
            chrome.active_doc = active_tab;
            chrome.recent = files.recent_names();
            chrome.rules = rules(viewport, chrome.recent.len());
            chrome.component_rows = viewport
                .session
                .component_rows()
                .into_iter()
                .map(|(def, name, uses)| crate::chrome::ComponentRow {
                    def: def.0,
                    name,
                    uses,
                })
                .collect();
            for op in std::mem::take(&mut chrome.component_ops) {
                match op {
                    crate::chrome::ComponentOp::Select(d) => {
                        viewport.session.select_component(prism_doc::DefId(d));
                    }
                    crate::chrome::ComponentOp::Rename(d, name) => {
                        if !viewport
                            .session
                            .rename_component(prism_doc::DefId(d), &name)
                        {
                            chrome.status_note("That name is empty or taken".into());
                        }
                    }
                }
                viewport.session.mark_all_dirty();
            }
            viewport.session.set_health_view(chrome.health_marks());
            // The View menu's check marks follow the view style.
            for id in prism_view::view_style::TOGGLES {
                let on = viewport.session.view_style().is_on(id) == Some(true);
                chrome.set_on(id, on);
            }
            if chrome.panels_open() {
                let s = &viewport.session;
                // The checks take a while on big models: rerun them only
                // when the model (or the open group) changed.
                let health = if !chrome.health_open() {
                    Vec::new()
                } else if health_version == Some(s.version()) {
                    std::mem::take(&mut chrome.panel_data.health)
                } else {
                    health_version = Some(s.version());
                    s.health_rows()
                };
                if !chrome.health_open() {
                    health_version = None;
                }
                chrome.panel_data = crate::chrome::PanelData {
                    outline: s.outline(),
                    info: Some(s.entity_info()),
                    tags: s.tag_rows(),
                    health,
                    view: s.view_style().clone(),
                    materials: s.material_rows(),
                    brush: s.brush(),
                    default_colour: s.style.face_front.0,
                    round: s.round_panel(),
                };
            }
            for op in std::mem::take(&mut chrome.panel_ops) {
                let s = &mut viewport.session;
                match op {
                    crate::chrome::PanelOp::Select(path) => s.select_path(&path),
                    crate::chrome::PanelOp::Open(path) => s.open_path_to(&path),
                    crate::chrome::PanelOp::Hide(id, hidden) => s.set_hidden(id, hidden),
                    crate::chrome::PanelOp::Name(id, name) => s.name_instance(id, &name),
                    crate::chrome::PanelOp::Tag(t) => s.tag_command(t),
                    crate::chrome::PanelOp::HealthSelect(k) => s.select_health(k),
                    crate::chrome::PanelOp::HealthFix(k) => s.health_fix(k),
                    crate::chrome::PanelOp::View(v) => s.set_view_style(v),
                    crate::chrome::PanelOp::Round(op) => s.round_input(op),
                    crate::chrome::PanelOp::RoundSize(text) => s.round_text(&text),
                    crate::chrome::PanelOp::Material(op) => {
                        // A swatch picks the Paint Bucket with it.
                        if let Some(tool) = s.material_op(op) {
                            chrome.active_tool = tool;
                        }
                        if let Some(why) = s.take_material_refusal() {
                            chrome.status_note(why.to_owned());
                        }
                    }
                    crate::chrome::PanelOp::LoadImage(target) => {
                        match crate::mesh_files::pick_image() {
                            Ok(Some(o)) => crate::chrome::image_loaded(chrome, s, target, o),
                            // On the web the image comes later.
                            Ok(None) => chrome.panels.image_for = Some(target),
                            Err(e) => chrome.status_note(format!("Could not load the image: {e}")),
                        }
                    }
                }
                s.mark_all_dirty();
            }
            // An image picked on the web arrives later.
            if let Some(r) = crate::mesh_files::take_picked_image() {
                let target = chrome.panels.image_for.take().flatten();
                match r {
                    Ok(o) => crate::chrome::image_loaded(chrome, &mut viewport.session, target, o),
                    Err(e) => chrome.status_note(format!("Could not load the image: {e}")),
                }
            }
            // Preferences reach every tab: the active one each frame.
            viewport.session.settings = chrome.prefs.inference;
            chrome.tolerance = crate::prefs::tolerance_text(viewport.session.scene().tol.dist);
            let status = viewport.status();
            chrome.set_hint(status.hint);
            chrome.measure = status.measure;
            chrome.lock_chip = status.lock.map(|(t, c)| (t, prism_ui::theme::srgba(c)));
            // Labels and the select box, from last frame's feedback.
            let o = viewport.overlay();
            chrome.viewport_labels = o
                .labels
                .iter()
                .filter_map(|l| {
                    let p = viewport.to_screen(l.at)? + egui::vec2(l.offset[0], l.offset[1]);
                    Some((p, l.text.clone(), prism_ui::theme::srgba(l.color.0)))
                })
                .collect();
            chrome.viewport_box = o.screen_box.map(|b| {
                (
                    egui::Rect::from_min_max(
                        viewport.local_to_screen(b.min),
                        viewport.local_to_screen(b.max),
                    ),
                    b.crossing,
                    o.box_label.clone().unwrap_or_default(),
                )
            });
            let layout = chrome.show(ctx);
            if chrome.active_tool != *last_tool {
                viewport.tool_changed(chrome.active_tool);
                *last_tool = chrome.active_tool;
            }
            dispatch(chrome, viewport, files, &mut tab_actions);
            if let Some(resp) = &layout.response {
                viewport.input(ctx, resp, layout.viewport, chrome);
            }
            *last_tool = chrome.active_tool;
            let now = ctx.input(|i| i.time);
            files.frame(ctx, now, &mut viewport.session, chrome);
            if let Some(after) = viewport.time(now) {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(after.min(1.0)));
            }
            let ppp = ctx.pixels_per_point();
            viewport_px = [
                (layout.viewport.width() * ppp).round().max(1.0) as u32,
                (layout.viewport.height() * ppp).round().max(1.0) as u32,
            ];
            let shown = overlay::Shown {
                debug: chrome.is_on("help.debug"),
                demo: chrome.is_on("help.themedemo"),
            };
            overlay::draw(ctx, layout.viewport, shown, &info, settings, demo, viewport);
        });
        egui_state.handle_platform_output(window, output.platform_output);
        self.tab_actions = tab_actions;
        self.health_version = health_version;
        if let Some(t) = self.files.new_title(&self.viewport.session) {
            window.set_title(&t);
            // winit does not set the page's title on the web.
            #[cfg(target_arch = "wasm32")]
            if let Some(d) = web_sys::window().and_then(|w| w.document()) {
                d.set_title(&t);
            }
        }
        let jobs = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        let s = &mut self.viewport.session;
        let mut redraw = s.take_dirty() || !self.shots.is_empty();
        let highlight_changed = s.take_highlight_dirty();
        let overlay_changed = s.take_overlay_dirty();
        redraw |= overlay_changed || highlight_changed;
        let upload = self.uploaded != Some(s.version());
        self.uploaded = Some(s.version());
        let overlay = overlay_changed.then(|| s.overlay());
        let highlight = prism_render::gpu::Highlight {
            hover: s.hover(),
            selection: s.selection().clone(),
            hover_more: s.hover_more(),
            paint_preview: s.paint_preview(),
        };
        let (scene, style) = (s.display_scene(), &s.style);
        let shadow = style.ground_shadow;
        let draw = crate::gpu::ViewportDraw {
            size: viewport_px,
            upload: upload.then_some(scene),
            highlight: (highlight_changed || upload).then_some((scene, highlight)),
            overlay: overlay.as_ref(),
            redraw,
            frame: prism_render::gpu::Frame {
                camera: s.camera(),
                lens: s.lens(),
                ppp: output.pixels_per_point,
                style,
                shadow,
                bounds: scene.bounds,
            },
        };
        gpu.render(
            &jobs,
            output.textures_delta,
            output.pixels_per_point,
            Some(draw),
        );
        // The assistant's screenshots show this frame (brief 21).
        if !self.shots.is_empty() {
            #[cfg(not(target_arch = "wasm32"))]
            let picture = gpu.read_viewport();
            #[cfg(target_arch = "wasm32")]
            let picture: Option<crate::mcp::image::Rgba> = None;
            let result = picture
                .map(|p| {
                    let small = p.shrink(crate::mcp::SHOT_WIDTH);
                    crate::mcp::protocol::Output::Image {
                        png: small.png(),
                        text: format!(
                            "The viewport, {} by {} pixels (drawn at {} by {}).",
                            small.w, small.h, p.w, p.h
                        ),
                    }
                })
                .ok_or_else(|| "The viewport has not been drawn yet.".to_owned());
            for call in std::mem::take(&mut self.shots) {
                let _ = call.reply.send(result.clone());
            }
        }

        // Draw again only when something will change: egui says when.
        let delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(std::time::Duration::MAX, |v| v.repaint_delay);
        self.next_redraw = None;
        if log::log_enabled!(log::Level::Trace) {
            log::trace!("repaint in {delay:?}: {:?}", self.egui_ctx.repaint_causes());
        }
        if self.settings.continuous_redraw || delay.is_zero() {
            window.request_redraw();
        } else if let Some(at) = web_time::Instant::now().checked_add(delay) {
            self.next_redraw = Some(at);
        }
    }
}

fn button_name(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "Left".into(),
        MouseButton::Right => "Right".into(),
        MouseButton::Middle => "Middle".into(),
        MouseButton::Back => "Back".into(),
        MouseButton::Forward => "Forward".into(),
        MouseButton::Other(n) => format!("Other({n})"),
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        #[allow(unused_mut)]
        let mut attributes = Window::default_attributes().with_title(crate::files::app_name());
        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            // The page's CSS sizes the canvas to fill the window.
            attributes = attributes.with_append(true);
        }

        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("could not create the window"),
        );

        self.egui_state = Some(egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        ));
        self.window = Some(window.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            let gpu = pollster::block_on(Gpu::new(window.clone()));
            self.gpu = Some(gpu);
            window.request_redraw();
        }
        #[cfg(target_arch = "wasm32")]
        {
            // The browser cannot block, so finish GPU setup asynchronously and
            // hand the result back to the event loop.
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let gpu = Gpu::new(window).await;
                if proxy.send_event(UserEvent::Gpu(Box::new(gpu))).is_err() {
                    log::error!("event loop closed before the GPU was ready");
                }
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        let gpu = match event {
            UserEvent::Gpu(gpu) => *gpu,
            UserEvent::Mcp => {
                self.serve_mcp();
                return;
            }
            UserEvent::Files => {
                // Each opens in a tab (or switches to it); the window comes
                // forward.
                for p in crate::instance::take() {
                    if p.is_empty() {
                        continue;
                    }
                    match crate::doc_files::open_path(&p) {
                        Ok(o) => {
                            self.tabs.open(
                                o,
                                &mut self.viewport,
                                &mut self.files,
                                &mut self.chrome,
                            );
                        }
                        Err(e) => self.chrome.status_note(e),
                    }
                }
                self.uploaded = None;
                self.viewport.session.mark_all_dirty();
                if let Some(w) = &self.window {
                    w.focus_window();
                    w.request_redraw();
                }
                return;
            }
        };
        self.gpu = Some(gpu);
        if let Some(window) = &self.window {
            if let Some(gpu) = &mut self.gpu {
                gpu.resize(window.inner_size());
            }
            window.request_redraw();
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::CursorMoved { position, .. } = &event {
            self.cursor = Some((position.x, position.y));
        }
        if matches!(event, WindowEvent::CursorLeft { .. }) {
            self.cursor = None;
        }
        let mut redraw = false;
        if let Some(input) = self.input_event(&event) {
            self.log.push(input);
            redraw = true;
        }
        // Typing for the measurement box comes first (so "m" after "2.5" is
        // a unit, not the Move tool); then shortcuts, Esc and key recording
        // go to the chrome; then what is left goes to the viewport.
        let mut used = false;
        if let WindowEvent::KeyboardInput { event: key, .. } = &event
            && let PhysicalKey::Code(code) = key.physical_key
        {
            let code_name = format!("{code:?}");
            let typing_elsewhere = self.egui_ctx.egui_wants_keyboard_input();
            let pressed = key.state == ElementState::Pressed;
            // Any other key makes a held Ctrl or Alt part of a shortcut, not
            // a tap, even when the menus use the key (Ctrl+Z).
            if pressed && !code_name.starts_with("Control") && !code_name.starts_with("Alt") {
                self.viewport.cancel_tap();
            }
            let chrome_ui = self.chrome.ui_open_pub() || self.files.modal();
            if pressed
                && !typing_elsewhere
                && !chrome_ui
                && !self.modifiers_state.control_key()
                && !self.modifiers_state.super_key()
                && let Some(t) = key.text.as_ref()
                && !t.chars().any(char::is_control)
            {
                used = self.viewport.text(t);
            }
            if !used && pressed && !key.repeat {
                let m = self.modifiers_state;
                let press = crate::actions::keymap::KeyPress {
                    code: code_name.clone(),
                    ctrl: m.control_key(),
                    cmd: m.super_key(),
                    shift: m.shift_key(),
                    alt: m.alt_key(),
                };
                used = self.chrome.key(&press, typing_elsewhere);
            }
            if !used
                && !typing_elsewhere
                && !chrome_ui
                && (!key.repeat || code_name.starts_with("Backspace"))
            {
                used = self.viewport.key(&code_name, pressed, &mut self.chrome);
            }
            redraw |= used;
        }
        // egui asks for a repaint after every redraw event, which would loop.
        if !used
            && !matches!(event, WindowEvent::RedrawRequested)
            && let (Some(window), Some(state)) = (&self.window, &mut self.egui_state)
        {
            redraw |= state.on_window_event(window, &event).repaint;
        }
        if let WindowEvent::Focused(_) = event {
            // Alt+Tab and the like: the key that went with the modifier
            // went to another window, so letting go is not a tap.
            self.viewport.cancel_tap();
        }
        if let WindowEvent::Focused(true) = event {
            // The system setting may have changed while we were away.
            self.system_reduce_motion = crate::platform::system_reduces_motion();
            redraw = true;
        }
        if redraw {
            self.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                self.tabs
                    .close_window(&mut self.viewport, &mut self.files, &mut self.chrome);
                self.tab_work();
                self.request_redraw();
            }
            WindowEvent::Resized(size) => {
                log::debug!("resized to {}x{} px", size.width, size.height);
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }
                self.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.tab_work() {
                    self.request_redraw();
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            WindowEvent::DroppedFile(path) => {
                match crate::doc_files::open_path(&path.display().to_string()) {
                    Ok(o) => self
                        .files
                        .dropped(o, &mut self.viewport.session, &mut self.chrome),
                    Err(e) => self.chrome.status_note(e),
                }
                self.request_redraw();
            }
            WindowEvent::HoveredFile(_) => {
                self.chrome
                    .status_note("Drop to import (or open a .prism file)".into());
                self.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.tabs.exit {
            event_loop.exit();
            return;
        }
        // Sleep until the next frame egui asked for, or until an event arrives.
        match self.next_redraw {
            Some(at) if web_time::Instant::now() >= at => {
                self.next_redraw = None;
                self.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
