use std::rc::Rc;

use plinth::graphics::Color;
use plinth::runtime::AppContext;
use plinth::runtime::AppContextBuilder;
use plinth::runtime::AppLifecycleHandler;
use plinth::runtime::Context;
use plinth::runtime::ViewportConfig;
use plinth::ui::Alignment;
use plinth::ui::LayoutDirection;
use plinth::ui::Padding;
use plinth::ui::Size::Flex;
use plinth::ui::Size::Grow;
use plinth::ui::UiBuilder;
use plinth::ui::text::TextStyle;
use plinth::ui::widgets::UiBuilderWidgetsExt;
use tracing::Level;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Default)]
#[cfg(feature = "profile")]
struct TracyConfig(tracing_subscriber::fmt::format::DefaultFields);

#[cfg(feature = "profile")]
impl tracing_tracy::Config for TracyConfig {
    type Formatter = tracing_subscriber::fmt::format::DefaultFields;

    fn formatter(&self) -> &Self::Formatter {
        &self.0
    }

    fn stack_depth(&self, _: &tracing::metadata::Metadata<'_>) -> u16 {
        10
    }
}

fn main() {
    let def_filter = tracing_subscriber::filter::Targets::new()
        .with_default(Level::DEBUG)
        .with_targets([
            ("naga", Level::WARN),
            ("wgpu_core", Level::WARN),
            ("wgpu_hal", Level::WARN),
            ("wgpu", Level::WARN),
        ]);

    #[allow(unused_mut)]
    let mut registry =
        tracing_subscriber::registry().with(tracing_subscriber::fmt::layer().pretty());

    #[cfg(feature = "profile")]
    {
        registry = registry.with(tracing_tracy::TracyLayer::new(TracyConfig::default()));
    }

    registry.with(def_filter).init();

    AppContextBuilder::default().run(SabreApp {
        text_style: Rc::new(TextStyle::default()),
    });
}

#[derive(Default)]
struct SabreApp {
    text_style: Rc<TextStyle>,
}

impl AppLifecycleHandler for SabreApp {
    fn resume(&mut self, runtime: &mut AppContext) {
        info!("Starting up Sabre application...");

        runtime.create_viewport(
            ViewportConfig {
                title: "Sabre App".into(),
                width: 800,
                height: 600,
            },
            ViewportState::new(self.text_style.clone()).into_handler(),
        );
    }
}

struct ViewportState {
    text_style: Rc<TextStyle>,
}

impl ViewportState {
    fn new(text_style: Rc<TextStyle>) -> Self {
        Self { text_style }
    }

    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, _context: Context, mut ui: UiBuilder) {
        ui.color(Color::srgb(0.1, 0.2, 0.3, 1.0))
            .child_alignment(Alignment::Center, Alignment::Center);

        let mut menu = ui.child();

        menu.width(Flex {
            min: 200.0,
            max: 600.0,
        })
        .child_direction(LayoutDirection::Vertical);

        if menu.text_button("Menu Button", &self.text_style).is_clicked {
            info!("Menu Item 1 clicked");
        };

        menu.with_child(|ui| {
            ui.child_minor_alignment(Alignment::Center)
                .width(Grow)
                .color(Color::WHITE)
                .padding(Padding {
                    left: 15.0,
                    right: 15.0,
                    top: 15.0,
                    bottom: 15.0,
                })
                .label("Menu Item 1", &self.text_style, None)
                .rect(Grow, None, None)
                .rect(45.0, 45.0, Color::RED);
        });

        menu.with_child(|ui| {
            ui.child_minor_alignment(Alignment::Center)
                .width(Grow)
                .color(Color::WHITE)
                .padding(Padding {
                    left: 15.0,
                    right: 15.0,
                    top: 15.0,
                    bottom: 15.0,
                })
                .label("modern morning merman even longer", &self.text_style, None)
                .rect(Grow, None, None)
                .rect(45.0, 45.0, Color::RED);
        });

        menu.with_child(|ui| {
            ui.child_minor_alignment(Alignment::Center)
                .width(Grow)
                .color(Color::WHITE)
                .padding(Padding {
                    left: 15.0,
                    right: 15.0,
                    top: 15.0,
                    bottom: 15.0,
                })
                .label("VA To ff ti it tt ft", &self.text_style, None)
                .rect(Grow, None, None)
                .rect(45.0, 45.0, Color::RED);
        });
    }
}
