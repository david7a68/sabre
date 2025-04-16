use std::collections::HashMap;
use std::sync::Mutex;

use log::info;
use wgpu::include_wgsl;

const SHADER: wgpu::ShaderModuleDescriptor = include_wgsl!("shader.wgsl");

#[derive(Clone)]
pub struct RenderPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::PipelineLayout,
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
    pipelines: Mutex<HashMap<wgpu::TextureFormat, RenderPipeline>>,
}

impl RenderPipelineCache {
    pub fn new(device: wgpu::Device) -> Self {
        let shader = device.create_shader_module(SHADER);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        Self {
            device,
            shader,
            layout,
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
            pipeline: render_pipeline,
            layout: self.layout.clone(),
        };

        pipelines.insert(format, pipeline.clone());

        pipeline
    }
}
