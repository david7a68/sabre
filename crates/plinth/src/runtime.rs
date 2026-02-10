use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use slotmap::SlotMap;
use slotmap::new_key_type;
use smallvec::SmallVec;
use winit::application::ApplicationHandler;
use winit::event::ButtonSource;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::event_loop::EventLoopProxy;
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::platform::windows::WindowAttributesWindows;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

use crate::graphics::Canvas;
use crate::graphics::Color;
use crate::graphics::GraphicsContext;
use crate::graphics::TextLayoutContext;
use crate::shell::Clipboard;
use crate::shell::DoubleClickTracker;
use crate::shell::Input;
use crate::shell::KeyboardEvent;
use crate::ui::Theme;
use crate::ui::UiBuilder;
use crate::ui::context::UiContext;
use crate::ui::text::TextLayoutStorage;

#[derive(Default)]
pub struct AppContextBuilder {
    theme: Option<Theme>,
}

impl AppContextBuilder {
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn run(self, handler: impl AppLifecycleHandler) {
        let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);

        let theme = self.theme.unwrap_or_default();

        let runtime = WinitApp {
            runtime: AppContext {
                viewports: SlotMap::with_key(),
                clipboard: Clipboard::new(),
                event_loop_proxy: event_loop.create_proxy(),
                deferred_commands: Vec::new(),
                theme,
                graphics: None,
                text_system: TextLayoutContext::default(),
                text_layouts: TextLayoutStorage::default(),
                format_buffer: String::with_capacity(2048),
            },
            windows: HashMap::new(),
            user_handler: handler,
        };

        event_loop.run_app(runtime).unwrap();
    }
}

pub trait AppLifecycleHandler: 'static {
    fn suspend(&mut self, runtime: &mut AppContext) {
        let _ = runtime;
    }

    fn resume(&mut self, runtime: &mut AppContext);
}

pub struct AppContext {
    viewports: SlotMap<ViewportId, Viewport>,
    clipboard: Clipboard,
    event_loop_proxy: EventLoopProxy,
    deferred_commands: Vec<ViewportCommand>,

    theme: Theme,

    graphics: Option<GraphicsContext>,
    text_system: TextLayoutContext,
    text_layouts: TextLayoutStorage,
    format_buffer: String,
}

impl AppContext {
    pub fn create_viewport(
        &mut self,
        config: ViewportConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) -> ViewportHandle {
        let id = self.viewports.insert(Viewport {
            input: Input::default(),
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

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    fn repaint<'a>(&mut self, windows: impl IntoIterator<Item = &'a mut WinitWindow>) {
        let windows = windows.into_iter();
        let mut outputs = SmallVec::with_capacity(windows.size_hint().0);

        for window in windows {
            let Some(viewport) = self.viewports.get_mut(window.viewport) else {
                continue;
            };

            // borrow input for this frame
            let input = std::mem::take(&mut viewport.input);

            let ui_builder = window.ui_context.begin_frame(
                &mut self.clipboard,
                &mut self.text_system,
                &mut self.text_layouts,
                &mut self.format_buffer,
                &self.theme,
                &input,
                Duration::ZERO,
            );

            let context = Context {
                window: window.window.as_ref(),
                viewports: &mut self.viewports,
                deferred_commands: &mut self.deferred_commands,
                event_loop_proxy: &self.event_loop_proxy,
            };

            (window.handler)(context, ui_builder);

            // Restore input allocs for next frame; use branch to avoid unwrap.
            // This should never fail.
            if let Some(viewport) = self.viewports.get_mut(window.viewport) {
                viewport.input = input;
                viewport.input.keyboard_events.clear();
            }

            window.canvas.reset(Color::BLACK);
            window.ui_context.finish(
                &mut self.text_system,
                &mut self.text_layouts,
                &mut window.canvas,
            );

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
    #[expect(unused)]
    id: ViewportId,
    event_loop_proxy: EventLoopProxy,
}

impl ViewportHandle {
    pub fn send_message(&self, _: ()) {
        self.event_loop_proxy.wake_up();
    }
}

#[derive(Clone, Debug)]
pub struct ViewportConfig {
    pub title: Cow<'static, str>,
    pub width: u32,
    pub height: u32,
}

pub struct Context<'a> {
    window: &'a dyn winit::window::Window,
    viewports: &'a mut SlotMap<ViewportId, Viewport>,
    deferred_commands: &'a mut Vec<ViewportCommand>,
    event_loop_proxy: &'a EventLoopProxy,
}

impl Context<'_> {
    pub fn create_viewport(
        &mut self,
        config: ViewportConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) -> ViewportHandle {
        let id = self.viewports.insert(Viewport {
            input: Input::default(),
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

    pub fn request_repaint(&self) {
        self.window.request_redraw();
    }
}

struct WinitApp<App> {
    runtime: AppContext,
    windows: HashMap<WindowId, WinitWindow>,

    user_handler: App,
}

impl<App> WinitApp<App> {
    fn handle_deferred_commands(&mut self, event_loop: &dyn ActiveEventLoop) {
        for command in self.runtime.deferred_commands.drain(..) {
            match command {
                ViewportCommand::Create { id, handler } => {
                    let window = Arc::<dyn Window>::from(
                        event_loop
                            .create_window(
                                WindowAttributes::default()
                                    .with_visible(false)
                                    .with_platform_attributes(Box::new(
                                        WindowAttributesWindows::default()
                                            .with_no_redirection_bitmap(true),
                                    )),
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
                            double_click_tracker: DoubleClickTracker::load_parameters(
                                window.scale_factor(),
                            ),
                            window,
                        },
                    );
                }
            }
        }

        if self.runtime.viewports.is_empty() {
            event_loop.exit();
        }
    }
}

impl<App: AppLifecycleHandler> ApplicationHandler for WinitApp<App> {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.user_handler.resume(&mut self.runtime);
        self.handle_deferred_commands(event_loop);

        self.runtime.repaint(self.windows.values_mut().inspect(|w| {
            w.window.set_visible(true);
        }));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::PointerMoved { position, .. } => {
                let window = self.windows.get_mut(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                viewport.input.pointer = glamour::Point2 {
                    x: position.x as f32,
                    y: position.y as f32,
                };

                window.window.request_redraw();
            }
            WindowEvent::PointerButton { state, button, .. } => {
                let window = self.windows.get_mut(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                let ButtonSource::Mouse(button) = button else {
                    return;
                };

                let click_count =
                    window
                        .double_click_tracker
                        .on_click(button, state, viewport.input.pointer);

                match (button, state) {
                    (winit::event::MouseButton::Left, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.left_click_count = click_count;
                    }
                    (winit::event::MouseButton::Left, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.left_click_count = click_count;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.right_click_count = click_count;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.right_click_count = click_count;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Pressed) => {
                        viewport.input.mouse_state.middle_click_count = click_count;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Released) => {
                        viewport.input.mouse_state.middle_click_count = click_count;
                    }
                    _ => {
                        return;
                    }
                }

                window.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let window = self.windows.get(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                viewport.input.keyboard_events.push(KeyboardEvent {
                    key: event.physical_key,
                    text: event.text,
                    location: event.location,
                    is_repeat: event.repeat,
                    state: event.state.into(),
                });

                window.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let window = self.windows.get(&window_id).unwrap();
                let viewport = self.runtime.viewports.get_mut(window.viewport).unwrap();

                viewport.input.modifiers = modifiers.state();
            }
            WindowEvent::SurfaceResized(physical_size) => {
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

                self.runtime.repaint([window]);
            }
            WindowEvent::Focused(_) => {
                let window = self.windows.get_mut(&window_id).unwrap();
                window.double_click_tracker.on_activate();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let window = self.windows.get_mut(&window_id).unwrap();
                window.double_click_tracker.on_dpi_changed(scale_factor);
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
    input: Input,
    config: ViewportConfig,
}

struct WinitWindow {
    window: Arc<dyn Window>,
    double_click_tracker: DoubleClickTracker,

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
