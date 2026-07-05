use plinth::graphics::Color;
use plinth::shell::{AppContext, AppContextBuilder, AppLifecycleHandler, Context, WindowConfig};
use plinth::ui::widget::{SplitPaneConfig, SplitPaneState};
use plinth::ui::{LayoutDirection, Padding, UiBuilder};

fn main() {
    AppContextBuilder::default().run(App::default());
}

#[derive(Default)]
struct App {}

impl AppLifecycleHandler for App {
    fn resume(&mut self, runtime: &mut AppContext) {
        runtime.create_window(
            WindowConfig {
                title: "N Split".into(),
                width: 1100,
                height: 700,
            },
            ViewState::default().into_handler(),
        );
    }
}

struct PaneState {
    color: Color,
}

struct ViewState {
    panes: Vec<PaneState>,
    splits: Vec<SplitPaneState>,
    next: usize,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            panes: vec![
                PaneState {
                    color: Color::srgb_nonlinear(0.15, 0.18, 0.25, 1.0),
                },
                PaneState {
                    color: Color::srgb_nonlinear(0.15, 0.25, 0.18, 1.0),
                },
                PaneState {
                    color: Color::srgb_nonlinear(0.25, 0.18, 0.15, 1.0),
                },
            ],
            splits: vec![SplitPaneState::new(0.33), SplitPaneState::new(0.5)],
            next: 4,
        }
    }
}

impl ViewState {
    fn into_handler(mut self) -> impl FnMut(Context, UiBuilder) {
        move |_context, mut ui| self.update(&mut ui)
    }

    fn update(&mut self, ui: &mut UiBuilder) {
        ui.child_direction(LayoutDirection::Vertical);
        ui.with_named_child("toolbar", |ui| {
            ui.height(48.0)
                .padding(Padding::equal(8.0))
                .child_spacing(8.0)
                .color(Color::srgb_nonlinear(0.10, 0.10, 0.12, 1.0));
            if ui.text_button("Add split").is_activated {
                let t = self.next as f32;
                self.next += 1;
                self.panes.push(PaneState {
                    color: Color::srgb_nonlinear(
                        0.12 + (t * 0.07).sin().abs() * 0.18,
                        0.12 + (t * 0.11).sin().abs() * 0.18,
                        0.12 + (t * 0.17).sin().abs() * 0.18,
                        1.0,
                    ),
                });
                self.splits.push(SplitPaneState::new(0.5));
            }
            if ui.text_button("Remove split").is_activated && self.panes.len() > 1 {
                self.panes.pop();
                self.splits.pop();
            }
            ui.text("Use the dividers to resize each split.", None);
        });
        ui.with_named_child("content", |ui| {
            render_panes(ui, &mut self.splits, &self.panes, 0);
        });
    }
}

fn render_panes(
    ui: &mut UiBuilder,
    splits: &mut [SplitPaneState],
    panes: &[PaneState],
    index: usize,
) {
    if panes.len() == 1 {
        render_leaf(ui, index, panes[0].color);
        return;
    }

    let (split, rest_splits) = splits.split_first_mut().unwrap();
    split.show(
        ui,
        SplitPaneConfig::horizontal(),
        |ui| render_leaf(ui, index, panes[0].color),
        |ui| render_panes(ui, rest_splits, &panes[1..], index + 1),
    );
}

fn render_leaf(ui: &mut UiBuilder, index: usize, color: Color) {
    ui.color(color).padding(Padding::equal(20.0));
    ui.text(&format!("Pane {}", index + 1), None);
}
