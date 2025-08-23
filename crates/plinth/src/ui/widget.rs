use glamour::Rect;

use crate::ui::Pixels;
use crate::ui::UiBuilder;

#[derive(Clone, Debug)]
pub struct Response {
    pub is_clicked: bool,
    pub is_hovered: bool,
}

pub trait Widget {
    fn apply(self, context: &mut UiBuilder) -> Response;
}

pub struct WidgetState {
    pub placement: Rect<Pixels>,
}
