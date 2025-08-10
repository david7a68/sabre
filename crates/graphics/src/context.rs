use std::path::Path;
use std::sync::Arc;

use futures::executor::block_on;
use smallvec::SmallVec;
use tracing::info;
use tracing::instrument;
use tracing::warn;
use winit::window::Window;
use winit::window::WindowId;
use workspace_hack as _;

use crate::Canvas;
use crate::Texture;
use crate::TextureLoadError;
use crate::draw::CanvasStorage;
use crate::glyph_cache::GlyphCache;
use crate::pipeline::RenderPipelineCache;
use crate::surface::RenderError;
use crate::surface::Surface;
use crate::texture::TextureManager;

pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    windows: Vec<Surface>,
    textures: TextureManager,
    glyph_cache: GlyphCache,

    render_pipelines: Arc<RenderPipelineCache>,
}

impl GraphicsContext {
    #[instrument(skip(window))]
    pub fn new(window: Arc<Window>) -> Self {
        info!("Creating graphics context");

        let mut flags = wgpu::InstanceFlags::empty();

        if cfg!(debug_assertions) {
            info!("Creating graphics context with debug and validation layers enabled");
            flags |= wgpu::InstanceFlags::DEBUG;
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            flags,
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    // shader_compiler: wgpu::Dx12Compiler::StaticDxc,
                    shader_compiler: wgpu::Dx12Compiler::Fxc,
                },
                ..Default::default()
            },
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(async {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                })
                .await
        })
        .unwrap();

        info!("Adapter: {:?}", adapter.get_info());

        let (device, queue) = block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                })
                .await
        })
        .unwrap();

        let render_pipelines = Arc::new(RenderPipelineCache::new(device.clone()));

        let windows = vec![Surface::new(
            window,
            surface,
            &device,
            &adapter,
            &render_pipelines,
        )];

        let textures = TextureManager::new(queue.clone(), device.clone());
        let glyph_cache = GlyphCache::new();

        Self {
            instance,
            adapter,
            device,
            queue,

            windows,
            textures,
            glyph_cache,

            render_pipelines,
        }
    }

    #[instrument(skip(self))]
    pub fn init_surface(&mut self, window: Arc<Window>) {
        let surface = self.instance.create_surface(window.clone()).unwrap();
        self.windows.push(Surface::new(
            window,
            surface,
            &self.device,
            &self.adapter,
            &self.render_pipelines,
        ));
    }

    #[instrument(skip(self))]
    pub fn destroy_surface(&mut self, window_id: WindowId) {
        if let Some(index) = self.windows.iter().position(|w| w.window_id() == window_id) {
            self.windows.remove(index);
        } else {
            warn!("Window not found, skipping destroy.");
        }
    }

    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn load_image(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.textures.load(path)
    }

    #[instrument(skip(self))]
    pub fn create_canvas(&mut self) -> Canvas {
        Canvas::new(
            CanvasStorage::default(),
            self.glyph_cache.clone(),
            self.textures.clone(),
        )
    }

    #[instrument(skip(self, targets))]
    pub fn render(
        &mut self,
        targets: SmallVec<[(WindowId, &Canvas); 2]>,
    ) -> Result<(), RenderError> {
        let mut command_buffers = SmallVec::<[_; 2]>::new();
        let mut presents = SmallVec::<[_; 2]>::new();

        self.textures.flush();

        for (window_id, canvas) in targets {
            let canvas = canvas.storage();

            let Some(window) = self.windows.iter_mut().find(|w| w.window_id() == window_id) else {
                warn!("Window not found, skipping render.");
                continue;
            };

            window.resize_if_necessary(&self.device);

            let (target, command_buffer) =
                window.write_commands(&self.queue, &self.device, canvas)?;
            command_buffers.push(command_buffer);
            presents.push((window_id, target));
        }

        tracing::info_span!("submit").in_scope(|| {
            self.queue.submit(command_buffers);
        });

        tracing::info_span!("present").in_scope(|| {
            for (window_id, target) in presents {
                let Some(window) = self.windows.iter_mut().find(|w| w.window_id() == window_id)
                else {
                    warn!("Window not found, skipping render.");
                    continue;
                };

                window.pre_present_notify();
                target.present();
            }
        });

        self.textures.end_frame();

        #[cfg(feature = "profile")]
        {
            tracing_tracy::client::frame_mark();
        }

        Ok(())
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        info!("Dropping graphics context");

        self.textures.end_frame();
    }
}
