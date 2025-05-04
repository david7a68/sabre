use std::collections::HashMap;
use std::sync::Arc;

use tracing::debug;
use tracing::instrument;
use tracing::trace;
use winit::window::Window;
use winit::window::WindowId;

use crate::Canvas;
use crate::draw::DrawCommand;
use crate::pipeline::DrawBuffer;
use crate::pipeline::DrawUniforms;
use crate::pipeline::RenderPipeline;
use crate::pipeline::RenderPipelineCache;
use crate::texture::StorageId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderError {
    OutOfMemory,
    TimedOut,
    Unknown,
}

pub struct Surface {
    window: Arc<Window>,
    config: wgpu::SurfaceConfiguration,
    handle: wgpu::Surface<'static>,

    frame_counter: u64,
    render_pipeline: RenderPipeline,
    frames_in_flight: [Frame; 2],
}

impl Surface {
    #[instrument(skip_all)]
    pub(crate) fn new(
        window: Arc<Window>,
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
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(device, &config);

        let render_pipeline = pipeline_cache.get(format);

        let frames_in_flight = [Frame::new(&render_pipeline), Frame::new(&render_pipeline)];

        Self {
            window,
            config,
            handle: surface,
            frame_counter: 0,
            render_pipeline,
            frames_in_flight,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    #[instrument(skip(self, device))]
    pub fn resize_if_necessary(&mut self, device: &wgpu::Device) {
        let new_size = self.window.inner_size();

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

    pub(crate) fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    #[instrument(
        skip_all,
        fields(
            frame_id = self.frame_counter,
            num_primitives = canvas.primitives().len(),
            num_commands = canvas.commands().len()
        )
    )]
    pub(crate) fn write_commands(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        canvas: &Canvas,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::CommandBuffer), RenderError> {
        let output = tracing::info_span!("get_current_texture").in_scope(|| {
            let mut attempts = 0;

            let mut output = self.handle.get_current_texture();

            loop {
                if attempts > 3 {
                    break Err(RenderError::TimedOut);
                }

                match output {
                    Ok(output) => break Ok(output),
                    Err(e) => match e {
                        wgpu::SurfaceError::Timeout => break Err(RenderError::TimedOut),
                        wgpu::SurfaceError::OutOfMemory => break Err(RenderError::OutOfMemory),
                        wgpu::SurfaceError::Other => break Err(RenderError::Unknown),
                        wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost => {
                            self.resize_if_necessary(device);
                            output = self.handle.get_current_texture();
                        }
                    },
                }

                attempts += 1;
            }
        })?;

        let frame = &mut self.frames_in_flight[self.frame_counter as usize % 2];

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let load_op = if let Some(clear_color) = canvas.clear_color() {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear_color.r.into(),
                g: clear_color.g.into(),
                b: clear_color.b.into(),
                a: clear_color.a.into(),
            })
        } else {
            wgpu::LoadOp::Load
        };

        tracing::info_span!("render_pass").in_scope(|| {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline.pipeline);

            frame.draw_buffer.upload_and_bind(
                device,
                queue,
                &mut render_pass,
                DrawUniforms {
                    viewport_size: [self.config.width, self.config.height],
                },
                canvas.primitives(),
            );

            let mut vertex_offset = 0;
            let mut bind_groups = HashMap::<(StorageId, StorageId), wgpu::BindGroup>::new();

            tracing::info!(primitives = ?canvas.primitives());

            for command in canvas.commands() {
                match command {
                    DrawCommand::Draw {
                        color_texture_id,
                        alpha_texture_id,
                        num_vertices,
                    } => {
                        let Some(color_texture) = canvas.texture(*color_texture_id) else {
                            debug!(
                                texture_id = ?color_texture_id,
                                "Color texture not found, skipping primitives"
                            );
                            continue;
                        };

                        let Some(alpha_texture) = canvas.texture(*alpha_texture_id) else {
                            debug!(
                                texture_id = ?alpha_texture_id,
                                "Alpha texture not found, skipping primitives"
                            );
                            continue;
                        };

                        let is_ready = color_texture.is_ready() && alpha_texture.is_ready();
                        if !is_ready {
                            debug!(
                                skipped_vertices = num_vertices,
                                "Texture {color_texture_id:?} not ready, skipping primitives"
                            );

                            vertex_offset += *num_vertices;
                            continue;
                        }

                        // Draw the previous batch of vertices and start a new one.
                        tracing::info!(
                            color_texture_id = ?color_texture_id,
                            alpha_texture_id = ?alpha_texture_id,
                            "batch!"
                        );

                        let bind_group = bind_groups
                            .entry((color_texture.storage_id(), alpha_texture.storage_id()))
                            .or_insert_with(|| {
                                self.render_pipeline.create_texure_bind_group(
                                    color_texture.texture_view(),
                                    alpha_texture.texture_view(),
                                )
                            });

                        self.render_pipeline
                            .bind_texture(&mut render_pass, bind_group);

                        render_pass.draw(vertex_offset..vertex_offset + *num_vertices, 0..1);
                        vertex_offset += *num_vertices;
                    }
                }
            }
        });

        self.frame_counter += 1;

        Ok((output, encoder.finish()))
    }
}

struct Frame {
    draw_buffer: DrawBuffer,
}

impl Frame {
    fn new(render_pipeline: &RenderPipeline) -> Self {
        Self {
            draw_buffer: render_pipeline.create_duffer(),
        }
    }
}
