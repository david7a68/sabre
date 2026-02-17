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

    AppContextBuilder::default().run(TextEditDemo {});
}

struct TextEditDemo {}

impl AppLifecycleHandler for TextEditDemo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            WindowConfig {
                title: "TextEdit Example".into(),
                width: 800,
                height: 600,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {
    text_content: String,
}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, _context: Context, mut ui: UiBuilder) {
        ui.child_alignment(Alignment::Center, Alignment::Center);

        let mut panel = ui.surface();
        panel
            .width(600.0)
            .height(400.0)
            .child_alignment(Alignment::Start, Alignment::Start)
            .child_direction(LayoutDirection::Vertical)
            .padding(Padding::equal(20.0));

        panel.label("TextEdit Widget Demo:");

        let (text_result, interaction) = panel.text_edit(&self.text_content, 200.0).finish();

        let is_composing = if let Some(text_str) = text_result {
            self.text_content = text_str.to_string();
            false
        } else {
            true
        };

        let mut info_panel = panel
            .surface()
            .with_child_direction(LayoutDirection::Horizontal);

        info_panel.label(&format!("Current text: {}", self.text_content));
        info_panel.label(&format!(
            "Text length: {} characters",
            self.text_content.len()
        ));
        info_panel.label(&format!("Hovered: {}", interaction.is_hovered));
        info_panel.label(&format!("Clicked: {}", interaction.is_activated));

        if is_composing {
            info_panel.label("(IME composition in progress)");
        }
    }
}
