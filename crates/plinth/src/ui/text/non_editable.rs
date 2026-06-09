use crate::graphics::TextAlignment;
use crate::ui::Size;
use crate::ui::UiBuilder;

use super::TextOverflow;

pub trait TextBuilderExt {
    fn text(&mut self, text: &str, height: impl Into<Size>) -> &mut Self;
    fn text_with_overflow(
        &mut self,
        text: &str,
        height: impl Into<Size>,
        overflow: TextOverflow,
    ) -> &mut Self;
}

impl TextBuilderExt for UiBuilder<'_> {
    fn text(&mut self, text: &str, height: impl Into<Size>) -> &mut Self {
        self.text_with_overflow(text, height, TextOverflow::Clip)
    }

    fn text_with_overflow(
        &mut self,
        text: &str,
        height: impl Into<Size>,
        overflow: TextOverflow,
    ) -> &mut Self {
        let alignment = self
            .theme
            .resolve_style::<TextAlignment>(self.style_id, self.state);
        let layout = self
            .context
            .static_text_layout(self.id, &self.text_services);
        let width = self.text_services.prepare_static_text_layout(
            layout,
            self.theme,
            self.style_id,
            self.state,
            text,
        );

        self.static_text(width, height, layout, alignment, overflow)
    }
}
