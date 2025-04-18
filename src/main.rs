use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;

use sabre::app::App;

fn main() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().ok();
    let def_filter = env_filter.is_none().then(|| {
        tracing_subscriber::filter::Targets::new().with_targets([
            ("wgpu_core", Level::WARN),
            ("wgpu_hal", Level::WARN),
            ("wgpu", Level::WARN),
            ("sabre", Level::DEBUG),
        ])
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_tracy::TracyLayer::default())
        .with(env_filter)
        .with(def_filter)
        .init();

    let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
