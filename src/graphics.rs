use std::sync::Arc;

use log::info;
use winit::window::Window;

pub use crate::graphics::pipeline::RenderPipeline;
use crate::graphics::pipeline::RenderPipelineCache;
use crate::window::WindowState;

mod pipeline;

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
