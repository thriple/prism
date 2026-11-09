//! Drawing the viewport with wgpu.
//!
//! The viewport is drawn into its own texture, which the app shows inside
//! the viewport well. Three passes: (1) sky and ground, the ground shadow,
//! faces, hover and selection tints; (2) the faces again, writing which face
//! is in front at each pixel; (3) the ground grid and axes, edges,
//! (optionally) hidden edges, highlighted edges and vertex dots. Every pass
//! has one colour target, as WebGL2 needs.
//!
//! Depth is reversed-Z in a 32-bit float buffer, with the far plane at
//! infinity and the near plane fitted
//! to what is visible every frame.
//!
//! There is no line bias. The second pass records, at each pixel, which
//! face is nearest and its depth. A line is never hidden by its own faces,
//! and any other face hides it if it is nearer (with a tiny tolerance that
//! scales with distance). So edges never flicker against the faces they
//! bound, and lines behind a thin wall stay hidden however far away it is.

use std::collections::BTreeSet;

use bytemuck::{Pod, Zeroable};
use prism_math::DVec3;
use prism_ops::select::Selection;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, FOV_Y_DEG, View};
use crate::pick::Hit;
use crate::scene::Scene;
use crate::style::{FaceDraw, HiddenLines, Rgba, Style};

/// The texture format the viewport draws into (sRGB values, like the canvas).
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Where faces are drawn: the face's number (from 1) and its depth.
pub const ID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Uint;
/// The nearest face's plane at each pixel, as bits (normal and offset).
const PLANE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Uint;
/// Back faces (seen from behind) are pushed back by a few float steps, so
/// a front face always wins a tie with a back face along a shared edge.
const BACK_BIAS_CONSTANT: i32 = -100;
/// The grid and axes sit this fraction of their depth behind faces on the ground.
const GRID_PUSH: f32 = 1.0 - 2e-6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye_hi: [f32; 4],
    eye_lo: [f32; 4],
    viewport: [f32; 4],
    light: [f32; 4],
    params: [f32; 4],
    front: [f32; 4],
    back: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    ground_horizon: [f32; 4],
    ground_bottom: [f32; 4],
    shadow: [f32; 4],
    stipple: [f32; 4],
    /// From the eye-relative model to the sun's shadow map.
    light_vp: [[f32; 4]; 4],
    /// On (1 or 0), depth bias, one texel in the map, unused.
    shadow_map: [f32; 4],
}

/// The sun's shadow map, in texels each way.
const SHADOW_SIZE: u32 = 2048;
/// How far a surface may be behind the depth in the map and still be lit,
/// as a share of the map's depth range.
const SHADOW_BIAS: f32 = 0.002;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Draw {
    color: [f32; 4],
    mode: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FaceVertex {
    hi: [f32; 3],
    lo: [f32; 3],
    normal: [f32; 3],
    face: u32,
    /// 1 for faces outside the open group, drawn faded (PLAN 1.2).
    faded: u32,
}

/// One line segment: two ends, the normals of its faces, colour and shape.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct LineInstance {
    a_hi: [f32; 3],
    a_lo: [f32; 3],
    b_hi: [f32; 3],
    b_lo: [f32; 3],
    n1: [f32; 3],
    n2: [f32; 3],
    color: [u8; 4],
    /// Width, dash and gap, in points.
    shape: [f32; 3],
    flags: u32,
    faces: [u32; 2],
}

const HAS_N1: u32 = 1;
const HAS_N2: u32 = 2;
const MODEL_EDGE: u32 = 4;
const FADE: u32 = 8;
const PUSH_BACK: u32 = 16;
const POINT: u32 = 64;
const NO_TEST: u32 = 128;
const MARK: u32 = 256;

fn split(v: DVec3) -> ([f32; 3], [f32; 3]) {
    let hi = v.as_vec3();
    let lo = (v - hi.as_dvec3()).as_vec3();
    (hi.to_array(), lo.to_array())
}

impl LineInstance {
    pub fn new(a: DVec3, b: DVec3, color: Rgba, width: f32) -> Self {
        let (a_hi, a_lo) = split(a);
        let (b_hi, b_lo) = split(b);
        Self {
            a_hi,
            a_lo,
            b_hi,
            b_lo,
            n1: [0.0; 3],
            n2: [0.0; 3],
            color: color.0,
            shape: [width, 0.0, 0.0],
            flags: 0,
            faces: [0, 0],
        }
    }

    fn dashed(mut self, dash: [f32; 2]) -> Self {
        self.shape[1] = dash[0];
        self.shape[2] = dash[1];
        self
    }

    fn flags(mut self, f: u32) -> Self {
        self.flags |= f;
        self
    }
}

/// Near and far planes that just contain the visible things (the model,
/// the ground grid and the axes).
pub fn fit_depth(view: &View, boxes: &[(DVec3, DVec3)]) -> (f64, f64) {
    let mut far: f64 = 0.0;
    let mut near = f64::MAX;
    for (lo, hi) in boxes {
        for i in 0..8 {
            let c = DVec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            );
            far = far.max((c - view.eye).length());
        }
        let closest = view.eye.clamp(*lo, *hi);
        near = near.min(closest.distance(view.eye));
    }
    if far <= 0.0 {
        return (1e-3, 1.0);
    }
    let far = far * 1.05;
    (near.mul_add(0.5, 0.0).max(far * 1e-7), far)
}

/// The view and reversed-Z projection, for positions relative to the eye:
/// depth is 1 at `near` and falls to 0 at infinity.
pub fn view_proj(view: &View, near: f64) -> glam::DMat4 {
    use glam::{DMat4, DVec4};
    let f = 1.0 / (FOV_Y_DEG.to_radians() * 0.5).tan();
    // Far is at infinity: depth = near / distance. A finite far plane
    // makes depth the difference of two nearly equal numbers when the far
    // plane sits just behind the model, which lost the thin-plate test on
    // real GPUs at 1 km.
    let proj = DMat4::from_cols(
        DVec4::new(f / view.aspect(), 0.0, 0.0, 0.0),
        DVec4::new(0.0, f, 0.0, 0.0),
        DVec4::new(0.0, 0.0, 0.0, -1.0),
        DVec4::new(0.0, 0.0, near, 0.0),
    );
    // Camera space: x right, y up, looking down -z.
    let look = DMat4::from_cols(
        DVec4::new(view.r.x, view.u.x, -view.f.x, 0.0),
        DVec4::new(view.r.y, view.u.y, -view.f.y, 0.0),
        DVec4::new(view.r.z, view.u.z, -view.f.z, 0.0),
        DVec4::W,
    );
    proj * look
}

fn face_numbers(scene: &Scene, faces: [Option<prism_topo::FaceId>; 2]) -> [u32; 2] {
    faces.map(|f| {
        f.and_then(|f| scene.face_index.get(&f))
            .map_or(0, |i| *i as u32 + 1)
    })
}

/// The sun's orthographic view of the box `lo`–`hi`, relative to `eye`,
/// looking along `-light` (depth 0 nearest the sun).
fn sun_view_proj(light: DVec3, lo: DVec3, hi: DVec3, eye: DVec3) -> prism_math::DMat4 {
    let l = light.normalize_or(DVec3::Z);
    let c = (lo + hi) * 0.5 - eye;
    let r = (hi - lo).length().max(1e-9) * 0.5;
    let up = if l.z.abs() > 0.99 { DVec3::Y } else { DVec3::Z };
    // The sun's camera: at `p`, looking along `f`; x and y across the
    // model's radius, depth from 0.5 r to 3.5 r mapped to 0..1.
    let p = c + l * (2.0 * r);
    let f = -l;
    let right = f.cross(up).normalize();
    let u = right.cross(f);
    let (near, far) = (0.5 * r, 3.5 * r);
    let rows = [
        (right / r, -right.dot(p) / r),
        (u / r, -u.dot(p) / r),
        (f / (far - near), (-f.dot(p) - near) / (far - near)),
    ];
    let col = |k: usize| glam::DVec4::new(rows[0].0[k], rows[1].0[k], rows[2].0[k], 0.0);
    prism_math::DMat4::from_cols(
        col(0),
        col(1),
        col(2),
        glam::DVec4::new(rows[0].1, rows[1].1, rows[2].1, 1.0),
    )
}

/// What to show highlighted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Highlight {
    pub hover: Option<Hit>,
    pub selection: Selection,
}

struct Target {
    size: [u32; 2],
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    ids: wgpu::Texture,
    ids_view: wgpu::TextureView,
    planes_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    ids_group: wgpu::BindGroup,
}

struct Lines {
    buffer: Option<wgpu::Buffer>,
    count: u32,
}

impl Lines {
    fn empty() -> Self {
        Self {
            buffer: None,
            count: 0,
        }
    }

    fn set(&mut self, device: &wgpu::Device, label: &str, lines: &[LineInstance]) {
        self.count = lines.len() as u32;
        self.buffer = (!lines.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(lines),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
    }

    fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let Some(b) = &self.buffer {
            pass.set_vertex_buffer(0, b.slice(..));
            pass.draw(0..6, 0..self.count);
        }
    }
}

struct Indices {
    buffer: Option<wgpu::Buffer>,
    count: u32,
}

impl Indices {
    fn set(&mut self, device: &wgpu::Device, label: &str, idx: &[u32]) {
        self.count = idx.len() as u32;
        self.buffer = (!idx.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(idx),
                usage: wgpu::BufferUsages::INDEX,
            })
        });
    }
}

struct DrawSlot {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub struct Renderer {
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    draws: Vec<DrawSlot>,
    ids_layout: wgpu::BindGroupLayout,
    bg: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    faces: [wgpu::RenderPipeline; 2],
    /// The faces' depth seen from the sun, and what faces read it with.
    shadow_map: wgpu::RenderPipeline,
    shadow_view: wgpu::TextureView,
    shadow_group: wgpu::BindGroup,
    /// Faces see-through (X-ray): blended over what is behind.
    xray: [wgpu::RenderPipeline; 2],
    tints: [wgpu::RenderPipeline; 2],
    ids: [wgpu::RenderPipeline; 2],
    line: wgpu::RenderPipeline,
    target: Option<Target>,
    face_vertices: Option<wgpu::Buffer>,
    face_indices: Indices,
    hover_faces: Indices,
    selected_faces: Indices,
    edges: Lines,
    /// Every vertex as a dot (View > Show > Points).
    points: Lines,
    grid: Lines,
    grid_style: Option<Style>,
    highlight_lines: Lines,
    dots: Lines,
    tris: wgpu::RenderPipeline,
    overlay_tris: Option<(wgpu::Buffer, u32)>,
    overlay: Lines,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TriVertex {
    hi: [f32; 3],
    lo: [f32; 3],
    color: [u8; 4],
}

// Draw slots.
const PLAIN: usize = 0;
const HOVER_TINT: usize = 1;
const SELECT_TINT: usize = 2;
const STIPPLE: usize = 3;
const HIDDEN: usize = 4;

/// Everything one frame needs.
pub struct Frame<'a> {
    pub camera: Camera,
    /// Pixels per point.
    pub ppp: f32,
    pub style: &'a Style,
    pub shadow: bool,
    /// Bounds of the model, to fit the depth range.
    pub bounds: Option<(DVec3, DVec3)>,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("viewport.wgsl").into()),
        });
        let uniform_layout = |label: &str| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        };
        let globals_layout = uniform_layout("viewport globals");
        let draw_layout = uniform_layout("viewport draw");
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = |layout: &wgpu::BindGroupLayout, buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let globals_group = group(&globals_layout, &globals);
        let draws = (0..5)
            .map(|_| {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport draw"),
                    size: std::mem::size_of::<Draw>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let group = group(&draw_layout, &buffer);
                DrawSlot { buffer, group }
            })
            .collect();
        let ids_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("face ids"),
            entries: &[0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }),
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun shadow map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("sun shadow map"),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sun shadow map"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sun shadow map"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        // Group 2 is the lines' face ids; faces read the shadow map at 3.
        let faces_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport faces"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&draw_layout),
                None,
                Some(&shadow_layout),
            ],
            immediate_size: 0,
        });
        let sun_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sun shadow map"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let lines_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport lines"),
            bind_group_layouts: &[Some(&globals_layout), Some(&draw_layout), Some(&ids_layout)],
            immediate_size: 0,
        });

        let face_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FaceVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Uint32, 4 => Uint32],
        };
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3,
                4 => Float32x3, 5 => Float32x3, 6 => Unorm8x4, 7 => Float32x3, 8 => Uint32,
                9 => Uint32x2
            ],
        };
        let tri_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TriVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4],
        };
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let min_blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Min,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        });
        let face_bias = wgpu::DepthBiasState::default();
        let back_bias = wgpu::DepthBiasState {
            constant: BACK_BIAS_CONSTANT,
            slope_scale: 0.0,
            clamp: 0.0,
        };
        // One colour target each. `compare` None means the lines pass, which
        // has no depth buffer.
        let pipeline = |label: &str,
                        vs: &str,
                        fs: &str,
                        buffers: &[wgpu::VertexBufferLayout],
                        blend: Option<wgpu::BlendState>,
                        compare: Option<wgpu::CompareFunction>,
                        write: bool,
                        bias: wgpu::DepthBiasState| {
            let target = |format| {
                Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            };
            let targets: Vec<_> = if fs == "id_fs" {
                vec![target(ID_FORMAT), target(PLANE_FORMAT)]
            } else {
                vec![target(COLOR_FORMAT)]
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(if compare.is_some() {
                    &faces_layout
                } else {
                    &lines_layout
                }),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &buffers.iter().cloned().map(Some).collect::<Vec<_>>(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: if label.ends_with("(front)") {
                        Some(wgpu::Face::Back)
                    } else if label.ends_with("(back)") {
                        Some(wgpu::Face::Front)
                    } else {
                        None
                    },
                    ..Default::default()
                },
                depth_stencil: compare.map(|c| wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(c),
                    stencil: wgpu::StencilState::default(),
                    bias,
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction as C;
        let none = wgpu::DepthBiasState::default();
        let face_l = std::slice::from_ref(&face_layout);
        let line_l = std::slice::from_ref(&line_layout);
        // Depth only, both sides of every face, pushed back a little so a
        // face does not shadow itself.
        let shadow_map = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sun shadow map"),
            layout: Some(&sun_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("sun_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(face_layout.clone())],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            shadow_map,
            shadow_view,
            shadow_group,
            bg: pipeline(
                "sky and ground",
                "bg_vs",
                "bg_fs",
                &[],
                None,
                Some(C::Always),
                false,
                none,
            ),
            shadow: pipeline(
                "ground shadow",
                "shadow_vs",
                "shadow_fs",
                face_l,
                min_blend,
                Some(C::Always),
                false,
                none,
            ),
            faces: [
                pipeline(
                    "faces (front)",
                    "face_vs",
                    "face_fs",
                    face_l,
                    None,
                    Some(C::Greater),
                    true,
                    face_bias,
                ),
                pipeline(
                    "faces (back)",
                    "face_vs",
                    "face_fs",
                    face_l,
                    None,
                    Some(C::Greater),
                    true,
                    back_bias,
                ),
            ],
            xray: [
                pipeline(
                    "x-ray faces (front)",
                    "face_vs",
                    "face_fs",
                    face_l,
                    alpha,
                    Some(C::Greater),
                    true,
                    face_bias,
                ),
                pipeline(
                    "x-ray faces (back)",
                    "face_vs",
                    "face_fs",
                    face_l,
                    alpha,
                    Some(C::Greater),
                    true,
                    back_bias,
                ),
            ],
            ids: [
                pipeline(
                    "face ids (front)",
                    "face_vs",
                    "id_fs",
                    face_l,
                    None,
                    Some(C::Equal),
                    false,
                    face_bias,
                ),
                pipeline(
                    "face ids (back)",
                    "face_vs",
                    "id_fs",
                    face_l,
                    None,
                    Some(C::Equal),
                    false,
                    back_bias,
                ),
            ],
            tints: [
                pipeline(
                    "face tints (front)",
                    "face_vs",
                    "tint_fs",
                    face_l,
                    alpha,
                    Some(C::GreaterEqual),
                    false,
                    face_bias,
                ),
                pipeline(
                    "face tints (back)",
                    "face_vs",
                    "tint_fs",
                    face_l,
                    alpha,
                    Some(C::GreaterEqual),
                    false,
                    back_bias,
                ),
            ],
            line: pipeline(
                "lines", "line_vs", "line_fs", line_l, alpha, None, false, none,
            ),
            tris: pipeline(
                "overlay triangles",
                "tri_vs",
                "tri_fs",
                std::slice::from_ref(&tri_layout),
                alpha,
                None,
                false,
                none,
            ),
            ids_layout,
            globals,
            globals_group,
            draws,
            target: None,
            face_vertices: None,
            face_indices: Indices {
                buffer: None,
                count: 0,
            },
            hover_faces: Indices {
                buffer: None,
                count: 0,
            },
            selected_faces: Indices {
                buffer: None,
                count: 0,
            },
            edges: Lines::empty(),
            points: Lines::empty(),
            grid: Lines::empty(),
            grid_style: None,
            highlight_lines: Lines::empty(),
            dots: Lines::empty(),
            overlay_tris: None,
            overlay: Lines::empty(),
        }
    }

    /// Upload the tools' feedback (guides, glyphs, rubber bands, previews).
    pub fn set_overlay(&mut self, device: &wgpu::Device, o: &prism_view::overlay::Overlay) {
        use prism_view::overlay::Shape;
        let mut lines: Vec<LineInstance> = o
            .lines
            .iter()
            .map(|l| {
                LineInstance::new(l.a, l.b, l.color, l.width)
                    .dashed(l.dash)
                    .flags(NO_TEST)
            })
            .collect();
        // Guides: faces in front of them hide them, as they do edges.
        lines.extend(
            o.tested
                .iter()
                .map(|l| LineInstance::new(l.a, l.b, l.color, l.width).dashed(l.dash)),
        );
        for m in &o.marks {
            let shape = match m.shape {
                Shape::Circle => 0.0,
                Shape::Triangle => 1.0,
                Shape::Square => 2.0,
                Shape::Diamond => 3.0,
                Shape::Cross => 4.0,
                Shape::Ring => 5.0,
                Shape::DiamondOutline => 6.0,
            };
            // The quad covers the shape and its outline.
            let mut l =
                LineInstance::new(m.at, m.at, m.color, 2.0 * (m.size + 4.0)).flags(MARK | NO_TEST);
            l.shape[1] = shape;
            l.shape[2] = m.size;
            lines.push(l);
        }
        self.overlay.set(device, "tool feedback", &lines);
        let verts: Vec<TriVertex> = o
            .tris
            .iter()
            .flat_map(|t| {
                t.p.map(|p| {
                    let (hi, lo) = split(p);
                    TriVertex {
                        hi,
                        lo,
                        color: t.color.0,
                    }
                })
            })
            .collect();
        self.overlay_tris = (!verts.is_empty()).then(|| {
            (
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tool feedback triangles"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                verts.len() as u32,
            )
        });
    }

    /// Make sure the target is `w` by `h` pixels. Returns true if it was
    /// made again (so the app must show the new texture).
    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) -> bool {
        let size = [w.max(1), h.max(1)];
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return false;
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport colour"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let ids = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport face ids"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ID_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let ids_view = ids.create_view(&Default::default());
        let planes_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("viewport face planes"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PLANE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let ids_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("face ids"),
            layout: &self.ids_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ids_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&planes_view),
                },
            ],
        });
        self.target = Some(Target {
            size,
            color_view: color.create_view(&Default::default()),
            depth_view: depth.create_view(&Default::default()),
            ids_view,
            planes_view,
            ids_group,
            ids,
            color,
        });
        true
    }

    pub fn color_view(&self) -> Option<&wgpu::TextureView> {
        self.target.as_ref().map(|t| &t.color_view)
    }

    /// The face numbers and depths of the last frame (for tests).
    pub fn ids_texture(&self) -> Option<&wgpu::Texture> {
        self.target.as_ref().map(|t| &t.ids)
    }

    pub fn color_texture(&self) -> Option<&wgpu::Texture> {
        self.target.as_ref().map(|t| &t.color)
    }

    /// Upload the scene's faces and edges (when the model changed).
    pub fn set_scene(&mut self, device: &wgpu::Device, scene: &Scene, style: &Style) {
        let verts: Vec<FaceVertex> = scene
            .mesh
            .positions
            .iter()
            .zip(&scene.mesh.normals)
            .zip(&scene.mesh.faces)
            .map(|((p, n), f)| {
                let (hi, lo) = split(*p);
                FaceVertex {
                    hi,
                    lo,
                    normal: n.as_vec3().to_array(),
                    face: f + 1,
                    faded: u32::from(scene.faces[*f as usize].faded),
                }
            })
            .collect();
        self.face_vertices = (!verts.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("face vertices"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        self.face_indices
            .set(device, "face indices", &scene.mesh.indices);
        let edges: Vec<LineInstance> = scene
            .edges
            .iter()
            .flat_map(|e| {
                // Along an axis, in its colour if asked; edges outside the
                // open group faint.
                let mut color = style.edge;
                if style.lines_by_axis && e.mid.is_empty() {
                    let d = (e.b - e.a).normalize_or_zero();
                    if let Some(i) = (0..3).find(|i| d[*i].abs() > 1.0 - 1e-9) {
                        color = style.axes[i];
                    }
                }
                if e.faded {
                    color = color.with_alpha(style.faded_alpha);
                }
                // A hidden object's ghost looks like a hidden line: faint,
                // and dashed unless hidden lines are set to Faint.
                if e.ghost {
                    color = style
                        .edge
                        .with_alpha(crate::style::alpha(style.hidden_alpha));
                }
                // An arc is drawn as its pieces.
                e.segments()
                    .map(|(a, b)| {
                        let mut l =
                            LineInstance::new(a, b, color, style.edge_width).flags(MODEL_EDGE);
                        if e.ghost && style.hidden_lines != HiddenLines::Faint {
                            l = l.dashed(style.hidden_dash);
                        }
                        if let Some(n) = e.normals[0] {
                            l.n1 = n.as_vec3().to_array();
                            l.flags |= HAS_N1;
                        }
                        if let Some(n) = e.normals[1] {
                            l.n2 = n.as_vec3().to_array();
                            l.flags |= HAS_N2;
                        }
                        l.faces = face_numbers(scene, e.faces);
                        l
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        self.edges.set(device, "edges", &edges);
        let points: Vec<LineInstance> = scene
            .vertices
            .iter()
            .map(|(_, p)| LineInstance::new(*p, *p, style.edge, style.point_size).flags(POINT))
            .collect();
        self.points.set(device, "points", &points);
    }

    /// Rebuild what is highlighted (when the hover or the selection changed).
    pub fn set_highlight(
        &mut self,
        device: &wgpu::Device,
        scene: &Scene,
        h: &Highlight,
        style: &Style,
    ) {
        let tris = |faces: &mut dyn Iterator<Item = prism_topo::FaceId>| -> Vec<u32> {
            let mut out = Vec::new();
            for f in faces {
                if let Some(f) = scene.face(f) {
                    out.extend_from_slice(
                        &scene.mesh.indices[f.tris.start as usize..f.tris.end as usize],
                    );
                }
            }
            out
        };
        let hover_face = match h.hover {
            Some(Hit::Face(f)) => Some(f),
            _ => None,
        };
        self.hover_faces
            .set(device, "hovered face", &tris(&mut hover_face.into_iter()));
        self.selected_faces.set(
            device,
            "selected faces",
            &tris(&mut h.selection.faces.iter().copied()),
        );
        let mut lines = Vec::new();
        let edge_line = |id, color| -> Vec<LineInstance> {
            scene.edge(id).map_or(Vec::new(), |e| {
                e.segments()
                    .map(|(a, b)| {
                        let mut l = LineInstance::new(a, b, color, style.highlight_width);
                        l.faces = face_numbers(scene, e.faces);
                        l
                    })
                    .collect()
            })
        };
        lines.extend(
            h.selection
                .edges
                .iter()
                .flat_map(|e| edge_line(*e, style.selected)),
        );
        if let Some(Hit::Edge(e)) = h.hover {
            lines.extend(edge_line(e, style.hover));
        }
        self.highlight_lines
            .set(device, "highlighted edges", &lines);
        let point = |id| {
            scene
                .vertices
                .binary_search_by(|(v, _)| v.cmp(&id))
                .ok()
                .map(|i| scene.vertices[i].1)
        };
        let mut dots = Vec::new();
        let mut dot = |p: DVec3, color: Rgba| {
            let r = style.vertex_dot_radius;
            let ring = style.vertex_ring_width * 0.5;
            dots.push(LineInstance::new(p, p, style.vertex_ring, 2.0 * (r + ring)).flags(POINT));
            dots.push(LineInstance::new(p, p, color, 2.0 * (r - ring)).flags(POINT));
        };
        let selected: BTreeSet<_> = h.selection.vertices.iter().copied().collect();
        for v in &selected {
            if let Some(p) = point(*v) {
                dot(p, style.selected);
            }
        }
        if let Some(Hit::Vertex(v)) = h.hover
            && let Some(p) = point(v)
        {
            dot(p, style.hover);
        }
        self.dots.set(device, "vertex dots", &dots);
    }

    fn ensure_grid(&mut self, device: &wgpu::Device, style: &Style) {
        if self.grid_style.as_ref() == Some(style) {
            return;
        }
        let mut lines = Vec::new();
        let n = if style.show_grid { style.grid_lines } else { 0 };
        let sp = style.grid_spacing;
        let step = style.grid_major_every.max(1);
        for k in -n..=n {
            if k == 0 {
                continue;
            }
            let major = k % step == 0;
            let (c, a, w) = if major {
                (
                    style.grid_major,
                    style.grid_major_alpha,
                    style.grid_major_width,
                )
            } else {
                (
                    style.grid_minor,
                    style.grid_minor_alpha,
                    style.grid_minor_width,
                )
            };
            let color = c.with_alpha(crate::style::alpha(a));
            let mut s = -n;
            while s < n {
                let s1 = (s + step).min(n);
                let (x, y0, y1) = (f64::from(k) * sp, f64::from(s) * sp, f64::from(s1) * sp);
                lines.push(
                    LineInstance::new(DVec3::new(x, y0, 0.0), DVec3::new(x, y1, 0.0), color, w)
                        .flags(FADE | PUSH_BACK),
                );
                lines.push(
                    LineInstance::new(DVec3::new(y0, x, 0.0), DVec3::new(y1, x, 0.0), color, w)
                        .flags(FADE | PUSH_BACK),
                );
                s = s1;
            }
        }
        let l = style.axis_length;
        let pos = crate::style::alpha(style.axis_alpha);
        let neg = crate::style::alpha(style.axis_negative_alpha);
        let axes = if style.show_axes { 3 } else { 0 };
        for (i, dir) in [DVec3::X, DVec3::Y, DVec3::Z]
            .into_iter()
            .enumerate()
            .take(axes)
        {
            let c = style.axes[i];
            lines.push(
                LineInstance::new(DVec3::ZERO, dir * l, c.with_alpha(pos), style.axis_width)
                    .flags(PUSH_BACK),
            );
            if i < 2 {
                lines.push(
                    LineInstance::new(-dir * l, DVec3::ZERO, c.with_alpha(neg), style.axis_width)
                        .dashed(style.axis_negative_dash)
                        .flags(PUSH_BACK),
                );
            }
        }
        self.grid.set(device, "grid and axes", &lines);
        self.grid_style = Some(*style);
    }

    /// The ground grid and the axes, as boxes (kept apart, so the eye is
    /// rarely inside one and the near plane can stay well away from it).
    fn ground_boxes(style: &Style) -> [(DVec3, DVec3); 4] {
        let g = f64::from(style.grid_lines) * style.grid_spacing;
        let l = style.axis_length;
        [
            (DVec3::new(-g, -g, 0.0), DVec3::new(g, g, 0.0)),
            (DVec3::new(-l, 0.0, 0.0), DVec3::new(l, 0.0, 0.0)),
            (DVec3::new(0.0, -l, 0.0), DVec3::new(0.0, l, 0.0)),
            (DVec3::ZERO, DVec3::new(0.0, 0.0, l)),
        ]
    }

    /// Draw one frame into the viewport texture.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        f: &Frame,
    ) {
        self.ensure_grid(device, f.style);
        let Some(target) = &self.target else {
            return;
        };
        let [w, h] = target.size;
        let ppp = f.ppp.max(0.1);
        let view = View::new(
            &f.camera,
            f64::from(w) / f64::from(ppp),
            f64::from(h) / f64::from(ppp),
        );
        let mut boxes = Self::ground_boxes(f.style).to_vec();
        boxes.extend(f.bounds);
        let (near, _far) = fit_depth(&view, &boxes);
        let vp = view_proj(&view, near).as_mat4();
        let (eye_hi, eye_lo) = split(view.eye);
        let s = f.style;
        let c = |c: Rgba| c.to_f32();
        let light = DVec3::from_array(s.light).normalize().as_vec3();
        let sd = DVec3::from_array(s.shadow_dir).normalize().as_vec3();
        // The sun's view of the model, relative to the eye (PLAN 4).
        let sun_vp = f
            .bounds
            .filter(|_| s.face_shadows && s.faces != FaceDraw::None)
            .map(|(lo, hi)| sun_view_proj(DVec3::from_array(s.light), lo, hi, view.eye));
        let globals = Globals {
            view_proj: vp.to_cols_array_2d(),
            eye_hi: [eye_hi[0], eye_hi[1], eye_hi[2], 0.0],
            eye_lo: [eye_lo[0], eye_lo[1], eye_lo[2], near as f32],
            viewport: [
                w as f32,
                h as f32,
                ppp,
                (view.horizon_y() * f64::from(ppp)) as f32,
            ],
            light: [light.x, light.y, light.z, s.ambient],
            params: [s.diffuse, s.profile_width, s.grid_fade, GRID_PUSH],
            front: c(s.face_front),
            back: c(s.face_back),
            sky_top: c(s.sky_top),
            sky_horizon: c(s.sky_horizon),
            // Without the ground, the sky fills the background.
            ground_horizon: c(if s.show_ground {
                s.ground_near_horizon
            } else {
                s.sky_horizon
            }),
            ground_bottom: c(if s.show_ground {
                s.ground_bottom
            } else {
                s.sky_horizon
            }),
            shadow: [sd.x, sd.y, sd.z, s.shadow_alpha],
            stipple: [s.stipple_tile, s.stipple_dot, 0.0, 0.0],
            light_vp: sun_vp.map_or([[0.0; 4]; 4], |m| m.as_mat4().to_cols_array_2d()),
            shadow_map: [
                if sun_vp.is_some() { 1.0 } else { 0.0 },
                SHADOW_BIAS,
                1.0 / SHADOW_SIZE as f32,
                0.0,
            ],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let tint = |col: Rgba, a: f32, mode: f32| Draw {
            color: {
                let mut v = col.to_f32();
                v[3] *= a;
                v
            },
            mode: [mode, 1.0, 0.0, 0.0],
        };
        let hidden = match s.hidden_lines {
            HiddenLines::Off | HiddenLines::Faint => [2.0, s.hidden_alpha, 0.0, 0.0],
            HiddenLines::Dashed => [1.0, s.hidden_alpha, s.hidden_dash[0], s.hidden_dash[1]],
        };
        let draws = [
            Draw {
                color: [0.0; 4],
                mode: [0.0, 1.0, 0.0, 0.0],
            },
            tint(s.hover, s.hover_face_alpha, 0.0),
            tint(s.selected, s.selected_face_alpha, 0.0),
            tint(s.stipple, 1.0, 1.0),
            Draw {
                color: [0.0; 4],
                mode: hidden,
            },
        ];
        for (slot, d) in self.draws.iter().zip(draws) {
            queue.write_buffer(&slot.buffer, 0, bytemuck::bytes_of(&d));
        }

        if let (Some(_), Some(vb), Some(ib)) =
            (sun_vp, &self.face_vertices, &self.face_indices.buffer)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sun shadow map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_map);
            pass.set_bind_group(0, &self.globals_group, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.face_indices.count, 0, 0..1);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport faces"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_bind_group(1, &self.draws[PLAIN].group, &[]);
        pass.set_bind_group(3, &self.shadow_group, &[]);
        pass.set_pipeline(&self.bg);
        pass.draw(0..3, 0..1);

        let faces = (s.faces != FaceDraw::None).then_some(()).and(
            self.face_vertices
                .as_ref()
                .zip(self.face_indices.buffer.as_ref()),
        );
        if let Some((vb, ib)) = faces {
            pass.set_vertex_buffer(0, vb.slice(..));
            if f.shadow {
                pass.set_pipeline(&self.shadow);
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.face_indices.count, 0, 0..1);
            }
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            let pipes = if s.faces == FaceDraw::XRay {
                &self.xray
            } else {
                &self.faces
            };
            for p in pipes {
                pass.set_pipeline(p);
                pass.draw_indexed(0..self.face_indices.count, 0, 0..1);
            }
            for p in &self.tints {
                pass.set_pipeline(p);
                for (idx, slots) in [
                    (&self.selected_faces, &[SELECT_TINT, STIPPLE][..]),
                    (&self.hover_faces, &[HOVER_TINT][..]),
                ] {
                    if let Some(b) = &idx.buffer {
                        pass.set_index_buffer(b.slice(..), wgpu::IndexFormat::Uint32);
                        for slot in slots {
                            pass.set_bind_group(1, &self.draws[*slot].group, &[]);
                            pass.draw_indexed(0..idx.count, 0, 0..1);
                        }
                    }
                }
            }
        }
        drop(pass);

        // Which face is in front at each pixel, for the lines.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport face ids"),
            color_attachments: &[&target.ids_view, &target.planes_view].map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            }),
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        // Only solid faces hide lines (X-ray and Wireframe show them all).
        if let (Some(vb), Some(ib), FaceDraw::Solid) =
            (&self.face_vertices, &self.face_indices.buffer, s.faces)
        {
            pass.set_bind_group(0, &self.globals_group, &[]);
            pass.set_bind_group(1, &self.draws[PLAIN].group, &[]);
            pass.set_bind_group(3, &self.shadow_group, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            for p in &self.ids {
                pass.set_pipeline(p);
                pass.draw_indexed(0..self.face_indices.count, 0, 0..1);
            }
        }
        drop(pass);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport lines"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_bind_group(1, &self.draws[PLAIN].group, &[]);
        pass.set_bind_group(2, &target.ids_group, &[]);
        pass.set_pipeline(&self.line);
        self.grid.draw(&mut pass);
        if s.show_lines {
            self.edges.draw(&mut pass);
            if s.hidden_lines != HiddenLines::Off {
                pass.set_bind_group(1, &self.draws[HIDDEN].group, &[]);
                self.edges.draw(&mut pass);
                pass.set_bind_group(1, &self.draws[PLAIN].group, &[]);
            }
        }
        if s.show_points {
            self.points.draw(&mut pass);
        }
        self.highlight_lines.draw(&mut pass);
        self.dots.draw(&mut pass);
        if let Some((b, n)) = &self.overlay_tris {
            pass.set_pipeline(&self.tris);
            pass.set_vertex_buffer(0, b.slice(..));
            pass.draw(0..*n, 0..1);
            pass.set_pipeline(&self.line);
        }
        self.overlay.draw(&mut pass);
    }
}
