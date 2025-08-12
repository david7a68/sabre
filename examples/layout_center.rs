use sabre::Alignment;
use sabre::AppContextBuilder;
use sabre::AppLifecycleHandler;
use sabre::Color;
use sabre::Context;
use sabre::LayoutDirection;
use sabre::Padding;
use sabre::UiBuilder;
use sabre::ViewportConfig;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
    color_backtrace::install();

    let def_filter = tracing_subscriber::filter::Targets::new()
        .with_default(Level::DEBUG)
        .with_targets([
            ("naga", Level::WARN),
            ("wgpu_core", Level::WARN),
            ("wgpu_hal", Level::WARN),
            ("wgpu", Level::WARN),
        ]);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(def_filter)
        .init();

    AppContextBuilder::default().run(App {});
}

struct App {}

impl AppLifecycleHandler for App {
    fn resume(&mut self, runtime: &mut sabre::AppContext) {
        runtime.create_viewport(
            ViewportConfig {
                title: "Sabre App".into(),
                width: 800,
                height: 600,
            },
            AppWindow::default().into_handler(),
        );
    }
}

#[derive(Default)]
struct AppWindow {}

impl AppWindow {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |context, ui| self.update(context, ui)
    }

    fn update(&mut self, _context: Context, mut ui: UiBuilder) {
        ui.color(Color::srgb(0.1, 0.2, 0.3, 1.0))
            .child_alignment(Alignment::Center, Alignment::Center)
            .with_container(|ui| {
                ui.child_direction(LayoutDirection::Vertical)
                    .with_container(|ui| {
                        ui.child_spacing(10.0)
                            .padding(Padding {
                                left: 15.0,
                                right: 15.0,
                                top: 15.0,
                                bottom: 15.0,
                            })
                            .color(Color::BLUE)
                            .rect(100.0, 100.0, Color::WHITE)
                            .rect(100.0, 200.0, Color::WHITE)
                            .rect(30.0, 150.0, Color::WHITE);
                    })
                    .with_container(|ui| {
                        ui.child_spacing(10.0)
                            .color(Color::GREEN)
                            .padding(Padding {
                                left: 15.0,
                                right: 15.0,
                                top: 15.0,
                                bottom: 15.0,
                            })
                            .rect(100.0, 91.0, Color::WHITE)
                            .rect(100.0, 15.0, Color::WHITE)
                            .rect(100.0, 299.0, Color::WHITE);
                    });
            });
    }
}
