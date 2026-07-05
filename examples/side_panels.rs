use plinth::graphics::Color;
use plinth::shell::{AppContext, AppContextBuilder, AppLifecycleHandler, Context, WindowConfig};
use plinth::ui::widget::{
    EdgeRegionConfig, EdgeRegionState, SectionPanelConfig, SectionPanelState,
};
use plinth::ui::{LayoutDirection, Padding, Size, UiBuilder};

fn main() {
    AppContextBuilder::default().run(App::default());
}

#[derive(Default)]
struct App {}

impl AppLifecycleHandler for App {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_window(
            WindowConfig {
                title: "Side Panels".into(),
                width: 1000,
                height: 700,
            },
            ViewState::default().into_handler(),
        );
    }
}

struct ViewState {
    left: EdgeRegionState,
    right: EdgeRegionState,
    transform: SectionPanelState,
    view: SectionPanelState,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            left: EdgeRegionState::new(0.24),
            right: EdgeRegionState::new(0.72),
            transform: SectionPanelState::open(),
            view: SectionPanelState::open(),
        }
    }
}

impl ViewState {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |_context, mut ui| self.update(&mut ui)
    }

    fn update(&mut self, ui: &mut UiBuilder) {
        ui.color(Color::srgb_nonlinear(0.08, 0.09, 0.11, 1.0));

        self.left.show(
            ui,
            EdgeRegionConfig::left().with_min_region(160.0),
            |ui| {
                sidebar(
                    ui,
                    "Navigation",
                    Color::srgb_nonlinear(0.13, 0.16, 0.20, 1.0),
                )
            },
            |ui| {
                self.right.show(
                    ui,
                    EdgeRegionConfig::right().with_min_region(220.0),
                    |ui| inspector(ui, &mut self.transform, &mut self.view),
                    main_area,
                );
            },
        );
    }
}

fn sidebar(ui: &mut UiBuilder, title: &str, color: Color) {
    ui.color(color)
        .padding(Padding::equal(12.0))
        .child_direction(LayoutDirection::Vertical)
        .child_spacing(8.0);
    ui.text(title, None);
    ui.text_button("Scene");
    ui.text_button("Objects");
    ui.text_button("Materials");
}

fn inspector(ui: &mut UiBuilder, transform: &mut SectionPanelState, view: &mut SectionPanelState) {
    ui.color(Color::srgb_nonlinear(0.12, 0.12, 0.14, 1.0))
        .padding(Padding::equal(8.0))
        .child_direction(LayoutDirection::Vertical)
        .child_spacing(6.0);
    transform.show(ui, "Transform", SectionPanelConfig::default(), |ui| {
        ui.text("Location: 0, 0, 0", None);
        ui.text("Rotation: 0, 0, 0", None);
        ui.text("Scale: 1, 1, 1", None);
    });
    view.show(ui, "View", SectionPanelConfig::default(), |ui| {
        ui.text("Lens: 50mm", None);
        ui.text("Clip: 0.1 - 1000", None);
    });
}

fn main_area(ui: &mut UiBuilder) {
    ui.color(Color::srgb_nonlinear(0.05, 0.06, 0.07, 1.0))
        .padding(Padding::equal(20.0));
    ui.text("Main editor area", None);
    ui.rect(
        Size::Grow,
        Size::Grow,
        Color::srgb_nonlinear(0.08, 0.10, 0.13, 1.0),
    );
}
