use std::sync::Arc;

use futures::executor::block_on;
use graphics::Primitive;
use graphics::TextPrimitive;
use graphics::TextStyle;
use smallvec::smallvec;
use tracing::info;
use tracing::instrument;
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

use sabre::graphics::Color;
use sabre::graphics::GraphicsContext;
use sabre::graphics::Texture;

pub struct App {
    graphics: Option<GraphicsContext>,
    windows: Vec<AppWindow>,
    texture: Option<Texture>,
    texture2: Option<Texture>,
}

impl App {
    #[expect(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            graphics: None,
            windows: vec![],
            texture: None,
            texture2: None,
        }
    }
}

impl App {
    pub fn run(mut self) {
        let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app(&mut self).unwrap();
    }
}

impl ApplicationHandler for App {
    #[instrument(skip_all)]
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

        self.texture = Some(graphics_context.load_image("scratch/test.png").unwrap());
        self.texture2 = Some(graphics_context.load_image("scratch/test2.png").unwrap());

        // Render to the window before showing it to avoid flashing when
        // creating the window for the first time.
        let mut canvas = graphics_context.get_canvas();
        canvas.clear(Color::BLACK);
        graphics_context
            .render(smallvec![(window.id(), canvas)])
            .unwrap();

        window.set_visible(true);

        self.windows.push(AppWindow { window });
        self.graphics = Some(graphics_context);
    }

    #[instrument(skip_all)]
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.windows.clear();
        self.graphics = None;
        self.texture = None;
    }

    #[instrument(skip(self, event_loop))]
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
                if event.logical_key == Key::Named(NamedKey::Space) && event.state.is_pressed() {
                    let window = self
                        .windows
                        .iter_mut()
                        .find(|rc| rc.window.id() == window_id)
                        .unwrap();

                    window.window.request_redraw();
                }

                if event.logical_key == Key::Named(NamedKey::Escape) && event.state.is_pressed() {
                    self.windows.retain(|rc| rc.window.id() != window_id);
                }
            }
            WindowEvent::CloseRequested => {
                self.windows.retain(|rc| rc.window.id() != window_id);
                self.graphics.as_mut().unwrap().destroy_surface(window_id);
            }
            WindowEvent::Destroyed => {
                if self.windows.is_empty() {
                    info!("All windows destroyed, shutting down.");
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let window = self
                    .windows
                    .iter_mut()
                    .find(|rc| rc.window.id() == window_id)
                    .unwrap();

                let graphics = self.graphics.as_mut().unwrap();
                let mut canvas = graphics.get_canvas();

                canvas.clear(Color::srgb(0.1, 0.2, 0.3, 1.0));

                canvas.draw(Primitive::new(100.0, 100.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(100.0, 200.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(100.0, 300.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(Primitive::new(100.0, 400.0, 50.0, 50.0, Color::WHITE));
                canvas.draw(
                    Primitive::new(200.0, 50.0, 400.0, 450.0, Color::WHITE)
                        .with_texture(self.texture.clone().unwrap()),
                );
                canvas.draw(
                    Primitive::new(200.0, 50.0, 300.0, 100.0, Color::WHITE)
                        .with_texture(self.texture2.clone().unwrap()),
                );
                canvas.draw_text(
                    TextPrimitive::new(
                        "Hello world!",
                        &TextStyle::default(),
                        100.0,
                        470.0,
                        Color::BLACK,
                    )
                    .with_max_width(200.),
                );

                if canvas.has_unready_textures() {
                    window.window.request_redraw();
                }

                graphics
                    .render(smallvec![(window.window.id(), canvas)])
                    .unwrap();
            }
            _ => (),
        }
    }
}

struct AppWindow {
    window: Arc<Window>,
}

use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Default)]
struct TracyConfig(tracing_subscriber::fmt::format::DefaultFields);

impl tracing_tracy::Config for TracyConfig {
    type Formatter = tracing_subscriber::fmt::format::DefaultFields;

    fn formatter(&self) -> &Self::Formatter {
        &self.0
    }

    fn stack_depth(&self, _: &tracing::metadata::Metadata<'_>) -> u16 {
        10
    }
}

fn main() {
    color_backtrace::install();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().ok();
    let def_filter = env_filter.is_none().then(|| {
        tracing_subscriber::filter::Targets::new()
            .with_default(Level::DEBUG)
            .with_targets([
                ("naga", Level::WARN),
                ("wgpu_core", Level::WARN),
                ("wgpu_hal", Level::WARN),
                ("wgpu", Level::WARN),
            ])
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(tracing_tracy::TracyLayer::new(TracyConfig::default()))
        .with(env_filter)
        .with(def_filter)
        .init();

    App::new().run();
}
