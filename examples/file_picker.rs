#![allow(unused_crate_dependencies)]

use plinth::shell::AppContext;
use plinth::shell::AppContextBuilder;
use plinth::shell::AppLifecycleHandler;
use plinth::shell::Context;
use plinth::shell::FileDialog;
use plinth::shell::FolderDialog;
use plinth::shell::WindowConfig;
use plinth::ui::Alignment;
use plinth::ui::CommonWidgetsExt;
use plinth::ui::LayoutDirection;
use plinth::ui::Size::Grow;
use plinth::ui::UiBuilder;

fn main() {
    tracing_subscriber::fmt().pretty().init();
    AppContextBuilder::default().run(Demo {});
}

struct Demo {}

impl AppLifecycleHandler for Demo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            WindowConfig {
                title: "File Picker".into(),
                width: 400,
                height: 300,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {
    open_file_result: String,
    open_files_result: Vec<String>,
    open_folder_result: String,
    open_folders_result: Vec<String>,
}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, context: Context, mut ui: UiBuilder) {
        let mut ui = ui
            .child_alignment(Alignment::Center, Alignment::Center)
            .surface()
            .with_child_direction(LayoutDirection::Vertical);

        let mut row1 = ui
            .frame()
            .with_child_direction(LayoutDirection::Horizontal)
            .with_child_alignment(Alignment::Center, Alignment::Center);

        row1.with_surface(|mut panel| {
            panel
                .child_direction(LayoutDirection::Vertical)
                .child_alignment(Alignment::Start, Alignment::Start)
                .width(200.0);

            if panel.text_button("Open File").is_activated {
                let file = context.pick_file(FileDialog {
                    title: "Open File".into(),
                    initial_file: String::new(),
                    directory: String::new(),
                    filters: vec![],
                });

                if let Some(file) = file {
                    self.open_file_result = file.to_string_lossy().to_string();
                }
            }

            panel.label(&self.open_file_result).width(Grow);
        })
        .with_surface(|mut panel| {
            panel
                .child_direction(LayoutDirection::Vertical)
                .child_alignment(Alignment::Start, Alignment::Start)
                .width(200.0);

            if panel.text_button("Open Folder").is_activated {
                let folder = context.pick_folder(FolderDialog {
                    title: "Open Folder".into(),
                    directory: String::new(),
                    filters: vec![],
                });

                if let Some(folder) = folder {
                    self.open_folder_result = folder.to_string_lossy().to_string();
                }
            }

            panel.label(&self.open_folder_result).width(Grow);
        });

        let mut row2 = ui
            .frame()
            .with_child_direction(LayoutDirection::Horizontal)
            .with_child_alignment(Alignment::Center, Alignment::Center);

        row2.with_surface(|mut panel| {
            panel
                .child_direction(LayoutDirection::Vertical)
                .child_alignment(Alignment::Start, Alignment::Start)
                .width(200.0);

            if panel.text_button("Open Multiple Files").is_activated {
                let files = context.pick_files(FileDialog {
                    title: "Open Multiple Files".into(),
                    initial_file: String::new(),
                    directory: String::new(),
                    filters: vec![],
                });

                if let Some(files) = files {
                    self.open_files_result = files
                        .iter()
                        .map(|f| f.to_string_lossy().to_string())
                        .collect();
                }
            }

            if self.open_files_result.is_empty() {
                panel.label("");
            } else {
                for file in &self.open_files_result {
                    panel.label(file).width(Grow);
                }
            }
        })
        .with_surface(|mut panel| {
            panel
                .child_direction(LayoutDirection::Vertical)
                .child_alignment(Alignment::Start, Alignment::Start)
                .width(200.0);

            if panel.text_button("Open Multiple Folders").is_activated {
                let folders = context.pick_folders(FolderDialog {
                    title: "Open Multiple Folders".into(),
                    directory: String::new(),
                    filters: vec![],
                });

                if let Some(folders) = folders {
                    self.open_folders_result = folders
                        .iter()
                        .map(|f| f.to_string_lossy().to_string())
                        .collect();
                }
            }

            if self.open_folders_result.is_empty() {
                panel.label("");
            } else {
                for folder in &self.open_folders_result {
                    panel.label(folder).width(Grow);
                }
            }
        });
    }
}
