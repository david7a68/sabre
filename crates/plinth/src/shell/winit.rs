use std::collections::HashMap;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::ButtonSource;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::WindowAttributesWindows;
use winit::window::Window;
use winit::window::WindowAttributes;
use winit::window::WindowId;

use crate::graphics::Canvas;
use crate::graphics::GraphicsContext;
use crate::shell::Input;
use crate::shell::KeyboardEvent;
use crate::shell::WindowConfig;
use crate::ui::UiBuilder;
use crate::ui::context::UiContext;

use super::app_context::AppContext;
use super::app_context::AppLifecycleHandler;
use super::frame::Context;
use super::input::DoubleClickTracker;

pub(super) struct WinitWindow {
    pub window: Arc<dyn Window>,
    pub double_click_tracker: DoubleClickTracker,

    pub canvas: Canvas,
    pub ui_context: UiContext,
    pub input: Input,
    pub config: WindowConfig,
    pub handler: Box<dyn FnMut(Context, UiBuilder)>,
}

pub(super) enum DeferredCommand {
    Create {
        config: WindowConfig,
        handler: Box<dyn FnMut(Context, UiBuilder)>,
    },
}

pub(super) struct WinitApp<App> {
    pub runtime: AppContext,
    pub windows: HashMap<WindowId, WinitWindow>,

    pub user_handler: App,
}

impl<App> WinitApp<App> {
    fn handle_deferred_commands(&mut self, event_loop: &dyn ActiveEventLoop) {
        for command in self.runtime.deferred_commands.drain(..) {
            match command {
                DeferredCommand::Create { config, handler } => {
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
                            input: Input::default(),
                            config,
                            double_click_tracker: DoubleClickTracker::load_parameters(
                                window.scale_factor(),
                            ),
                            window,
                        },
                    );
                }
            }
        }

        if self.windows.is_empty() {
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

                window.input.pointer = glamour::Point2 {
                    x: position.x as f32,
                    y: position.y as f32,
                };

                window.window.request_redraw();
            }
            WindowEvent::PointerButton { state, button, .. } => {
                let window = self.windows.get_mut(&window_id).unwrap();

                let ButtonSource::Mouse(button) = button else {
                    return;
                };

                let click_count =
                    window
                        .double_click_tracker
                        .on_click(button, state, window.input.pointer);

                match (button, state) {
                    (winit::event::MouseButton::Left, winit::event::ElementState::Pressed) => {
                        window.input.mouse_state.left_click_count = click_count;
                    }
                    (winit::event::MouseButton::Left, winit::event::ElementState::Released) => {
                        window.input.mouse_state.left_click_count = click_count;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Pressed) => {
                        window.input.mouse_state.right_click_count = click_count;
                    }
                    (winit::event::MouseButton::Right, winit::event::ElementState::Released) => {
                        window.input.mouse_state.right_click_count = click_count;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Pressed) => {
                        window.input.mouse_state.middle_click_count = click_count;
                    }
                    (winit::event::MouseButton::Middle, winit::event::ElementState::Released) => {
                        window.input.mouse_state.middle_click_count = click_count;
                    }
                    _ => {
                        return;
                    }
                }

                window.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let window = self.windows.get_mut(&window_id).unwrap();

                window.input.keyboard_events.push(KeyboardEvent {
                    key: event.physical_key,
                    text: event.text,
                    location: event.location,
                    is_repeat: event.repeat,
                    state: event.state.into(),
                });

                window.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let window = self.windows.get_mut(&window_id).unwrap();

                window.input.modifiers = modifiers.state();
            }
            WindowEvent::SurfaceResized(physical_size) => {
                let window = self.windows.get_mut(&window_id).unwrap();

                window.config.width = physical_size.width;
                window.config.height = physical_size.height;

                window.input.window_size.width = physical_size.width as f32;
                window.input.window_size.height = physical_size.height as f32;
            }
            WindowEvent::CloseRequested => {
                self.windows.remove(&window_id);

                let graphics = self.runtime.graphics.as_mut().unwrap();
                graphics.destroy_surface(window_id);
            }
            WindowEvent::RedrawRequested => {
                let window = self.windows.get_mut(&window_id).unwrap();

                self.runtime.repaint([window]);
            }
            WindowEvent::Focused(is_focused) if is_focused => {
                let window = self.windows.get_mut(&window_id).unwrap();
                window.double_click_tracker.on_activate();
                window.input.focus_changed();
                window.window.request_redraw();
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
