use std::sync::Arc;

use winit::window::Window;

use crate::window::WindowState;

pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GraphicsContext {
    pub async fn new(window: Arc<Window>) -> (WindowState, Self) {
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

        let this = Self {
            instance,
            adapter,
            device,
            queue,
        };

        let rcx = WindowState::new(&this, window, surface);

        (rcx, this)
    }

    pub fn create_window(&self, window: Arc<Window>) -> WindowState {
        let surface = self.instance.create_surface(window.clone()).unwrap();
        WindowState::new(self, window, surface)
    }
}
