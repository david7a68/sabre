//! Task 1 of the 7 GUIs benchmark: Counter.
//!
//! A simple counter that increments when the button is pressed.

#![allow(unused_crate_dependencies)]

use plinth::graphics::Color;
use plinth::shell::AppContext;
use plinth::shell::AppContextBuilder;
use plinth::shell::AppLifecycleHandler;
use plinth::shell::Context;
use plinth::shell::WindowConfig;
use plinth::ui::Alignment;
use plinth::ui::LayoutDirection;
use plinth::ui::UiBuilder;
use plinth::ui::widget::UiBuilderWidgetsExt;

fn main() {
    tracing_subscriber::fmt().pretty().init();
    AppContextBuilder::default().run(Demo {});
}

struct Demo {}

impl AppLifecycleHandler for Demo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            WindowConfig {
                title: "Counter".into(),
                width: 400,
                height: 300,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {
    value: i32,
}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, _context: Context, mut ui: UiBuilder) {
        let mut panel = ui
            .child_alignment(Alignment::Center, Alignment::Center)
            .panel();

        panel
            .color(Color::LIGHT_GRAY)
            .child_direction(LayoutDirection::Vertical);

        panel.label(&format!("Counter Value: {}", self.value));
        panel.label("Press the button to increment");

        if panel.text_button("Increment").is_activated {
            self.value += 1;
        }
    }
}
