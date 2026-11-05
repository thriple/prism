//! GPU setup and the per-frame draw: clear the screen, then draw the egui overlay.

use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::window::Window;

/// The window background behind the app's frame, from the theme layer.
const CLEAR: egui::Color32 = prism_ui::theme::color::SURFACE;

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    adapter_info: wgpu::AdapterInfo,
    egui: egui_wgpu::Renderer,
}

impl Gpu {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::util::new_instance_with_webgpu_detection(
            wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        )
        .await;
        let surface = instance
            .create_surface(window.clone())
            .expect("could not create a drawing surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .expect("no suitable GPU adapter");
        let adapter_info = adapter.get_info();
        log::info!("GPU adapter: {adapter_info:?}");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("prism device"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("could not open the GPU device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = pick_format(&caps.formats);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let egui = egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

        Self {
            surface,
            device,
            queue,
            config,
            adapter_info,
            egui,
        }
    }

    pub fn adapter_summary(&self) -> String {
        format!(
            "{} ({:?})",
            self.adapter_info.name, self.adapter_info.backend
        )
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            // Minimised: keep the old surface until the window comes back.
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        jobs: &[egui::ClippedPrimitive],
        mut textures: egui::TexturesDelta,
        pixels_per_point: f32,
    ) {
        // Apply texture changes even if this frame is skipped, so none are lost.
        for (id, deltas) in textures.set.drain() {
            for delta in &deltas {
                self.egui
                    .update_texture(&self.device, &self.queue, id, delta);
            }
        }
        let freed: Vec<egui::TextureId> = textures.free.drain().collect();
        self.draw(jobs, pixels_per_point);
        for id in &freed {
            self.egui.free_texture(id);
        }
    }

    fn draw(&mut self, jobs: &[egui::ClippedPrimitive], pixels_per_point: f32) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let mut buffers =
            self.egui
                .update_buffers(&self.device, &self.queue, &mut encoder, jobs, &screen);

        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear and overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color(self.config.format)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            let mut pass = pass.forget_lifetime();
            self.egui.render(&mut pass, jobs, &screen);
        }

        buffers.push(encoder.finish());
        self.queue.submit(buffers);
        self.queue.present(frame);
    }
}

/// Prefer a plain (non-sRGB) 8-bit format, which egui expects; fall back to
/// whatever the surface offers first.
fn pick_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(|f| {
            matches!(
                f,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            )
        })
        .unwrap_or(formats[0])
}

/// The clear colour for this surface format. An sRGB surface expects linear
/// values; a plain one takes the sRGB values as they are.
fn clear_color(format: wgpu::TextureFormat) -> wgpu::Color {
    let [r, g, b] = [CLEAR.r(), CLEAR.g(), CLEAR.b()].map(|c| {
        let c = f64::from(c) / 255.0;
        if format.is_srgb() {
            srgb_to_linear(c)
        } else {
            c
        }
    });
    wgpu::Color { r, g, b, a: 1.0 }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_surface_gets_srgb_values_unchanged() {
        let c = clear_color(wgpu::TextureFormat::Bgra8Unorm);
        assert!((c.r - f64::from(CLEAR.r()) / 255.0).abs() < 1e-12);
        assert!((c.b - f64::from(CLEAR.b()) / 255.0).abs() < 1e-12);
    }

    #[test]
    fn srgb_surface_gets_linear_values() {
        let c = clear_color(wgpu::TextureFormat::Bgra8UnormSrgb);
        // Linear light is darker than the sRGB value for dark greys.
        let srgb = f64::from(CLEAR.r()) / 255.0;
        assert!((c.r - srgb_to_linear(srgb)).abs() < 1e-12);
        assert!(c.r < srgb * 0.2, "r was {}", c.r);
        // The known value for #26: about 0.019.
        assert!((srgb_to_linear(38.0 / 255.0) - 0.019_382).abs() < 1e-5);
    }

    #[test]
    fn prefers_a_plain_format() {
        let formats = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ];
        assert_eq!(pick_format(&formats), wgpu::TextureFormat::Bgra8Unorm);
        assert_eq!(
            pick_format(&[wgpu::TextureFormat::Rgba16Float]),
            wgpu::TextureFormat::Rgba16Float
        );
    }
}
