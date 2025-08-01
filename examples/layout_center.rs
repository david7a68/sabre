use std::sync::Arc;
use std::time::Duration;

use futures::executor::block_on;
use sabre::graphics::Color;
use sabre::graphics::GraphicsContext;
use sabre::ui::UiContext;
use sabre::ui::input::InputState;
use smallvec::smallvec;
use tracing::Level;
use tracing::info;
use tracing::instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use ui_base::layout::LayoutDirection;
use ui_base::layout::Padding;
use ui_base::layout::Size::Grow;
use winit::application::ApplicationHandler;
use winit::event::ElementState;
use winit::event::MouseButton;
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
        .with(env_filter)
        .with(def_filter)
        .init();

    App::new().run();
}

struct AppWindow {
    window: Arc<Window>,

    input: InputState,
    ui_context: UiContext,
}

struct App {
    graphics: Option<GraphicsContext>,
    windows: Vec<AppWindow>,
}

impl App {
    fn new() -> Self {
        Self {
            graphics: None,
            windows: vec![],
        }
    }
}

impl App {
    fn run(mut self) {
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

        // Render to the window before showing it to avoid flashing when
        // creating the window for the first time.
        let mut canvas = graphics_context.get_canvas();
        canvas.clear(Color::BLACK);
        graphics_context
            .render(smallvec![(window.id(), canvas)])
            .unwrap();

        window.set_visible(true);

        self.windows.push(AppWindow {
            window,
            input: InputState::default(),
            ui_context: UiContext::new(),
        });
        self.graphics = Some(graphics_context);
    }

    #[instrument(skip_all)]
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.windows.clear();
        self.graphics = None;
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
            WindowEvent::Resized(size) => {
                let window = self
                    .windows
                    .iter_mut()
                    .find(|rc| rc.window.id() == window_id)
                    .unwrap();

                window.input.window_size.width = size.width as f32;
                window.input.window_size.height = size.height as f32;
            }
            WindowEvent::CloseRequested => {
                self.windows.retain(|rc| rc.window.id() != window_id);
                self.graphics.as_mut().unwrap().destroy_surface(window_id);
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } => {
                let window = self
                    .windows
                    .iter_mut()
                    .find(|rc| rc.window.id() == window_id)
                    .unwrap();

                window.input.pointer.x = position.x as f32;
                window.input.pointer.y = position.y as f32;
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                let window = self
                    .windows
                    .iter_mut()
                    .find(|rc| rc.window.id() == window_id)
                    .unwrap();

                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) => {
                        window.input.mouse_state.is_left_down = true;
                    }
                    (MouseButton::Left, ElementState::Released) => {
                        window.input.mouse_state.is_left_down = false;
                    }
                    (MouseButton::Right, ElementState::Pressed) => {
                        window.input.mouse_state.is_right_down = true;
                    }
                    (MouseButton::Right, ElementState::Released) => {
                        window.input.mouse_state.is_right_down = false;
                    }
                    (MouseButton::Middle, ElementState::Pressed) => {
                        window.input.mouse_state.is_middle_down = true;
                    }
                    (MouseButton::Middle, ElementState::Released) => {
                        window.input.mouse_state.is_middle_down = false;
                    }
                    _ => (),
                }
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

                window
                    .ui_context
                    .next_frame(window.input.clone(), Duration::ZERO, |ui| {
                        ui.with_color(Color::srgb(0.1, 0.2, 0.3, 1.0))
                            .with_element(|ui| {
                                ui.with_width(Grow);
                            })
                            .with_container(|ui| {
                                ui.with_child_direction(LayoutDirection::Vertical)
                                    .with_height(Grow)
                                    .with_element(|ui| {
                                        ui.with_height(Grow);
                                    })
                                    .with_container(|ui| {
                                        ui.with_child_spacing(10.0)
                                            .with_padding(Padding {
                                                left: 15.0,
                                                right: 15.0,
                                                top: 15.0,
                                                bottom: 15.0,
                                            })
                                            .with_color(Color::BLUE)
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(100.0)
                                                    .with_width(100.0);
                                            })
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(100.0)
                                                    .with_width(200.0);
                                            })
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(30.0)
                                                    .with_width(150.0);
                                            });
                                    })
                                    .with_container(|ui| {
                                        ui.with_child_spacing(10.0)
                                            .with_color(Color::GREEN)
                                            .with_padding(Padding {
                                                left: 15.0,
                                                right: 15.0,
                                                top: 15.0,
                                                bottom: 15.0,
                                            })
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(100.0)
                                                    .with_width(91.0);
                                            })
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(100.0)
                                                    .with_width(15.0);
                                            })
                                            .with_element(|ui| {
                                                ui.with_color(Color::WHITE)
                                                    .with_height(100.0)
                                                    .with_width(299.0);
                                            });
                                    })
                                    .with_element(|ui| {
                                        ui.with_height(Grow);
                                    });
                            })
                            .with_element(|ui| {
                                ui.with_width(Grow);
                            });
                    })
                    .finish(&mut canvas);

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
