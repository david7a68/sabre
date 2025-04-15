use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;

use sabre::app::App;

fn main() {
    env_logger::builder()
        .format_file(true)
        .format_line_number(true)
        .format_timestamp_micros()
        .init();

    let event_loop = EventLoop::builder().with_dpi_aware(true).build().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
