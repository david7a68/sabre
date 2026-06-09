use std::cell::RefCell;
use std::rc::Rc;

use parley::Layout;

use crate::graphics::Canvas;
use crate::graphics::ClipRect;
use crate::graphics::Color;
use crate::graphics::TextAlignment;
use crate::ui::Flex;
use crate::ui::Size;
use crate::ui::UiBuilder;
use crate::ui::context::LayoutRect;
use crate::ui::context::UiContent;
use crate::ui::style::StateFlags;
use crate::ui::style::StyleId;
use crate::ui::theme::Theme;

use super::TextOverflow;
use super::TextServices;

pub trait TextBuilderExt {
    fn text(&mut self, text: &str, height: impl Into<Size>) -> &mut Self;
    fn text_overflow(&mut self, overflow: TextOverflow) -> &mut Self;
    fn clip_text(&mut self) -> &mut Self;
    fn wrap_text(&mut self) -> &mut Self;
}

impl TextBuilderExt for UiBuilder<'_> {
    fn text(&mut self, text: &str, height: impl Into<Size>) -> &mut Self {
        let alignment = self
            .theme
            .resolve_style::<TextAlignment>(self.style_id, self.state);
        let overflow = self.text_overflow;
        let services = self.text_services.clone();
        let (content, width) = StaticTextContent::new(
            services,
            self.theme,
            self.style_id,
            self.state,
            text,
            alignment,
            overflow,
        );

        self.custom_content(
            width,
            height,
            matches!(overflow, TextOverflow::Clip),
            content,
        )
    }

    fn text_overflow(&mut self, overflow: TextOverflow) -> &mut Self {
        self.text_overflow = overflow;
        self
    }

    fn clip_text(&mut self) -> &mut Self {
        self.text_overflow(TextOverflow::Clip)
    }

    fn wrap_text(&mut self) -> &mut Self {
        self.text_overflow(TextOverflow::Wrap)
    }
}

struct StaticTextContent {
    layout: RefCell<Layout<Color>>,
    alignment: TextAlignment,
    overflow: TextOverflow,
}

impl StaticTextContent {
    fn new(
        services: TextServices,
        theme: &Theme,
        style_id: StyleId,
        state: StateFlags,
        text: &str,
        alignment: TextAlignment,
        overflow: TextOverflow,
    ) -> (Rc<Self>, Size) {
        let mut layout = Layout::new();

        services.with_context(|context| {
            let mut builder = context
                .layouts
                .ranged_builder(&mut context.fonts, text, 1.0, false);
            theme.push_text_defaults(style_id, state, &mut builder);
            builder.build_into(&mut layout, text);
        });

        let size = layout.calculate_content_widths();
        let content = Rc::new(Self {
            layout: RefCell::new(layout),
            alignment,
            overflow,
        });

        (
            content,
            Flex {
                min: size.min,
                max: size.max,
            },
        )
    }
}

impl UiContent for StaticTextContent {
    fn measure(&self, max_width: f32) -> Option<f32> {
        let mut layout = self.layout.borrow_mut();
        match self.overflow {
            TextOverflow::Clip => {
                layout.break_all_lines(None);
            }
            TextOverflow::Wrap => {
                layout.break_all_lines(Some(max_width));
            }
        }
        layout.align(Some(max_width), self.alignment.into(), Default::default());
        Some(layout.height())
    }

    fn draw(&self, canvas: &mut Canvas, rect: LayoutRect, clip: ClipRect) {
        canvas.draw_text_layout(&self.layout.borrow(), [rect.origin.x, rect.origin.y], clip);
    }
}
