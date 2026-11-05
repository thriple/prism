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

/// How many input events the overlay keeps.
const LOG_CAPACITY: usize = 14;

/// Start the app. Natively this blocks until the window closes. On the web it
/// returns once the event loop is handed to the browser.
pub fn run() {
    let event_loop = EventLoop::<Gpu>::with_user_event()
        .build()
        .expect("could not create the event loop");
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

struct App {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    proxy: winit::event_loop::EventLoopProxy<Gpu>,
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
}

impl App {
    fn new(event_loop: &EventLoop<Gpu>) -> Self {
        let egui_ctx = egui::Context::default();
        prism_ui::theme::install(&egui_ctx);
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
            chrome: Chrome::new(crate::platform::keys()),
            modifiers_state: winit::keyboard::ModifiersState::empty(),
        }
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
        let (settings, demo, aurora, chrome) = (
            &mut self.settings,
            &mut self.demo,
            &mut self.aurora,
            &mut self.chrome,
        );
        let output = self.egui_ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx();
            prism_ui::set_motion(ctx, motion);
            aurora.prepare(ctx);
            let layout = chrome.show(ctx);
            let shown = overlay::Shown {
                debug: chrome.is_on("help.debug"),
                demo: chrome.is_on("help.themedemo"),
            };
            overlay::draw(ctx, layout.viewport, shown, &info, settings, demo);
        });
        egui_state.handle_platform_output(window, output.platform_output);
        let jobs = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        gpu.render(&jobs, output.textures_delta, output.pixels_per_point);

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

impl ApplicationHandler<Gpu> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        #[allow(unused_mut)]
        let mut attributes = Window::default_attributes().with_title("Prism");
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
                if proxy.send_event(gpu).is_err() {
                    log::error!("event loop closed before the GPU was ready");
                }
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, gpu: Gpu) {
        self.gpu = Some(gpu);
        if let Some(window) = &self.window {
            if let Some(gpu) = &mut self.gpu {
                gpu.resize(window.inner_size());
            }
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
        // Shortcuts, Esc and key recording go to the chrome first. A key it
        // uses is not passed on, so it does not also type into a field.
        let mut used = false;
        if let WindowEvent::KeyboardInput { event: key, .. } = &event
            && key.state == ElementState::Pressed
            && !key.repeat
            && let PhysicalKey::Code(code) = key.physical_key
        {
            let m = self.modifiers_state;
            let press = crate::actions::keymap::KeyPress {
                code: format!("{code:?}"),
                ctrl: m.control_key(),
                cmd: m.super_key(),
                shift: m.shift_key(),
                alt: m.alt_key(),
            };
            used = self
                .chrome
                .key(&press, self.egui_ctx.egui_wants_keyboard_input());
            redraw |= used;
        }
        // egui asks for a repaint after every redraw event, which would loop.
        if !used
            && !matches!(event, WindowEvent::RedrawRequested)
            && let (Some(window), Some(state)) = (&self.window, &mut self.egui_state)
        {
            redraw |= state.on_window_event(window, &event).repaint;
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
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                log::debug!("resized to {}x{} px", size.width, size.height);
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }
                self.request_redraw();
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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
