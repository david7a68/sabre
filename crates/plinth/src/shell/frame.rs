use std::path::Path;
use std::path::PathBuf;

use slotmap::SlotMap;

use crate::graphics::GraphicsContext;
use crate::graphics::Texture;
use crate::graphics::TextureLoadError;
use crate::ui::UiBuilder;

use super::Input;
use super::WindowConfig;
use super::window::Viewport;
use super::window::ViewportId;
use super::winit::DeferredCommand;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl FileDialog {
    fn builder(self, window: &dyn winit::window::Window) -> rfd::FileDialog {
        let mut builder = rfd::FileDialog::new()
            .set_title(self.title)
            .set_directory(self.directory)
            .set_file_name(self.initial_file);

        for (name, extensions) in self.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(window)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FolderDialog {
    /// The title of the folder dialog window.
    pub title: String,
    /// The starting directory for the folder dialog.
    pub directory: String,
    /// File extension filters for the file dialog. Each filter consists of a
    /// name and a list of extensions.
    ///
    /// On MacOS, the name is ignored and all extensions are merged into a
    /// single filter.
    pub filters: Vec<(String, Vec<String>)>,
}

impl FolderDialog {
    fn builder(self, window: &dyn winit::window::Window) -> rfd::FileDialog {
        let mut builder = rfd::FileDialog::new()
            .set_title(self.title)
            .set_directory(self.directory);

        for (name, extensions) in self.filters {
            builder = builder.add_filter(&name, &extensions);
        }

        builder.set_parent(window)
    }
}

pub struct Context<'a> {
    pub(super) window: &'a dyn winit::window::Window,
    pub(super) graphics: &'a mut GraphicsContext,
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

    pub fn load_image(&self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.graphics.load_image(path)
    }

    pub fn pick_file(&self, dialog: FileDialog) -> Option<PathBuf> {
        dialog.builder(self.window).pick_file()
    }

    pub fn pick_files(&self, dialog: FileDialog) -> Option<Vec<PathBuf>> {
        dialog.builder(self.window).pick_files()
    }

    pub fn pick_folder(&self, dialog: FolderDialog) -> Option<PathBuf> {
        dialog.builder(self.window).pick_folder()
    }

    pub fn pick_folders(&self, dialog: FolderDialog) -> Option<Vec<PathBuf>> {
        dialog.builder(self.window).pick_folders()
    }
}
