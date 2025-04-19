use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Mutex;

use bytemuck::NoUninit;
use bytemuck::Pod;
use bytemuck::Zeroable;
use tracing::info;
use wgpu::include_wgsl;

use crate::graphics::draw::GpuPrimitive;

use super::texture_manager::StoredTexture;

const SHADER: wgpu::ShaderModuleDescriptor = include_wgsl!("shader.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawInfo {
    pub viewport_size: [u32; 2],
}

pub struct DrawBuffer<T: ?Sized> {
    buffer: wgpu::Buffer,
    binding: wgpu::BindGroup,
    bind_group: u32,
    phantom: PhantomData<fn(&T) -> ()>,
}

impl<T: ?Sized> DrawBuffer<T> {
    fn new(
        device: &wgpu::Device,
        usage: wgpu::BufferUsages,
        size: u64,
        bind_group: u32,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Draw Info Uniforms"),
            size,
            usage,
            mapped_at_creation: false,
        });

        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Draw Info Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            binding,
            bind_group,
            phantom: PhantomData,
        }
    }
}

impl<T: NoUninit> DrawBuffer<T> {
    pub fn bind_and_update(
        &self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        contents: &T,
    ) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(contents));
        render_pass.set_bind_group(self.bind_group, &self.binding, &[]);
    }
}

impl<T: NoUninit> DrawBuffer<[T]> {
    pub fn ensure_capacity(&mut self, device: &wgpu::Device, capacity: u64) {
        if self.buffer.size() >= capacity * std::mem::size_of::<T>() as u64 {
            return;
        }

        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Draw Info Uniforms"),
            size: capacity * std::mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
    }

    pub fn bind_and_update(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass,
        contents: &[T],
    ) {
        let contents = bytemuck::cast_slice(contents);

        self.ensure_capacity(device, contents.len() as u64);

        queue.write_buffer(&self.buffer, 0, contents);
        render_pass.set_bind_group(self.bind_group, &self.binding, &[]);
    }
}

#[derive(Clone)]
pub(crate) struct RenderPipeline {
    pub device: wgpu::Device,
    pub pipeline: wgpu::RenderPipeline,
    pub sampler_bind_group: wgpu::BindGroup,
    pub draw_info_bind_group_layout: wgpu::BindGroupLayout,
    pub primitive_bind_group_layout: wgpu::BindGroupLayout,
}

impl RenderPipeline {
    pub fn create_draw_info_uniforms(&self) -> DrawBuffer<DrawInfo> {
        DrawBuffer::new(
            &self.device,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            std::mem::size_of::<DrawInfo>() as u64,
            0,
            &self.draw_info_bind_group_layout,
        )
    }

    pub fn create_primitive_buffer(&self) -> DrawBuffer<[GpuPrimitive]> {
        DrawBuffer::new(
            &self.device,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            std::mem::size_of::<GpuPrimitive>() as u64 * 1024,
            1,
            &self.primitive_bind_group_layout,
        )
    }

    pub fn bind_texture(&self, render_pass: &mut wgpu::RenderPass, texture: &StoredTexture) {
        render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);
        render_pass.set_bind_group(3, &texture.bind_group, &[]);
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
    sampler_bind_group: wgpu::BindGroup,

    content_bind_group_layout: wgpu::BindGroupLayout,
    // sampler_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    viewport_bind_group_layout: wgpu::BindGroupLayout,

    pipelines: Mutex<HashMap<wgpu::TextureFormat, RenderPipeline>>,
}

impl RenderPipelineCache {
    pub fn new(device: wgpu::Device) -> Self {
        let shader = device.create_shader_module(SHADER);

        let viewport_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Draw Info Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let content_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Content Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Sampler Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &viewport_bind_group_layout,
                &content_bind_group_layout,
                &sampler_bind_group_layout,
                &texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
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

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sampler Bind Group"),
            layout: &sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
            }],
        });

        Self {
            device,
            shader,
            layout,
            diffuse_sampler,
            sampler_bind_group,
            content_bind_group_layout,
            // sampler_bind_group_layout,
            texture_bind_group_layout,
            viewport_bind_group_layout,
            pipelines: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, format: wgpu::TextureFormat) -> RenderPipeline {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline) = pipelines.get(&format) {
            info!("Found a cached pipeline for {:?}", format);
            return pipeline.clone();
        }

        info!("Creating a new pipeline for {:?}", format);

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
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            });

        let pipeline = RenderPipeline {
            device: self.device.clone(),
            pipeline: render_pipeline,
            sampler_bind_group: self.sampler_bind_group.clone(),
            draw_info_bind_group_layout: self.viewport_bind_group_layout.clone(),
            primitive_bind_group_layout: self.content_bind_group_layout.clone(),
        };

        pipelines.insert(format, pipeline.clone());

        pipeline
    }

    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }
}
