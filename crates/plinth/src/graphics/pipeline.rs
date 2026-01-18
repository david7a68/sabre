use std::collections::HashMap;
use std::sync::Mutex;

use bytemuck::Pod;
use bytemuck::Zeroable;
use tracing::debug;

use crate::graphics::Color;

const SHADER_SOURCE: &str = include_str!("shader.wgsl");

#[derive(Clone)]
pub(crate) struct RenderPipeline {
    pub device: wgpu::Device,
    pub pipeline: wgpu::RenderPipeline,
    pub sampler_bind_group: wgpu::BindGroup,
    pub draw_data_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl RenderPipeline {
    pub fn create_texure_bind_group(
        &self,
        color_texture: &wgpu::TextureView,
        alpha_texture: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(alpha_texture),
                },
            ],
        })
    }

    pub fn create_duffer(&self) -> DrawBuffer {
        DrawBuffer::new(&self.device, &self.draw_data_layout, 1024)
    }

    pub fn bind_texture(
        &self,
        render_pass: &mut wgpu::RenderPass,
        texture_bind_group: &wgpu::BindGroup,
    ) {
        render_pass.set_bind_group(1, &self.sampler_bind_group, &[]);
        render_pass.set_bind_group(2, texture_bind_group, &[]);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawUniforms {
    pub viewport_size: [u32; 2],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPrimitive {
    pub point: [f32; 2],
    pub extent: [f32; 2],
    pub background: GpuPaint,
    pub control_flags: PrimitiveRenderFlags,
    pub _padding0: u32,
    pub _padding1: u32,
    pub _padding2: u32,
}

/// A union type representing either a sampled texture paint or a gradient paint.
/// The interpretation depends on the `USE_GRADIENT_PAINT` flag in `PrimitiveRenderFlags`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct GpuPaint {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
}

impl GpuPaint {
    /// Create a sampled texture paint.
    pub fn sampled(color_tint: Color, color_uvwh: [f32; 4], alpha_uvwh: [f32; 4]) -> Self {
        Self {
            a: color_tint.into(),
            b: color_uvwh,
            c: alpha_uvwh,
        }
    }

    /// Create a gradient paint.
    /// `p1` and `p2` are normalized coordinates (0.0-1.0) within the rect.
    pub fn gradient(color_a: Color, color_b: Color, p1: [f32; 2], p2: [f32; 2]) -> Self {
        Self {
            a: color_a.into(),
            b: color_b.into(),
            c: [p1[0], p1[1], p2[0], p2[1]],
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
    #[repr(transparent)]
    pub struct PrimitiveRenderFlags: u32 {
        const USE_NEAREST_SAMPLING = 1;
        const USE_GRADIENT_PAINT = 2;
    }
}

pub struct DrawBuffer {
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    primitive_buffer: wgpu::Buffer,
}

impl DrawBuffer {
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        prim_capacity: usize,
    ) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<DrawUniforms>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let primitive_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Primitive Buffer"),
            size: (std::mem::size_of::<GpuPrimitive>() * prim_capacity) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bindings"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: primitive_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            bind_group,
            uniform_buffer,
            primitive_buffer,
        }
    }

    pub fn upload_and_bind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        draw_info: DrawUniforms,
        primitives: &[GpuPrimitive],
    ) {
        let required_size = std::mem::size_of_val(primitives) as u64;
        if self.primitive_buffer.size() < required_size {
            self.primitive_buffer.destroy();

            self.primitive_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Draw Info Uniforms"),
                size: required_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&draw_info));
        queue.write_buffer(&self.primitive_buffer, 0, bytemuck::cast_slice(primitives));

        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }
}

/// A cache for render pipelines.
///
/// This is used because different monitors may have different preferred color
/// formats and we want to accommodate that. The cache allows us to share render
/// pipelines between windows in that case, instead of creating a new pipeline
/// for each window.
///
/// There is no mechanism to invalidate the cache, under the assumption that
/// there are a fixed number of formats that can be used.
pub(crate) struct RenderPipelineCache {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,

    #[expect(unused)]
    diffuse_sampler: wgpu::Sampler,
    #[expect(unused)]
    nearest_sampler: wgpu::Sampler,

    sampler_bind_group: wgpu::BindGroup,

    draw_data_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    pipelines: Mutex<HashMap<wgpu::TextureFormat, RenderPipeline>>,
}

impl RenderPipelineCache {
    pub fn new(device: wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        let draw_data_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Draw Info Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Sampler Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &draw_data_layout,
                &sampler_bind_group_layout,
                &texture_bind_group_layout,
            ],
            immediate_size: 0,
        });

        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Diffuse Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Diffuse Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sampler Bind Group"),
            layout: &sampler_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&nearest_sampler),
                },
            ],
        });

        Self {
            device,
            shader,
            layout,
            diffuse_sampler,
            nearest_sampler,
            sampler_bind_group,
            draw_data_layout,
            texture_bind_group_layout,
            pipelines: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, format: wgpu::TextureFormat) -> RenderPipeline {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.get(&format) {
            debug!("Found a cached pipeline for {:?}", format);
            return pipeline.clone();
        }

        debug!("Creating a new pipeline for {:?}", format);

        let render_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        let pipeline = RenderPipeline {
            device: self.device.clone(),
            pipeline: render_pipeline,
            sampler_bind_group: self.sampler_bind_group.clone(),
            draw_data_layout: self.draw_data_layout.clone(),
            texture_bind_group_layout: self.texture_bind_group_layout.clone(),
        };

        pipelines.insert(format, pipeline.clone());

        pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_primitive_size() {
        // Expected: 2*4 + 2*4 + 4*4 + 4*4 + 4*4 + 4 + 4 + 4 + 4 = 80 bytes
        assert_eq!(std::mem::size_of::<GpuPrimitive>(), 80);
        assert_eq!(std::mem::align_of::<GpuPrimitive>(), 16);
    }
}
