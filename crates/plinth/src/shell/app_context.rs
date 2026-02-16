use std::collections::HashMap;
use std::time::Duration;

use slotmap::SlotMap;
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
use crate::ui::text::TextLayoutStorage;

use super::frame::Context;
use super::window::Viewport;
use super::window::ViewportId;
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
            runtime: AppContext {
                viewports: SlotMap::with_key(),
                clipboard: Clipboard::new(),
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
    pub(super) viewports: SlotMap<ViewportId, Viewport>,
    pub(super) clipboard: Clipboard,
    pub(super) deferred_commands: Vec<DeferredCommand>,

    pub(super) theme: Theme,

    pub(super) graphics: Option<GraphicsContext>,
    pub(super) text_system: TextLayoutContext,
    pub(super) text_layouts: TextLayoutStorage,
    pub(super) format_buffer: String,
}

impl AppContext {
    pub fn create_viewport(
        &mut self,
        config: WindowConfig,
        handler: impl FnMut(Context, UiBuilder) + 'static,
    ) {
        let id = self.viewports.insert(Viewport {
            input: Input::default(),
            config,
        });

        self.deferred_commands.push(DeferredCommand::Create {
            id,
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
        let graphics = self.graphics.as_mut().unwrap();

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
                graphics,
                viewports: &mut self.viewports,
                deferred_commands: &mut self.deferred_commands,
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

        graphics.render(outputs).unwrap();
    }
}
