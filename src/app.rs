use std::sync::Arc;

use futures::executor::block_on;
use log::warn;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::Window;
use winit::window::WindowId;

use crate::color::Color;
use crate::graphics::Canvas;
use crate::graphics::GraphicsContext;
use crate::graphics::Primitive;
use crate::window::RenderError;
use crate::window::WindowState;

pub struct App {
    canvas: Canvas,
    context: Option<GraphicsContext>,
    render_contexts: Vec<WindowState>,
}

impl App {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            canvas: Canvas::new(),
            context: None,
            render_contexts: vec![],
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_no_redirection_bitmap(false)
                        .with_visible(false),
                )
                .unwrap(),
        );

        let (mut render_context, graphics_context) =
            block_on(async { GraphicsContext::new(window.clone()).await });

        // Render to the window before showing it to avoid flashing when
        // creating the window for the first time.
        self.canvas.begin(Color::BLACK);
        render_context.render(&self.canvas).unwrap();
        render_context.set_visible(true);

        self.render_contexts.push(render_context);
        self.context = Some(graphics_context);
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.render_contexts.clear();
        self.context = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                if event.logical_key == Key::Named(NamedKey::Escape) && event.state.is_pressed() {
                    self.render_contexts
                        .retain(|rc| rc.window_id() != window_id);
                }
            }
            WindowEvent::Resized(physical_size) => {
                let window = self
                    .render_contexts
                    .iter_mut()
                    .find(|rc| rc.window_id() == window_id)
                    .unwrap();

                window.resize(physical_size);
            }
            WindowEvent::CloseRequested => {
                self.render_contexts
                    .retain(|rc| rc.window_id() != window_id);
            }
            WindowEvent::Destroyed => {
                if self.render_contexts.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let window = self
                    .render_contexts
                    .iter_mut()
                    .find(|rc| rc.window_id() == window_id)
                    .unwrap();

                self.canvas.begin(Color::srgb(0.1, 0.2, 0.3, 1.0));

                self.canvas
                    .draw(Primitive::new(100.0, 100.0, 50.0, 50.0, Color::WHITE));
                self.canvas
                    .draw(Primitive::new(200.0, 200.0, 50.0, 50.0, Color::WHITE));
                self.canvas
                    .draw(Primitive::new(300.0, 300.0, 50.0, 50.0, Color::WHITE));
                self.canvas
                    .draw(Primitive::new(400.0, 400.0, 50.0, 50.0, Color::WHITE));

                match window.render(&self.canvas) {
                    Ok(_) => {}
                    Err(e) => match e {
                        RenderError::SurfaceOutOfMemory => {
                            self.render_contexts.clear();
                            event_loop.exit();
                        }
                        RenderError::SurfaceUnknownError => {
                            self.render_contexts.clear();
                            event_loop.exit();
                        }
                        RenderError::SurfaceTimedOut => {
                            warn!("Surface timed out, something went wrong.");
                        }
                    },
                }

                tracy::frame!();
            }
            _ => (),
        }
    }
}
