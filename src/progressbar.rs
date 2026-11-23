//! GPU-rendered liquid-lava progress bar fill.
//!
//! The WGSL source lives in `shaders/progressbar.wgsl` and is compiled
//! at build time via `wgpu::include_wgsl!`.

use std::sync::{Arc, Mutex};

use bytemuck::{cast_slice, Pod, Zeroable};
use eframe::egui::{PaintCallbackInfo, Rect, Ui};
use egui_wgpu::CallbackTrait;
use wgpu::{
    util::DeviceExt as _, BindGroup, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, ColorTargetState, CommandEncoder, Device,
    FragmentState, PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, TextureFormat,
};

/// Load the shader module from the embedded WGSL source.
pub fn create_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::include_wgsl!("../shaders/progressbar.wgsl"))
}

/// Per-bar GPU resources, shared across frames via an Arc.
pub struct FillResources {
    uniform_buffer: Buffer,
    bind_group: BindGroup,
}

pub struct FillPipelineResources {
    pub pipeline: Option<RenderPipeline>,
    pub bind_group_layout: Option<BindGroupLayout>,
}

/// Per-fill uniform data. Layout must match the WGSL `Uniforms` struct
/// (size 64): vec4s at 0/16/32, then four f32s at 48/52/56/60.
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct FillUniforms {
    /// Expanded callback rect in physical pixels (min_x, min_y, max_x, max_y)
    pub viewport: [f32; 4],
    /// Fill rect in physical pixels (min_x, min_y, max_x, max_y)
    pub fill_rect: [f32; 4],
    /// Base fill color (RGBA)
    pub color: [f32; 4],
    /// Animation time (seconds)
    pub time: f32,
    /// Brightness multiplier
    pub intensity: f32,
    /// Pattern speed multiplier
    pub speed: f32,
    /// Padding to keep the struct 16-byte aligned
    pub _pad: f32,
}

impl FillUniforms {
    pub fn new(
        viewport: Rect,
        fill_rect: Rect,
        color: [f32; 4],
        time: f32,
        intensity: f32,
        speed: f32,
        pixels_per_point: f32,
    ) -> Self {
        Self {
            viewport: [
                viewport.min.x * pixels_per_point,
                viewport.min.y * pixels_per_point,
                viewport.max.x * pixels_per_point,
                viewport.max.y * pixels_per_point,
            ],
            fill_rect: [
                fill_rect.min.x * pixels_per_point,
                fill_rect.min.y * pixels_per_point,
                fill_rect.max.x * pixels_per_point,
                fill_rect.max.y * pixels_per_point,
            ],
            color,
            time,
            intensity,
            speed,
            _pad: 0.0,
        }
    }
}

/// Fill render callback. Creates GPU resources in `prepare`, draws in `paint`.
pub struct FillCallback {
    uniforms: FillUniforms,
    resource: Arc<Mutex<Option<FillResources>>>,
}

impl FillCallback {
    pub fn new(uniforms: FillUniforms, resource: Arc<Mutex<Option<FillResources>>>) -> Self {
        Self { uniforms, resource }
    }
}

impl CallbackTrait for FillCallback {
    fn prepare(
        &self,
        device: &Device,
        queue: &Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = callback_resources
            .entry::<FillPipelineResources>()
            .or_insert_with(|| FillPipelineResources {
                pipeline: None,
                bind_group_layout: None,
            });

        if resources.bind_group_layout.is_none() {
            let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("progressbar_fill_bg_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            core::mem::size_of::<FillUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });
            resources.bind_group_layout = Some(bgl);
        }

        if resources.pipeline.is_none() {
            let shader = create_shader_module(device);
            let bgl = resources.bind_group_layout.as_ref().unwrap();
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("progressbar_fill_pipeline_layout"),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("progressbar_fill_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format: TextureFormat::Bgra8Unorm,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    polygon_mode: PolygonMode::Fill,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

            resources.pipeline = Some(pipeline);
        }

        let bgl = resources.bind_group_layout.as_ref().unwrap();
        let mut guard = self.resource.lock().unwrap();
        match guard.as_mut() {
            Some(res) => {
                let uniform_array = [self.uniforms];
                let bytes = cast_slice(&uniform_array);
                queue.write_buffer(&res.uniform_buffer, 0, bytes);
            }
            None => {
                let uniform_array = [self.uniforms];
                let uniform_bytes = cast_slice(&uniform_array);
                let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("progressbar_fill_uniform"),
                    contents: uniform_bytes,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("progressbar_fill_bind_group"),
                    layout: bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    }],
                });
                *guard = Some(FillResources {
                    uniform_buffer,
                    bind_group,
                });
            }
        }

        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let resources = match callback_resources.get::<FillPipelineResources>() {
            Some(r) => r,
            None => return,
        };
        let pipeline = match &resources.pipeline {
            Some(p) => p,
            None => return,
        };
        let resource_guard = self.resource.lock().unwrap();
        let fill = match resource_guard.as_ref() {
            Some(b) => b,
            None => return,
        };

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &fill.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

/// Paint the liquid-lava fill for the given rect. Call once per frame;
/// the callback is attached to `ui.layer_painter`-equivalent via
/// `ui.painter().add(egui::Shape::Callback(...))`.
pub fn paint_fill(
    ui: &mut Ui,
    rect: Rect,
    color: [f32; 4],
    time: f32,
    intensity: f32,
    speed: f32,
    resource: Arc<Mutex<Option<FillResources>>>,
) {
    let pixels_per_point = ui.ctx().pixels_per_point();
    let uniforms = FillUniforms::new(rect, rect, color, time, intensity, speed, pixels_per_point);
    let callback =
        egui_wgpu::Callback::new_paint_callback(rect, FillCallback::new(uniforms, resource));
    ui.painter().add(eframe::egui::Shape::Callback(callback));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_struct_matches_wgsl_layout() {
        assert_eq!(core::mem::size_of::<FillUniforms>(), 64);
        assert_eq!(core::mem::offset_of!(FillUniforms, time), 48);
        assert_eq!(core::mem::offset_of!(FillUniforms, intensity), 52);
        assert_eq!(core::mem::offset_of!(FillUniforms, speed), 56);
    }
}
