use slotmap::SlotMap;

use crate::ui::UiBuilder;

use super::Input;
use super::WindowConfig;
use super::window::Viewport;
use super::window::ViewportId;
use super::winit::DeferredCommand;

pub struct Context<'a> {
    pub(super) window: &'a dyn winit::window::Window,
    pub(super) viewports: &'a mut SlotMap<ViewportId, Viewport>,
    pub(super) deferred_commands: &'a mut Vec<DeferredCommand>,
}

impl Context<'_> {
    pub fn create_window(
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

    pub fn request_repaint(&self) {
        self.window.request_redraw();
    }
}
