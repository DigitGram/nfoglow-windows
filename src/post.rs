// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The view through a render pass of its own, for what egui cannot do: the colour fringe, the shadow mask and the
//! aperture grille (a multiplying mask), the CRT power-on (the picture stretched out of a line) and the openings that
//! uncover the page bit by bit with the stars still behind it (modem, dither, code rain).
//!
//! The view is painted as usual, only into three layers of its own: the scene (everything), what lies under the page
//! (the stars) and the covers (where the page is not there yet). Those shapes are drawn into textures with egui's own
//! renderer, and one shader lays the result onto the window, as gpu.py of NfoGlow for Linux does with GSK nodes.
use eframe::egui::{self, Color32, LayerId, Rect, Shape};
use eframe::egui_wgpu::{self, wgpu, RenderState, ScreenDescriptor};
use std::hash::{Hash, Hasher};

pub const SCENE: &str = "post-scene";
pub const UNDER: &str = "post-under";
pub const COVER: &str = "post-cover";

pub fn layer(name: &str) -> LayerId {
    LayerId::new(egui::Order::Background, egui::Id::new(name))
}

/// What the shader does this frame.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Look {
    pub chroma: bool,
    pub mask: u32,  // 0 none, 1 shadow mask (dots), 2 aperture grille (stripes)
    pub grown: f32, // the CRT power-on: how far the picture has grown out of the line, 1 = whole
    pub cover: bool,
}

impl Look {
    pub fn needed(&self) -> bool {
        self.chroma || self.mask != 0 || self.grown < 1. || self.cover
    }
}

/// The textures and the pipeline, kept in egui's renderer (its callback resources) so the paint callback finds them.
struct Gpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    targets: Option<([u32; 2], [wgpu::Texture; 3], wgpu::BindGroup)>,
    srgb: bool,
}

/// Remembers what was drawn last, to ask for one more frame after a change (see `draw`).
#[derive(Default)]
pub struct Post {
    last: u64,
}

const SHADER: &str = r#"
struct U { view: vec4<f32>, bg: vec4<f32>, a: vec4<f32>, b: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var under: texture_2d<f32>;
@group(0) @binding(3) var cover: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    if (u.a.w < 0.5) { return c; }
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}
fn to_linear(c: vec3<f32>) -> vec3<f32> {
    if (u.a.w < 0.5) { return c; }
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// the picture at a pixel of the window: stretched while the tube switches on, the stars where the page is covered
fn picture(p: vec2<f32>) -> vec3<f32> {
    var q = p;
    if (u.b.x < 1.0) { q.y = u.b.y + (p.y - u.b.y) / max(u.b.x, 0.002); }
    if (q.x < u.view.x || q.y < u.view.y || q.x >= u.view.z || q.y >= u.view.w) { return u.bg.rgb; }
    let i = vec2<i32>(floor(q));
    var c = textureLoad(scene, i, 0).rgb;
    if (u.b.w > 0.5) { c = mix(c, textureLoad(under, i, 0).rgb, textureLoad(cover, i, 0).a); }
    return to_gamma(c);
}

fn luma(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.18, 0.35, 0.07)); }

// one pixel of a tube's colour mask, counted from the corner of the view
fn mask(p: vec2<f32>) -> vec3<f32> {
    let red = vec3<f32>(1.0, 0.62, 0.62);
    let green = vec3<f32>(0.62, 1.0, 0.62);
    let blue = vec3<f32>(0.62, 0.62, 1.0);
    let at = vec2<u32>(floor(p - u.view.xy));
    if (u.a.z > 1.5) {   // aperture grille: red, green, blue stripes
        let k = at.x % 3u;
        return select(select(blue, green, k == 1u), red, k == 0u);
    }
    let row = at.y % 6u;
    if (row == 2u || row == 5u) { return vec3<f32>(0.5); }
    let k = ((at.x + select(0u, 3u, row > 2u)) % 6u) / 2u;   // R R G G B B, the second triad shifted by three
    return select(select(blue, green, k == 1u), red, k == 0u);
}

@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pos.xy;
    var c = picture(p);
    if (u.a.x > 0.5) {
        // the colour fringe: red seams on one side of every edge, blue on the other, from the brightness
        let l = luma(c);
        let shift = vec2<f32>(u.b.z, 0.0);
        let seam = vec3<f32>(abs(luma(picture(p + shift)) - l), 0.0, abs(luma(picture(p - shift)) - l));
        if (u.a.y > 0.5) { c = c + seam - c * seam; } else { c = abs(c - seam); }   // screen on dark, difference on light
    }
    if (u.a.z > 0.5) { c = c * mask(p); }
    return vec4<f32>(to_linear(c), 1.0);
}
"#;

pub fn setup(rs: &RenderState) {
    let device = &rs.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("post"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            texture(1),
            texture(2),
            texture(3),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("post"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("post"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState { format: rs.target_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("post"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let srgb = rs.target_format.is_srgb();
    rs.renderer.write().callback_resources.insert(Gpu { pipeline, layout, uniforms, targets: None, srgb });
}

/// One pixel of a tube's colour mask (as the shader does it), counted from the corner: mask 1 a shadow mask (dots in
/// staggered triads), 2 an aperture grille (red, green, blue stripes). Factors to multiply the picture with.
pub fn mask(mask: u32, x: usize, y: usize) -> [f32; 3] {
    let (red, green, blue) = ([1., 0.62, 0.62], [0.62, 1., 0.62], [0.62, 0.62, 1.]);
    let pick = |k: usize| [red, green, blue][k];
    if mask == 2 {
        return pick(x % 3);
    }
    match y % 6 {
        2 | 5 => [0.5; 3],
        row => pick((x + if row > 2 { 3 } else { 0 }) % 6 / 2),
    }
}

fn clear(c: Color32, srgb: bool) -> wgpu::Color {
    let f = |v: u8| {
        let v = v as f64 / 255.;
        if srgb { if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } } else { v }
    };
    wgpu::Color { r: f(c.r()), g: f(c.g()), b: f(c.b()), a: c.a() as f64 / 255. }
}

impl Post {
    /// Takes what was painted into the three layers, draws it into the textures and returns the shape that shows
    /// it in view. fill: the colour under the page (the panel's). Call it after all of the view is painted.
    pub fn draw(&mut self, ctx: &egui::Context, rs: &RenderState, view: Rect, fill: Color32, bg: Color32, look: Look) -> Shape {
        let ppp = ctx.pixels_per_point();
        let size = (ctx.content_rect().size() * ppp).round();
        let size = [size.x.max(1.) as u32, size.y.max(1.) as u32];
        let screen = ScreenDescriptor { size_in_pixels: size, pixels_per_point: ppp };
        let jobs = [SCENE, UNDER, COVER].map(|name| {
            let shapes = ctx.graphics_mut(|g| std::mem::take(g.entry(layer(name))));
            ctx.tessellate(shapes.all_entries().cloned().collect(), ppp)
        });
        // a texture or glyph that is new in this frame reaches the renderer only when the frame is painted: draw once
        // more in the next frame whenever something changed
        let mut hasher = std::hash::DefaultHasher::new();
        (size, ppp.to_bits()).hash(&mut hasher);
        for job in jobs.iter().flatten() {
            let r = job.clip_rect;
            [r.min.x, r.min.y, r.max.x, r.max.y].map(f32::to_bits).hash(&mut hasher);
            if let egui::epaint::Primitive::Mesh(mesh) = &job.primitive {
                mesh.texture_id.hash(&mut hasher);
                mesh.indices.hash(&mut hasher);
                for v in &mesh.vertices {
                    ([v.pos.x, v.pos.y, v.uv.x, v.uv.y].map(f32::to_bits), v.color.to_array()).hash(&mut hasher);
                }
            }
        }
        let now = hasher.finish();
        if std::mem::replace(&mut self.last, now) != now {
            ctx.request_repaint();
        }
        let mut renderer = rs.renderer.write();
        let srgb = renderer.callback_resources.get::<Gpu>().map(|g| g.srgb).unwrap_or(false);
        // the textures, as big as the window
        let gpu = renderer.callback_resources.get_mut::<Gpu>().expect("post::setup ran");
        if gpu.targets.as_ref().is_none_or(|t| t.0 != size) {
            let make = |label| {
                rs.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: rs.target_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            };
            let textures = [make("post-scene"), make("post-under"), make("post-cover")];
            let views: Vec<_> = textures.iter().map(|t| t.create_view(&Default::default())).collect();
            let group = rs.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &gpu.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: gpu.uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[0]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[1]) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[2]) },
                ],
            });
            gpu.targets = Some((size, textures, group));
        }
        let views: Vec<_> = gpu.targets.as_ref().unwrap().1.iter().map(|t| t.create_view(&Default::default())).collect();
        for (i, job) in jobs.iter().enumerate() {
            if i > 0 && !look.cover {
                continue;
            }
            // one submit per layer: the renderer has one vertex buffer, and the next layer's upload would replace it
            let mut encoder = rs.device.create_command_encoder(&Default::default());
            let extra = renderer.update_buffers(&rs.device, &rs.queue, &mut encoder, job, &screen);
            {
                let base = if i == 2 { Color32::TRANSPARENT } else { fill };
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("post"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &views[i],
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear(base, srgb)), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                renderer.render(&mut pass.forget_lifetime(), job, &screen);
            }
            rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
        }
        let px = |r: Rect| [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp];
        let v = px(view);
        let g = |c: u8| c as f32 / 255.;
        let uniforms: [f32; 16] = [
            v[0], v[1], v[2], v[3],
            g(bg.r()), g(bg.g()), g(bg.b()), 1.,
            look.chroma as u8 as f32, crate::core::is_dark([bg.r(), bg.g(), bg.b()]) as u8 as f32, look.mask as f32, srgb as u8 as f32,
            look.grown, (v[1] + v[3]) / 2., (2. * ppp).round().max(1.), look.cover as u8 as f32,
        ];
        rs.queue.write_buffer(&gpu_buffer(&renderer), 0, &uniforms.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
        Shape::Callback(egui_wgpu::Callback::new_paint_callback(view, Composite))
    }
}

fn gpu_buffer(renderer: &egui_wgpu::Renderer) -> wgpu::Buffer {
    renderer.callback_resources.get::<Gpu>().unwrap().uniforms.clone()
}

struct Composite;

impl egui_wgpu::CallbackTrait for Composite {
    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        let Some(Gpu { pipeline, targets: Some((_, _, group)), .. }) = resources.get::<Gpu>() else { return };
        // the whole window: egui has set the viewport to the callback's rect, the shader works in window pixels
        let size = resources.get::<Gpu>().and_then(|g| g.targets.as_ref()).map(|t| t.0).unwrap();
        pass.set_viewport(0., 0., size[0] as f32, size[1] as f32, 0., 1.);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_masks_repeat_as_on_linux() {
        let (r, g, b) = ([1., 0.62, 0.62], [0.62, 1., 0.62], [0.62, 0.62, 1.]);
        assert_eq!((0..4).map(|x| mask(2, x, 7)).collect::<Vec<_>>(), [r, g, b, r]); // stripes, in every row
        assert_eq!((0..6).map(|x| mask(1, x, 0)).collect::<Vec<_>>(), [r, r, g, g, b, b]); // a triad of dots
        assert_eq!((0..6).map(|x| mask(1, x, 4)).collect::<Vec<_>>(), [g, b, b, r, r, g]); // the next one shifted by three
        assert_eq!((mask(1, 3, 2), mask(1, 0, 5), mask(1, 0, 6)), ([0.5; 3], [0.5; 3], r)); // dark gaps between them
        assert!(!Look { grown: 1., ..Default::default() }.needed() && Look { grown: 0.5, ..Default::default() }.needed());
    }
}
