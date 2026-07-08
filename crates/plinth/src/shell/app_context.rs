use std::collections::HashMap;
use std::time::Duration;

use smallvec::SmallVec;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;

use crate::graphics::Color;
use crate::graphics::GraphicsContext;
use crate::graphics::TextLayoutContext;
use crate::shell::Clipboard;
use crate::shell::Input;
use crate::shell::WindowConfig;
use crate::ui::Theme;
use crate::ui::UiBuilder;
use crate::ui::context::UiContext;
use crate::ui::text::TextLayoutStorage;

use super::frame::Context;
use super::winit::DeferredCommand;
use super::winit::WinitApp;
use super::winit::WinitWindow;

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
            runtime: AppContext::new(theme),
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
    pub(super) clipboard: Clipboard,
    pub(super) deferred_commands: Vec<DeferredCommand>,

    pub(super) theme: Theme,

    pub(super) graphics: Option<GraphicsContext>,
    pub(super) text_system: TextLayoutContext,
    pub(super) text_layouts: TextLayoutStorage,
    pub(super) format_buffer: String,
}

impl AppContext {
    pub(super) fn new(theme: Theme) -> Self {
        Self {
            clipboard: Clipboard::new(),
            deferred_commands: Vec::new(),
            theme,
            graphics: None,
            text_system: TextLayoutContext::default(),
            text_layouts: TextLayoutStorage::default(),
            format_buffer: String::with_capacity(2048),
        }
    }

    pub fn create_window(
        &mut self,
        config: WindowConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) {
        self.deferred_commands.push(DeferredCommand::Create {
            config,
            handler: Box::new(handler),
        });
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    pub(super) fn repaint<'a>(&mut self, windows: impl IntoIterator<Item = &'a mut WinitWindow>) {
        let AppContext {
            clipboard,
            deferred_commands,
            theme,
            graphics,
            text_system,
            text_layouts,
            format_buffer,
        } = self;

        let graphics = graphics.as_mut().unwrap();

        let windows = windows.into_iter();
        let mut outputs = SmallVec::with_capacity(windows.size_hint().0);

        for window in windows {
            let winit_window: &dyn winit::window::Window = window.window.as_ref();
            let graphics_for_frame: &mut GraphicsContext = &mut *graphics;
            let handler = &mut window.handler;

            run_ui_frame_core(
                &mut window.input,
                &mut window.ui_context,
                deferred_commands,
                clipboard,
                text_system,
                text_layouts,
                format_buffer,
                theme,
                Duration::ZERO,
                |deferred_commands, ui_builder| {
                    let context = Context {
                        window: winit_window,
                        graphics: graphics_for_frame,
                        deferred_commands,
                    };

                    (handler)(context, ui_builder);
                },
            );

            window.canvas.reset(Color::BLACK);
            window
                .ui_context
                .finish(text_system, text_layouts, &mut window.canvas);

            if window.canvas.has_unready_textures() {
                window.window.request_redraw();
            }

            outputs.push((window.window.id(), &window.canvas));
        }

        graphics.render(outputs).unwrap();
    }
}

#[expect(clippy::too_many_arguments)]
pub(super) fn run_ui_frame_core(
    input: &mut Input,
    ui_context: &mut UiContext,
    deferred_commands: &mut Vec<DeferredCommand>,
    clipboard: &mut Clipboard,
    text_system: &mut TextLayoutContext,
    text_layouts: &mut TextLayoutStorage,
    format_buffer: &mut String,
    theme: &Theme,
    time_delta: Duration,
    run_ui: impl FnOnce(&mut Vec<DeferredCommand>, UiBuilder),
) {
    // `begin_frame` returns a `UiBuilder` that holds a shared borrow on the
    // input. Move the input into a local for the duration of the frame so
    // it can be mutated after the user closure runs.
    let mut local_input = std::mem::take(input);

    let ui_builder = ui_context.begin_frame(
        clipboard,
        text_system,
        text_layouts,
        format_buffer,
        theme,
        &local_input,
        time_delta,
    );

    run_ui(deferred_commands, ui_builder);

    local_input.prev_pointer = local_input.pointer;
    *input = local_input;
    input.keyboard_events.clear();
}
