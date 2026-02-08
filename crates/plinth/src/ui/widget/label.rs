use crate::ui::StyleClass;
use crate::ui::UiBuilder;
use crate::ui::style::StateFlags;

pub struct Label {}

impl Label {
    pub fn new(builder: &mut UiBuilder<'_>, text: &str) -> Self {
        let mut builder = builder.child();
        builder.apply_style(StyleClass::Label, StateFlags::NORMAL);
        builder.text(text, None);
        Self {}
    }
}
