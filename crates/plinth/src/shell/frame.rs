use std::path::PathBuf;

use slotmap::SlotMap;

use crate::ui::UiBuilder;

use super::Input;
use super::WindowConfig;
use super::window::Viewport;
use super::window::ViewportId;
use super::winit::DeferredCommand;

pub struct FileDialog {
    /// The title of the file dialog window.
    pub title: String,
    /// The starting file name for the file dialog.
    pub initial_file: String,
    /// The starting directory for the file dialog.
    pub directory: String,
    /// File extension filters for the file dialog. Each filter consists of a
    /// name and a list of extensions.
    ///
    /// On MacOS, the name is ignored and all extensions are merged into a
    /// single filter.
    pub filters: Vec<(String, Vec<String>)>,
}

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

    pub fn pick_file(&self, dialog: FileDialog) -> Option<PathBuf> {
        let mut builder = rfd::FileDialog::new()
            .set_title(dialog.title)
            .set_directory(dialog.directory)
            .set_file_name(dialog.initial_file);

        for (name, extensions) in dialog.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(self.window).pick_file()
    }

    pub fn pick_files(&self, dialog: FileDialog) -> Option<Vec<PathBuf>> {
        let mut builder = rfd::FileDialog::new()
            .set_title(dialog.title)
            .set_directory(dialog.directory)
            .set_file_name(dialog.initial_file);

        for (name, extensions) in dialog.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(self.window).pick_files()
    }

    pub fn pick_folder(&self, dialog: FileDialog) -> Option<PathBuf> {
        let mut builder = rfd::FileDialog::new()
            .set_title(dialog.title)
            .set_directory(dialog.directory);

        for (name, extensions) in dialog.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(self.window).pick_folder()
    }

    pub fn pick_folders(&self, dialog: FileDialog) -> Option<Vec<PathBuf>> {
        let mut builder = rfd::FileDialog::new()
            .set_title(dialog.title)
            .set_directory(dialog.directory);

        for (name, extensions) in dialog.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(self.window).pick_folders()
    }
}
