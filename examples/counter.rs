#![allow(unused_crate_dependencies)]

use plinth::runtime::AppContext;
use plinth::runtime::AppContextBuilder;
use plinth::runtime::AppLifecycleHandler;
use plinth::runtime::Context;
use plinth::runtime::ViewportConfig;
use plinth::ui::Alignment;
use plinth::ui::LayoutDirection;
use plinth::ui::UiBuilder;
use plinth::ui::widgets::UiBuilderWidgetsExt;

fn main() {
    tracing_subscriber::fmt().pretty().init();
    AppContextBuilder::default().run(CounterDemo {});
}

struct CounterDemo {}

impl AppLifecycleHandler for CounterDemo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            ViewportConfig {
                title: "Counter Demo".into(),
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
        let mut ui = ui
            .child_alignment(Alignment::Center, Alignment::Center)
            .container();

        ui.child_direction(LayoutDirection::Vertical);
        ui.label(&format!("Counter Value: {}", self.value), None);

        let was_key_pressed = ui
            .input()
            .keyboard_events
            .iter()
            .any(|event| event.state.is_pressed());

        if ui.text_button("Increment").is_clicked || was_key_pressed {
            self.value += 1;
        }
    }
}
