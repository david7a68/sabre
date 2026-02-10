//! Task 2 of the 7 GUIs benchmark: Temperature Converter.
//!
//! A simple bidirectional temperature converter between Celsius and Fahrenheit
//! using user-provided text input.

#![allow(unused_crate_dependencies)]

use plinth::graphics::Color;
use plinth::graphics::Paint;
use plinth::shell::AppContext;
use plinth::shell::AppContextBuilder;
use plinth::shell::AppLifecycleHandler;
use plinth::shell::Context;
use plinth::shell::WindowConfig;
use plinth::ui::Alignment;
use plinth::ui::StyleClass;
use plinth::ui::Theme;
use plinth::ui::UiBuilder;
use plinth::ui::style::StateFlags;
use plinth::ui::style::StyleProperty;
use plinth::ui::widget::Interaction;
use plinth::ui::widget::UiBuilderWidgetsExt;

fn main() {
    tracing_subscriber::fmt().pretty().init();

    let mut theme = Theme::new();

    theme.set_base_style([(
        StateFlags::NORMAL,
        StyleProperty::Background(Paint::solid(Color::LIGHT_GRAY)),
    )]);

    theme
        .set_style_class(
            StyleClass::TextEdit,
            None,
            [(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(Color::WHITE)),
            )],
        )
        .unwrap();

    theme
        .set_style_class(
            StyleClass::Label,
            None,
            [
                (
                    StateFlags::NORMAL,
                    StyleProperty::BorderWidths(Default::default()),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::CornerRadii(Default::default()),
                ),
            ],
        )
        .unwrap();

    AppContextBuilder::default().with_theme(theme).run(Demo {});
}

struct Demo {}

impl AppLifecycleHandler for Demo {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_viewport(
            WindowConfig {
                title: "Temperature Converter".into(),
                width: 400,
                height: 300,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {
    temp_f: f32,
    temp_c: Option<f32>,
}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, context: Context, mut ui: UiBuilder) {
        let mut panel = ui
            .child_alignment(Alignment::Center, Alignment::Center)
            .panel();

        let mut edit_c = panel.text_edit("", 60.0);
        if let Some(temp_c) = self.temp_c.take() {
            edit_c.set_text(&format!("{:.2}", temp_c));
        }

        let temp_c = parse_temp(edit_c.finish());

        panel.label("°C =");

        let mut edit_f = panel.text_edit("", 60.0);

        if let Some(temp_c) = temp_c {
            let temp_f = temp_c * 9.0 / 5.0 + 32.0;
            if temp_f != self.temp_f {
                edit_f.set_text(&format!("{:.2}", temp_f));
                self.temp_f = temp_f;
            }
        }

        if let Some(temp) = parse_temp(edit_f.finish()) {
            let temp_c = (temp - 32.0) * 5.0 / 9.0;

            if self.temp_c != Some(temp_c) {
                self.temp_c = Some(temp_c);

                // Need to request a repaint here since the changes to temp_c
                // won't be reflected until the next update, which won't happen
                // until the next user interaction otherwise. This means that
                // the data in temp_c will be out of date until the next frame.
                context.request_repaint();
            }
        }

        panel.label("°F");
    }
}

fn parse_temp((text, interaction): (Option<&str>, Interaction)) -> Option<f32> {
    if let Some(text) = text
        && interaction.is_focused
        && let Ok(temp) = text.parse::<f32>()
    {
        Some(temp)
    } else {
        None
    }
}
