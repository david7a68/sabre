use std::sync::Arc;
use std::time::Duration;

use futures::executor::block_on;
use smallvec::smallvec;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::Window;
use winit::window::WindowId;

use crate::color::Color;
use crate::graphics::GraphicsContext;
use crate::graphics::Primitive;

pub struct App {
    graphics: Option<GraphicsContext>,
    windows: Vec<Arc<Window>>,
}

impl App {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            graphics: None,
            windows: vec![],
        }
    }
}

impl App {
    pub fn run(mut self) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app(&mut self).unwrap();

        rt.shutdown_timeout(Duration::from_secs(1));
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

        let mut graphics_context = block_on(async { GraphicsContext::new(window.clone()).await });

        graphics_context.load_image("test.png").unwrap();

        // Render to the window before showing it to avoid flashing when
        // creating the window for the first time.
        let mut canvas = graphics_context.get_canvas();
        canvas.begin(Color::BLACK);
        graphics_context
            .render(smallvec![(window.id(), canvas)])
            .unwrap();

        window.set_visible(true);

        self.windows.push(window);
        self.graphics = Some(graphics_context);
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.windows.clear();
        self.graphics = None;
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
                    self.windows.retain(|rc| rc.id() != window_id);
                }
            }
            WindowEvent::Resized(_) => {}
            WindowEvent::CloseRequested => {
                self.windows.retain(|rc| rc.id() != window_id);
                self.graphics.as_mut().unwrap().destroy_surface(window_id);
            }
            WindowEvent::Destroyed => {
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let window = self
                    .windows
                    .iter_mut()
                    .find(|rc| rc.id() == window_id)
                    .unwrap();

                let graphics = self.graphics.as_mut().unwrap();
                let mut canvas = graphics.get_canvas();

                canvas.begin(Color::srgb(0.1, 0.2, 0.3, 1.0));

                canvas.draw(Primitive::new(100.0, 100.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(200.0, 200.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(300.0, 300.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(400.0, 400.0, 50.0, 50.0, Color::WHITE));

                graphics.render(smallvec![(window.id(), canvas)]).unwrap();
            }
            _ => (),
        }
    }
}
