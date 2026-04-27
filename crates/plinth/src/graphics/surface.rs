use std::collections::HashMap;
use std::sync::Arc;

use tracing::instrument;
use tracing::trace;
use winit::window::Window;
use winit::window::WindowId;

use crate::graphics::pipeline::DrawBuffer;
use crate::graphics::pipeline::RenderPipeline;
use crate::graphics::pipeline::RenderPipelineCache;
use crate::graphics::texture::StorageId;

type BindGroupCache = HashMap<(StorageId, StorageId), wgpu::BindGroup>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    Occluded,
    TimedOut,
    Unknown,
}

pub(crate) struct Surface {
    window: Arc<dyn Window>,
    config: wgpu::SurfaceConfiguration,
    handle: wgpu::Surface<'static>,

    frame_counter: u64,
    render_pipeline: RenderPipeline,
    frame: Frame,

    bind_groups: BindGroupCache,
    cached_storage_version: u64,
}

impl Surface {
    #[instrument(skip_all)]
    pub fn new(
        window: Arc<dyn Window>,
        surface: wgpu::Surface<'static>,
        device: &wgpu::Device,
        adapter: &wgpu::Adapter,
        pipeline_cache: &RenderPipelineCache,
    ) -> Self {
        let caps = surface.get_capabilities(adapter);

        let format = caps
            .formats
            .first()
            .copied()
            .expect("Surface incompatible with selected adapter!");

        let present_mode = {
            let mut mailbox = None;
            let mut relaxed = None;
            let mut fifo = None;

            for mode in caps.present_modes.iter().copied() {
                match mode {
                    wgpu::PresentMode::Mailbox => mailbox = Some(mode),
                    wgpu::PresentMode::FifoRelaxed => relaxed = Some(mode),
                    wgpu::PresentMode::Fifo => fifo = Some(mode),
                    _ => {}
                }
            }

            mailbox
                .or(relaxed)
                .or(fifo)
                .unwrap_or(caps.present_modes[0])
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: window.surface_size().width,
            height: window.surface_size().height,
            present_mode,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(device, &config);

        let render_pipeline = pipeline_cache.get(format);

        let frame = Frame::new(&render_pipeline);

        Self {
            window,
            config,
            handle: surface,
            frame_counter: 0,
            render_pipeline,
            frame,
            bind_groups: HashMap::new(),
            cached_storage_version: 0,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    #[instrument(skip(self, device))]
    pub fn resize_if_necessary(&mut self, device: &wgpu::Device) {
        let new_size = self.window.surface_size();

        if self.config.width == new_size.width && self.config.height == new_size.height {
            return;
        }

        if new_size.width > 0 && new_size.height > 0 {
            trace!("Recreating lost or outdated surface. New size: {new_size:?}",);

            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.handle.configure(device, &self.config);
        }
    }

    pub fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    pub fn frame_counter(&self) -> u64 {
        self.frame_counter
    }

    pub fn next_frame(
        &mut self,
        device: &wgpu::Device,
        storage_version: u64,
    ) -> Result<(wgpu::SurfaceTexture, &mut Frame, &RenderPipeline, &mut BindGroupCache), RenderError> {
        let output = tracing::info_span!("get_current_texture").in_scope(|| {
            let mut attempts = 0;

            let mut output = self.handle.get_current_texture();

            loop {
                if attempts > 3 {
                    break Err(RenderError::TimedOut);
                }

                match output {
                    wgpu::CurrentSurfaceTexture::Success(surface_texture) => {
                        return Ok(surface_texture);
                    }
                    wgpu::CurrentSurfaceTexture::Suboptimal(_)
                    | wgpu::CurrentSurfaceTexture::Outdated => {
                        self.resize_if_necessary(device);
                        output = self.handle.get_current_texture()
                    }
                    wgpu::CurrentSurfaceTexture::Timeout => break Err(RenderError::TimedOut),
                    wgpu::CurrentSurfaceTexture::Occluded => break Err(RenderError::Occluded),
                    wgpu::CurrentSurfaceTexture::Lost => {
                        unimplemented!("Surface lost handling not implemented yet")
                    }
                    wgpu::CurrentSurfaceTexture::Validation => break Err(RenderError::Unknown),
                }

                attempts += 1;
            }
        })?;

        if storage_version != self.cached_storage_version {
            self.bind_groups.clear();
            self.cached_storage_version = storage_version;
        }

        self.frame_counter += 1;

        Ok((
            output,
            &mut self.frame,
            &self.render_pipeline,
            &mut self.bind_groups,
        ))
    }
}

pub struct Frame {
    pub draw_buffer: DrawBuffer,
}

impl Frame {
    fn new(render_pipeline: &RenderPipeline) -> Self {
        Self {
            draw_buffer: render_pipeline.create_draw_buffer(),
        }
    }
}
