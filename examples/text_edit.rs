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
use plinth::ui::PlainEditableText;
use plinth::ui::UiBuilder;

fn main() {
    tracing_subscriber::fmt().pretty().init();

    AppContextBuilder::default().run(TextEditDemo {});
}

struct TextEditDemo {}

impl AppLifecycleHandler for TextEditDemo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_window(
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
    text_content: PlainEditableText,
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

        let interaction = panel
            .text_edit(&mut self.text_content, 200.0)
            .with_height(100.0)
            .finish();

        let text = self.text_content.raw_text();
        let is_composing = self.text_content.is_composing();

        let mut info_panel = panel
            .surface()
            .with_child_direction(LayoutDirection::Horizontal);

        info_panel.label(&format!("Current text: {text}"));
        info_panel.label(&format!("Text length: {} characters", text.len()));
        info_panel.label(&format!("Hovered: {}", interaction.is_hovered));
        info_panel.label(&format!("Clicked: {}", interaction.is_activated));

        if is_composing {
            info_panel.label("(IME composition in progress)");
        }
    }
}
