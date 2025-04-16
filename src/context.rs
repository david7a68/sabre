use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use log::info;
use wgpu::include_wgsl;
use winit::window::Window;

use crate::window::WindowState;

const SHADER: wgpu::ShaderModuleDescriptor = include_wgsl!("shader.wgsl");

pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    render_pipelines: Arc<RenderPipelineCache>,
}

impl GraphicsContext {
    pub async fn new(window: Arc<Window>) -> (WindowState, Self) {
        info!("Creating graphics context");

        let mut flags = wgpu::InstanceFlags::empty();

        if cfg!(debug_assertions) {
            flags |= wgpu::InstanceFlags::DEBUG;
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            flags,
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    shader_compiler: wgpu::Dx12Compiler::StaticDxc,
                },
                ..Default::default()
            },
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        info!("Adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let render_pipelines = Arc::new(RenderPipelineCache::new(device.clone()));

        let this = Self {
            instance,
            adapter,
            device,
            queue,

            render_pipelines,
        };

        let rcx = WindowState::new(&this, window, surface);

        (rcx, this)
    }

    pub fn create_window(&self, window: Arc<Window>) -> WindowState {
        let surface = self.instance.create_surface(window.clone()).unwrap();
        WindowState::new(self, window, surface)
    }

    pub fn get_render_pipeline(&self, render_target_format: wgpu::TextureFormat) -> RenderPipeline {
        self.render_pipelines.get_pipeline(render_target_format)
    }
}

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
struct RenderPipelineCache {
    device: wgpu::Device,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    pipelines: Mutex<HashMap<wgpu::TextureFormat, RenderPipeline>>,
}

impl RenderPipelineCache {
    fn new(device: wgpu::Device) -> Self {
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

    fn get_pipeline(&self, format: wgpu::TextureFormat) -> RenderPipeline {
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
