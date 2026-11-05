//! Drawing a scene off screen and reading the pixels back, for the
//! screenshot tests.

#![allow(dead_code)]

use prism_render::gpu::{Frame, Highlight, Renderer};
use prism_render::{Camera, Scene, Style};
use prism_topo::Model;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub renderer: Renderer,
}

impl Gpu {
    /// A GPU to draw with, or `None` if this machine has none (not even a
    /// software one). CI installs Mesa's software renderer.
    pub fn new() -> Option<Gpu> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .ok()?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("test device"),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                })
                .await
                .ok()?;
            let renderer = Renderer::new(&device);
            Some(Gpu {
                device,
                queue,
                renderer,
            })
        })
    }

    /// Draw and read back `w` by `h` RGBA pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn shot(
        &mut self,
        model: &Model,
        camera: Camera,
        w: u32,
        h: u32,
        ppp: f32,
        style: &Style,
        highlight: &Highlight,
    ) -> Image {
        let scene = Scene::build(model);
        self.renderer.resize(&self.device, w, h);
        self.renderer.set_scene(&self.device, &scene, style);
        self.renderer
            .set_highlight(&self.device, &scene, highlight, style);
        self.draw(camera, w, h, ppp, style, scene.bounds)
    }

    /// Draw the scene already uploaded from another camera, and read it back.
    pub fn draw(
        &mut self,
        camera: Camera,
        w: u32,
        h: u32,
        ppp: f32,
        style: &Style,
        bounds: Option<(prism_math::DVec3, prism_math::DVec3)>,
    ) -> Image {
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut enc,
            &Frame {
                camera,
                ppp,
                style,
                shadow: true,
                bounds,
            },
        );
        let row = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.renderer.color_texture().expect("target"),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
        let data = slice.get_mapped_range().expect("mapped");
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let r = &data[(y * row) as usize..(y * row + w * 4) as usize];
            for c in r.as_chunks::<4>().0 {
                px.push(*c);
            }
        }
        Image { w, h, px }
    }
}

pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<[u8; 4]>,
}

impl Image {
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        self.px[(y * self.w + x) as usize]
    }

    /// Pixels within `tol` (per channel) of a colour.
    pub fn count_near(&self, c: [u8; 3], tol: u8) -> usize {
        self.px.iter().filter(|p| near(**p, c, tol)).count()
    }

    /// Save as a PPM in `target/viewport-screens` (for looking at by hand).
    pub fn save(&self, name: &str) {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/viewport-screens");
        let _ = std::fs::create_dir_all(&dir);
        let mut out = format!("P6\n{} {}\n255\n", self.w, self.h).into_bytes();
        for p in &self.px {
            out.extend_from_slice(&p[..3]);
        }
        let _ = std::fs::write(dir.join(format!("{name}.ppm")), out);
    }
}

pub fn near(p: [u8; 4], c: [u8; 3], tol: u8) -> bool {
    (0..3).all(|i| p[i].abs_diff(c[i]) <= tol)
}

impl Gpu {
    /// The face number and depth at each pixel of the last shot.
    pub fn ids(&self, w: u32, h: u32) -> Vec<[u32; 2]> {
        let row = (w * 8).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.renderer.ids_texture().expect("target"),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
        let data = slice.get_mapped_range().expect("mapped");
        let mut out = Vec::new();
        for y in 0..h {
            let r = &data[(y * row) as usize..(y * row + w * 8) as usize];
            for c in r.as_chunks::<8>().0 {
                out.push([
                    u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                    u32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                ]);
            }
        }
        out
    }
}
