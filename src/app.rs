use std::sync::Arc;
use std::time::Duration;

use futures::executor::block_on;
use smallvec::smallvec;
use tracing::info;
use tracing::instrument;
use ui_base::input::InputState;
use ui_base::layout::Padding;
use ui_base::ui::UiContext;
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

use crate::graphics::Color;
use crate::graphics::GraphicsContext;
use crate::graphics::Texture;

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
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app(&mut self).unwrap();

        rt.shutdown_timeout(Duration::from_secs(1));
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
                            .with_child_spacing(4.0)
                            .with_container(|ui| {
                                ui.with_color(Color::GREEN)
                                    .with_child_spacing(5.0)
                                    .with_padding(Padding {
                                        left: 5.0,
                                        right: 5.0,
                                        top: 5.0,
                                        bottom: 5.0,
                                    })
                                    .with_element(|ui| {
                                        ui.with_color(Color::WHITE)
                                            .with_height(100.0)
                                            .with_width(100.0);
                                    })
                                    .with_element(|ui| {
                                        ui.with_color(Color::WHITE)
                                            .with_height(100.0)
                                            .with_width(100.0);
                                    });
                            })
                            .with_element(|ui| {
                                ui.with_color(Color::RED)
                                    .with_height(100.0)
                                    .with_width(100.0);
                            });

                        // ui.with_color(Color::srgb(0.3, 0.3, 0.3, 1.0))
                        //     .with_element(|ui| {
                        //         ui.with_color(Color::WHITE)
                        //             .with_height(100.0, None)
                        //             .with_width(100.0, None);
                        //     })
                        //     .with_container(|ui| {
                        //         ui.with_color(Color::GREEN)
                        //             .with_element(|ui| {
                        //                 ui.with_color(Color::WHITE)
                        //                     .with_height(100.0, None)
                        //                     .with_width(100.0, None);
                        //             })
                        //             .with_element(|ui| {
                        //                 ui.with_color(Color::RED)
                        //                     .with_height(100.0, None)
                        //                     .with_width(100.0, None);
                        //             });
                        //     });
                    })
                    .finish(&mut canvas);

                // canvas.draw(Primitive::new(100.0, 100.0, 50.0, 50.0, Color::WHITE));
                // canvas.draw(Primitive::new(100.0, 200.0, 50.0, 50.0, Color::WHITE));
                // canvas.draw(Primitive::new(100.0, 300.0, 50.0, 50.0, Color::WHITE));
                // canvas.draw(Primitive::new(100.0, 400.0, 50.0, 50.0, Color::WHITE));
                // canvas.draw(
                //     Primitive::new(200.0, 50.0, 400.0, 450.0, Color::WHITE)
                //         .with_texture(self.texture.clone().unwrap()),
                // );
                // canvas.draw(
                //     Primitive::new(200.0, 50.0, 300.0, 100.0, Color::WHITE)
                //         .with_texture(self.texture2.clone().unwrap()),
                // );
                // canvas.draw_text(
                //     TextPrimitive::new(
                //         "Hello world!",
                //         &TextStyle::default(),
                //         100.0,
                //         470.0,
                //         Color::BLACK,
                //     )
                //     .with_max_width(200.),
                // );

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

    input: InputState,
    ui_context: UiContext,
}
