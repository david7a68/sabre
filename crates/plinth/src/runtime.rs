use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use slotmap::SlotMap;
use slotmap::new_key_type;
use smallvec::SmallVec;
use tracing::warn;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::event_loop::EventLoopProxy;
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::Window;
use winit::window::WindowId;

use crate::graphics::Canvas;
use crate::graphics::Color;
use crate::graphics::GraphicsContext;
use crate::ui::DrawCommand;
use crate::ui::InputState;
use crate::ui::UiBuilder;
use crate::ui::UiContext;
use crate::ui::text::TextLayoutContext;

#[derive(Default)]
pub struct AppContextBuilder {}

impl AppContextBuilder {
    pub fn run(self, handler: impl AppLifecycleHandler) {
        let event_loop = EventLoop::with_user_event()
            .with_dpi_aware(true)
            .build()
            .unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);

        let mut runtime = WinitApp {
            runtime: AppContext {
                viewports: SlotMap::with_key(),
                deferred_commands: Vec::new(),
                graphics: None,
                text_system: TextLayoutContext::default(),
                event_loop_proxy: event_loop.create_proxy(),
            },
            windows: HashMap::new(),
            user_handler: handler,
        };

        event_loop.run_app(&mut runtime).unwrap();
    }
}

pub trait AppLifecycleHandler {
    fn suspend(&mut self, runtime: &mut AppContext) {
        let _ = runtime;
    }

    fn resume(&mut self, runtime: &mut AppContext);
}

pub struct AppContext {
    viewports: SlotMap<ViewportId, Viewport>,

    deferred_commands: Vec<ViewportCommand>,

    graphics: Option<GraphicsContext>,
    text_system: TextLayoutContext,

    event_loop_proxy: EventLoopProxy<WinitEvent>,
}

impl AppContext {
    pub fn create_viewport(
        &mut self,
        config: ViewportConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) -> ViewportHandle {
        let id = self.viewports.insert(Viewport {
            input: InputState::default(),
            config,
        });

        self.deferred_commands.push(ViewportCommand::Create {
            id,
            handler: Box::new(handler),
        });

        ViewportHandle {
            id,
            event_loop_proxy: self.event_loop_proxy.clone(),
        }
    }

    fn repaint<'a>(
        &mut self,
        windows: impl IntoIterator<Item = (&'a mut WinitWindow, InputState)>,
    ) {
        let windows = windows.into_iter();
        let mut outputs = SmallVec::with_capacity(windows.size_hint().0);

        for (window, input) in windows {
            let ui_builder =
                window
                    .ui_context
                    .begin_frame(&mut self.text_system, input, Duration::ZERO);

            let context = Context {
                viewports: &mut self.viewports,
                deferred_commands: &mut self.deferred_commands,
                event_loop_proxy: &self.event_loop_proxy,
            };

            (window.handler)(context, ui_builder);

            window.canvas.reset(Color::BLACK);

            for draw_command in window.ui_context.finish() {
                match draw_command {
                    DrawCommand::Primitive(primitive) => {
                        window.canvas.draw(primitive);
                    }
                    DrawCommand::TextLayout(layout, coords) => {
                        window.canvas.draw_text_layout(layout, coords);
                    }
                }
            }

            if window.canvas.has_unready_textures() {
                window.window.request_redraw();
            }

            outputs.push((window.window.id(), &window.canvas));
        }

        let graphics = self.graphics.as_mut().unwrap();
        graphics.render(outputs).unwrap();
    }
}

#[derive(Clone)]
pub struct ViewportHandle {
    id: ViewportId,
    event_loop_proxy: EventLoopProxy<WinitEvent>,
}

impl ViewportHandle {
    pub fn send_message(&self, _: ()) {
        self.event_loop_proxy
            .send_event(WinitEvent {
                viewport_id: self.id,
            })
            .unwrap();
    }
}

#[derive(Clone, Debug)]
pub struct ViewportConfig {
    pub title: Cow<'static, str>,
    pub width: u32,
    pub height: u32,
}

pub struct Context<'a> {
    viewports: &'a mut SlotMap<ViewportId, Viewport>,
    deferred_commands: &'a mut Vec<ViewportCommand>,
    event_loop_proxy: &'a EventLoopProxy<WinitEvent>,
}

impl Context<'_> {
    pub fn create_viewport(
        &mut self,
        config: ViewportConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) -> ViewportHandle {
        let id = self.viewports.insert(Viewport {
            input: InputState::default(),
            config,
        });

        self.deferred_commands.push(ViewportCommand::Create {
            id,
            handler: Box::new(handler),
        });

        ViewportHandle {
            id,
            event_loop_proxy: self.event_loop_proxy.clone(),
        }
    }
}

struct WinitApp<App> {
    runtime: AppContext,
    windows: HashMap<WindowId, WinitWindow>,

    user_handler: App,
}

impl<App> WinitApp<App> {
    fn handle_deferred_commands(&mut self, event_loop: &ActiveEventLoop) {
        for command in self.runtime.deferred_commands.drain(..) {
            match command {
                ViewportCommand::Create { id, handler } => {
                    let window = Arc::new(
                        event_loop
                            .create_window(
                                Window::default_attributes()
                                    .with_no_redirection_bitmap(true)
                                    .with_visible(false),
                            )
                            .unwrap(),
                    );

                    let graphics = self
                        .runtime
                        .graphics
                        .get_or_insert_with(|| GraphicsContext::new(window.clone()));

                    self.windows.insert(
                        window.id(),
                        WinitWindow {
                            canvas: graphics.create_canvas(),
                            handler,
                            ui_context: UiContext::default(),
                            viewport: id,
                            window,
                        },
                    );
                }
            }
        }

        if self.runtime.viewports.is_empty() {
            warn!("No open windows, terminating...");
            event_loop.exit();
        }
    }
}

impl<App: AppLifecycleHandler> ApplicationHandler<WinitEvent> for WinitApp<App> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.user_handler.resume(&mut self.runtime);
        self.handle_deferred_commands(event_loop);

        self.runtime.repaint(self.windows.values_mut().map(|w| {
            w.window.set_visible(true);
            (w, InputState::default())
        }));
    }

    #[allow(unused_variables)]
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                let window = self.windows.get_mut(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                viewport.input.pointer = glamour::Point2 {
                    x: position.x as f32,
                    y: position.y as f32,
                };

                window.window.request_redraw();
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let window = self.windows.get_mut(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                match (button, state) {
                    (winit::event::MouseButton::Left, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.is_left_down = true;
                    }
                    (winit::event::MouseButton::Left, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.is_left_down = false;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.is_right_down = true;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.is_right_down = false;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.is_middle_down = true;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.is_middle_down = false;
                    }
                    _ => {
                        return;
                    }
                }

                window.window.request_redraw();
            }
            WindowEvent::Resized(physical_size) => {
                let window = self.windows.get(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                viewport.config.width = physical_size.width;
                viewport.config.height = physical_size.height;

                viewport.input.window_size.width = physical_size.width as f32;
                viewport.input.window_size.height = physical_size.height as f32;
            }
            WindowEvent::CloseRequested => {
                let window = self.windows.remove(&window_id).unwrap();

                self.runtime.viewports.remove(window.viewport);

                let graphics = self.runtime.graphics.as_mut().unwrap();
                graphics.destroy_surface(window_id);
            }
            WindowEvent::RedrawRequested => {
                let window = self.windows.get_mut(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();
                let input = viewport.input.clone();

                self.runtime.repaint([(window, input)]);
            }
            _ => {}
        }

        self.handle_deferred_commands(event_loop);
    }
}

new_key_type! {
    struct ViewportId;
}

struct Viewport {
    input: InputState,
    config: ViewportConfig,
}

struct WinitWindow {
    window: Arc<Window>,

    canvas: Canvas,
    viewport: ViewportId,
    ui_context: UiContext,
    handler: Box<dyn FnMut(Context, UiBuilder)>,
}

enum ViewportCommand {
    Create {
        id: ViewportId,
        handler: Box<dyn FnMut(Context, UiBuilder)>,
    },
}

#[derive(Debug)]
struct WinitEvent {
    #[expect(unused)]
    viewport_id: ViewportId,
}
