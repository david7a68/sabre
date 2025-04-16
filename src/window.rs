use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::window::Window;
use winit::window::WindowId;

use crate::graphics::DrawBuffer;
use crate::graphics::DrawInfo;
use crate::graphics::GraphicsContext;
use crate::graphics::Primitive;
use crate::graphics::RenderPipeline;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    SurfaceOutOfMemory,
    SurfaceUnknownError,
    SurfaceTimedOut,
}

pub struct WindowState {
    window: Arc<Window>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    frame_counter: u64,
    render_pipeline: RenderPipeline,
    frames_in_flight: [Frame; 2],
}

impl WindowState {
    pub(crate) fn new(
        context: &GraphicsContext,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
    ) -> Self {
        let caps = surface.get_capabilities(&context.adapter);

        let format = caps
            .formats
            .first()
            .cloned()
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
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&context.device, &config);

        let render_pipeline = context.get_render_pipeline(format);

        let frames_in_flight = [Frame::new(&render_pipeline), Frame::new(&render_pipeline)];

        Self {
            queue: context.queue.clone(),
            device: context.device.clone(),
            window,
            surface,
            surface_config: config,
            frame_counter: 0,
            render_pipeline,
            frames_in_flight,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.window.set_visible(visible);
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn render(&mut self) -> Result<(), RenderError> {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(e) => match e {
                wgpu::SurfaceError::Timeout => return Err(RenderError::SurfaceTimedOut),
                wgpu::SurfaceError::OutOfMemory => return Err(RenderError::SurfaceOutOfMemory),
                wgpu::SurfaceError::Other => return Err(RenderError::SurfaceUnknownError),
                wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost => {
                    self.resize(self.window.inner_size());
                    self.surface.get_current_texture().unwrap()
                }
            },
        };

        let frame = &mut self.frames_in_flight[self.frame_counter as usize % 2];

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline.pipeline);

            frame.draw_uniforms.bind_and_update(
                &self.queue,
                &mut render_pass,
                &DrawInfo {
                    viewport_size: [self.surface_config.width, self.surface_config.height],
                },
            );

            frame
                .primitives
                .bind_and_update(&self.queue, &mut render_pass, &[]);

            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);

        self.window.pre_present_notify();
        output.present();

        self.frame_counter += 1;
        Ok(())
    }
}

struct Frame {
    draw_uniforms: DrawBuffer<DrawInfo>,
    primitives: DrawBuffer<[Primitive]>,
}

impl Frame {
    fn new(render_pipeline: &RenderPipeline) -> Self {
        let draw_uniforms = render_pipeline.create_draw_info_uniforms();
        let primitives = render_pipeline.create_primitive_buffer();

        Self {
            draw_uniforms,
            primitives,
        }
    }
}
