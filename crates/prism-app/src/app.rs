//! The window and event loop. Kept thin: anything testable lives in other modules.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::frame_stats::FrameStats;
use crate::gpu::Gpu;
use crate::input_log::{InputEvent, InputLog, Modifiers, WheelUnit};

/// How many input events the overlay keeps.
const LOG_CAPACITY: usize = 24;

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
}

impl App {
    fn new(event_loop: &EventLoop<Gpu>) -> Self {
        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::dark());
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
        let info = OverlayInfo {
            stats: &self.stats,
            log: &self.log,
            cursor: self.cursor,
            modifiers: self.modifiers,
            size: window.inner_size(),
            scale: window.scale_factor(),
            adapter: gpu.adapter_summary(),
        };
        let output = self
            .egui_ctx
            .run_ui(raw_input, |ui| draw_overlay(ui, &info));
        egui_state.handle_platform_output(window, output.platform_output);
        let jobs = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        gpu.render(&jobs, output.textures_delta, output.pixels_per_point);

        // Keep drawing so the frame counter moves. Later chunks redraw on demand.
        window.request_redraw();
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
        if let Some(input) = self.input_event(&event) {
            self.log.push(input);
        }

        if let (Some(window), Some(state)) = (&self.window, &mut self.egui_state) {
            let _ = state.on_window_event(window, &event);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                log::debug!("resized to {}x{} px", size.width, size.height);
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

struct OverlayInfo<'a> {
    stats: &'a FrameStats,
    log: &'a InputLog,
    cursor: Option<(f64, f64)>,
    modifiers: Modifiers,
    size: winit::dpi::PhysicalSize<u32>,
    scale: f64,
    adapter: String,
}

/// Which build this is, set by the release workflow (for example "build 12").
/// Local builds say "local".
const BUILD: &str = match option_env!("PRISM_BUILD") {
    Some(b) => b,
    None => "local",
};

fn draw_overlay(ui: &mut egui::Ui, info: &OverlayInfo<'_>) {
    egui::Window::new("Prism debug")
        .default_pos([12.0, 12.0])
        .default_width(420.0)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            let fps = info
                .stats
                .fps()
                .map_or_else(|| "…".to_string(), |f| format!("{f:.0}"));
            ui.monospace(format!("build {BUILD}"));
            ui.monospace(format!("frame {}   fps {fps}", info.stats.frames()));
            ui.monospace(format!(
                "size {}x{} px   scale {:.2}",
                info.size.width, info.size.height, info.scale
            ));
            ui.monospace(format!("gpu {}", info.adapter));
            let cursor = info.cursor.map_or_else(
                || "outside".to_string(),
                |(x, y)| format!("({x:.0}, {y:.0})"),
            );
            ui.monospace(format!("cursor {cursor}   mods {}", info.modifiers));
            ui.separator();
            ui.label(format!("Input events ({} total)", info.log.total()));
            for line in info.log.lines() {
                ui.monospace(line);
            }
        });
}
