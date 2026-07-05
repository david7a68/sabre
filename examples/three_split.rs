use plinth::graphics::Color;
use plinth::shell::{AppContext, AppContextBuilder, AppLifecycleHandler, Context, WindowConfig};
use plinth::ui::widget::{SplitPaneConfig, SplitPaneState};
use plinth::ui::{Padding, UiBuilder};

fn main() {
    AppContextBuilder::default().run(App::default());
}

#[derive(Default)]
struct App {}

impl AppLifecycleHandler for App {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_window(
            WindowConfig {
                title: "Three Split".into(),
                width: 1000,
                height: 700,
            },
            ViewState::default().into_handler(),
        );
    }
}

struct ViewState {
    outer: SplitPaneState,
    inner: SplitPaneState,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            outer: SplitPaneState::new(0.33),
            inner: SplitPaneState::new(0.5),
        }
    }
}

impl ViewState {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |_context, mut ui| self.update(&mut ui)
    }

    fn update(&mut self, ui: &mut UiBuilder) {
        self.outer.show(
            ui,
            SplitPaneConfig::horizontal(),
            |ui| pane(ui, "Left", Color::srgb_nonlinear(0.13, 0.17, 0.23, 1.0)),
            |ui| {
                self.inner.show(
                    ui,
                    SplitPaneConfig::horizontal(),
                    |ui| pane(ui, "Middle", Color::srgb_nonlinear(0.13, 0.22, 0.17, 1.0)),
                    |ui| pane(ui, "Right", Color::srgb_nonlinear(0.23, 0.16, 0.14, 1.0)),
                );
            },
        );
    }
}

fn pane(ui: &mut UiBuilder, title: &str, color: Color) {
    ui.color(color).padding(Padding::equal(20.0));
    ui.text(title, None);
}
