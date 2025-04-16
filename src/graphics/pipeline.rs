use std::collections::HashMap;
use std::sync::Mutex;

use bytemuck::Pod;
use bytemuck::Zeroable;
use log::info;
use wgpu::include_wgsl;

const SHADER: wgpu::ShaderModuleDescriptor = include_wgsl!("shader.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawInfo {
    pub viewport_size: [u32; 2],
}

pub struct DrawInfoUniforms {
    buffer: wgpu::Buffer,
    binding: wgpu::BindGroup,
}

impl DrawInfoUniforms {
    fn new(device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Draw Info Uniforms"),
            size: std::mem::size_of::<DrawInfo>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
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

        Self { buffer, binding }
    }

    pub fn bind_and_update(
        &self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        draw_info: DrawInfo,
    ) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[draw_info]));
        render_pass.set_bind_group(0, &self.binding, &[]);
    }
}

#[derive(Clone)]
pub struct RenderPipeline {
    pub device: wgpu::Device,
    pub pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::PipelineLayout,
    pub draw_info_bind_group_layout: wgpu::BindGroupLayout,
}

impl RenderPipeline {
    pub fn create_draw_info_uniforms(&self) -> DrawInfoUniforms {
        DrawInfoUniforms::new(&self.device, &self.draw_info_bind_group_layout)
    }
}

/// A cache for render pipelines.
///
/// This is used because different monitors may have different preffered color
/// formats and we want to accommodate that. The cache allows us to share render
/// pipelines between windows in that case, instead of creating a new pipeline
/// for each window.
///
/// There is no mechanism to invalidate the cache, under the assumption that
/// there are a fixed number of formats that can be used.
pub(super) struct RenderPipelineCache {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,

    draw_info_bind_group_layout: wgpu::BindGroupLayout,

    pipelines: Mutex<HashMap<wgpu::TextureFormat, RenderPipeline>>,
}

impl RenderPipelineCache {
    pub fn new(device: wgpu::Device) -> Self {
        let shader = device.create_shader_module(SHADER);

        let draw_info_bind_group_layout =
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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&draw_info_bind_group_layout],
            push_constant_ranges: &[],
        });

        Self {
            device,
            shader,
            layout,
            draw_info_bind_group_layout,
            pipelines: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_pipeline(&self, format: wgpu::TextureFormat) -> RenderPipeline {
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
            layout: self.layout.clone(),
            draw_info_bind_group_layout: self.draw_info_bind_group_layout.clone(),
        };

        pipelines.insert(format, pipeline.clone());

        pipeline
    }
}
