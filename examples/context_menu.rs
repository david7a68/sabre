//! Context menu widget example demonstrating right-click activation, item
//! selection, keyboard navigation, and close-on-outside-click behavior.

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

fn main() {
    tracing_subscriber::fmt().pretty().init();

    AppContextBuilder::default().run(ContextMenuDemo::default());
}

#[derive(Default)]
struct ContextMenuDemo {}

impl AppLifecycleHandler for ContextMenuDemo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_window(
            WindowConfig {
                title: "Context Menu Example".into(),
                width: 600,
                height: 500,
            },
            AppWindow::default().into_handler(),
        );
    }
}

struct AppWindow {
    selected_action: &'static str,
}

impl Default for AppWindow {
    fn default() -> Self {
        Self {
            selected_action: "No action selected yet",
        }
    }
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
            .height(300.0)
            .child_direction(LayoutDirection::Vertical)
            .child_alignment(Alignment::Start, Alignment::Start)
            .padding(Padding::equal(20.0));

        panel.label("Context Menu Demo");
        panel.label("Right-click the target area below.");
        panel.label(self.selected_action);

        let actions = ["Copy", "Rename", "Delete"];
        if let Some(selected) = panel.context_menu(
            "demo_context_menu",
            |trigger| {
                let mut target = trigger.surface();
                target
                    .width(300.0)
                    .height(120.0)
                    .child_alignment(Alignment::Center, Alignment::Center)
                    .padding(Padding::equal(12.0));
                target.label("Right-click me");
            },
            |menu| {
                for action in actions {
                    menu.item(action);
                }
            },
        ) {
            self.selected_action = actions[selected];
        }

        panel.label("Keyboard navigation: Arrow Up/Down, Enter, Escape");
    }
}
