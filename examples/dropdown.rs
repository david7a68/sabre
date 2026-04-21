//! Dropdown menu widget example demonstrating single-select, keyboard navigation,
//! and close-on-outside-click behavior.

#![allow(unused_crate_dependencies)]

use plinth::shell::AppContext;
use plinth::shell::AppContextBuilder;
use plinth::shell::AppLifecycleHandler;
use plinth::shell::Context;
use plinth::shell::WindowConfig;
use plinth::ui::Alignment;
use plinth::ui::CommonWidgetsExt;
use plinth::ui::LayoutDirection;
use plinth::ui::Padding;
use plinth::ui::UiBuilder;
use plinth::ui::widget::DropdownItem;

fn main() {
    tracing_subscriber::fmt().pretty().init();

    AppContextBuilder::default().run(DropdownDemo {});
}

struct DropdownDemo {}

impl AppLifecycleHandler for DropdownDemo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            WindowConfig {
                title: "Dropdown Example".into(),
                width: 600,
                height: 500,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {
    selected_color: Option<usize>,
    selected_size: Option<usize>,
    selected_style: Option<usize>,
}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, _context: Context, mut ui: UiBuilder) {
        ui.child_alignment(Alignment::Center, Alignment::Center);

        let mut panel = ui.surface();
        panel
            .width(400.0)
            .height(350.0)
            .child_direction(LayoutDirection::Vertical)
            .child_alignment(Alignment::Start, Alignment::Start)
            .padding(Padding::equal(20.0));

        panel.label("Dropdown Widget Demo");

        panel.label("Select a color:");
        let items = ["Red", "Green", "Blue", "Yellow", "Purple"];

        self.selected_color = panel.dropdown(
            "color_dropdown",
            // how to avoid this new string on every frame?
            format!(
                "Selected: {}",
                self.selected_color.map(|i| items[i]).unwrap_or("None")
            )
            .as_str(),
            self.selected_color,
            items,
        );

        panel.label("Select a size:");

        let sizes = [
            "Small (8px)",
            "Medium (12px)",
            "Large (16px)",
            "Extra Large (20px)",
        ];
        self.selected_size = panel.dropdown(
            "size_dropdown",
            // how to avoid this new string on every frame?
            format!(
                "Selected: {}",
                self.selected_size
                    .map(|i| sizes[i])
                    .unwrap_or("Click to select")
            )
            .as_str(),
            self.selected_size,
            sizes,
        );

        panel.label("Select a style (custom items):");

        let style_items = ["Bold", "Italic", "Underline"];
        self.selected_style = panel.dropdown(
            "style_dropdown",
            // how to avoid this new string on every frame?
            format!(
                "Selected: {}",
                self.selected_style
                    .map(|i| style_items[i])
                    .unwrap_or("Click to select")
            )
            .as_str(),
            self.selected_style,
            style_items.iter().map(|size| size as &dyn DropdownItem),
        );

        panel.label("");
        panel.label("Keyboard navigation:");
        panel.label("• Arrow Up/Down to navigate");
        panel.label("• Enter to select, Escape to close");
    }
}
