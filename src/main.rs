use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use sabre::app::App;

#[derive(Default)]
struct TracyConfig(tracing_subscriber::fmt::format::DefaultFields);

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
    color_backtrace::install();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().ok();
    let def_filter = env_filter.is_none().then(|| {
        tracing_subscriber::filter::Targets::new()
            .with_default(Level::DEBUG)
            .with_targets([
                ("naga", Level::WARN),
                ("wgpu_core", Level::WARN),
                ("wgpu_hal", Level::WARN),
                ("wgpu", Level::WARN),
            ])
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(tracing_tracy::TracyLayer::new(TracyConfig::default()))
        .with(env_filter)
        .with(def_filter)
        .init();

    App::new().run();
}
