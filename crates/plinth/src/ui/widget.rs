use glamour::Rect;

use crate::ui::Pixels;
use crate::ui::UiBuilder;

#[derive(Clone, Debug)]
pub struct Interaction {
    pub is_clicked: bool,
    pub is_hovered: bool,
}

pub trait Widget {
    fn apply(self, context: &mut UiBuilder) -> Interaction;
}

pub struct WidgetState {
    pub placement: Rect<Pixels>,
}
