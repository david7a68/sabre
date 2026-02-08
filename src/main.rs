use plinth::graphics::Color;
use plinth::graphics::Paint;
use plinth::runtime::AppContext;
use plinth::runtime::AppContextBuilder;
use plinth::runtime::AppLifecycleHandler;
use plinth::runtime::Context;
use plinth::runtime::ViewportConfig;
use plinth::ui::Alignment;
use plinth::ui::LayoutDirection;
use plinth::ui::Padding;
use plinth::ui::Size;
use plinth::ui::Size::Flex;
use plinth::ui::Size::Grow;
use plinth::ui::StyleClass;
use plinth::ui::Theme;
use plinth::ui::UiBuilder;
use plinth::ui::style::StateFlags;
use plinth::ui::style::StyleProperty;
use plinth::ui::widget::UiBuilderWidgetsExt;
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

    let mut theme = Theme::new();

    theme.set_base_style([
        (
            StateFlags::NORMAL,
            StyleProperty::Background(Paint::solid(Color::srgb(0.1, 0.2, 0.3, 1.0))),
        ),
        (StateFlags::NORMAL, StyleProperty::FontSize(32)),
    ]);

    theme
        .set_style_class(
            StyleClass::Button,
            None,
            [
                (
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(Color::DARK_GRAY)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::Background(Paint::vertical_gradient(
                        Color::LIGHT_GRAY,
                        Color::DARK_GRAY,
                    )),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::Padding(Padding {
                        left: 10.0,
                        right: 10.0,
                        top: 5.0,
                        bottom: 5.0,
                    }),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::Width(Size::Fit {
                        min: 20.0,
                        max: f32::MAX,
                    }),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::Height(Size::Fit {
                        min: 10.0,
                        max: f32::MAX,
                    }),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::ChildMajorAlignment(Alignment::Center),
                ),
            ],
        )
        .unwrap();

    AppContextBuilder::default()
        .with_theme(theme)
        .run(SabreApp {});
}

#[derive(Default)]
struct SabreApp {}

impl AppLifecycleHandler for SabreApp {
    fn resume(&mut self, runtime: &mut AppContext) {
        info!("Starting up Sabre application...");

        runtime.create_viewport(
            ViewportConfig {
                title: "Sabre App".into(),
                width: 800,
                height: 600,
            },
            ViewportState::new().into_handler(),
        );
    }
}

struct ViewportState {}

impl ViewportState {
    fn new() -> Self {
        Self {}
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

        if menu.text_button("Menu Button").is_activated {
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
                .text("Menu Item 1", None)
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
                .text("modern morning merman even longer", None)
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
                .text("VA To ff ti it tt ft", None)
                .rect(Grow, None, None)
                .rect(45.0, 45.0, Color::RED);
        });
    }
}
