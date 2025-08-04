use std::sync::Arc;

use graphics::Color;

use crate::FontStack;
use crate::FontStyle;
use crate::FontWeight;
use crate::TextStyle;

#[derive(Clone, Debug)]
pub struct Style {
    pub text: TextStyle,
}

#[derive(Clone, Debug)]
pub enum StyleProperty {
    FontColor(Color),
    FontSize(f32),
    FontStyle(FontStyle),
    FontWeight(FontWeight),
    FontStack(Arc<FontStack<'static>>),
    StrikethroughColor(Option<Color>),
    StrikethroughOffset(Option<f32>),
    UnderlineColor(Option<Color>),
    UnderlineOffset(Option<f32>),
}

pub(crate) struct StyleStack {
    style: Style,
    changes: Vec<StyleChange>,
}

impl StyleStack {
    pub fn new(style: Style) -> Self {
        Self {
            style,
            changes: Vec::new(),
        }
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn push(&mut self, changes: impl IntoIterator<Item = StyleProperty>) {
        self.changes.push(StyleChange::StackPush);

        for change in changes.into_iter() {
            let record = self.apply(change);
            self.changes.push(StyleChange::Property(record));
        }
    }

    pub fn pop(&mut self) {
        while let Some(change) = self
            .changes
            .pop_if(|c| matches!(c, StyleChange::Property(_)))
        {
            match change {
                StyleChange::Property(prop) => {
                    self.apply(prop);
                }
                StyleChange::StackPush => {
                    // We don't need to do anything here, just popping the stack is enough.
                }
            }
        }

        self.changes.pop_if(|c| matches!(c, StyleChange::StackPush));
    }

    fn apply(&mut self, property: StyleProperty) -> StyleProperty {
        use std::mem::swap;

        match property {
            StyleProperty::FontColor(mut color) => {
                swap(&mut self.style.text.font_color, &mut color);
                StyleProperty::FontColor(color)
            }
            StyleProperty::FontSize(mut size) => {
                swap(&mut self.style.text.font_size, &mut size);
                StyleProperty::FontSize(size)
            }
            StyleProperty::FontStyle(mut style) => {
                swap(&mut self.style.text.font_style, &mut style);
                StyleProperty::FontStyle(style)
            }
            StyleProperty::FontWeight(mut weight) => {
                swap(&mut self.style.text.font_weight, &mut weight);
                StyleProperty::FontWeight(weight)
            }
            StyleProperty::FontStack(mut stack) => {
                swap(&mut self.style.text.font, &mut stack);
                StyleProperty::FontStack(stack)
            }
            StyleProperty::StrikethroughColor(mut color) => {
                swap(&mut self.style.text.strikethrough_color, &mut color);
                StyleProperty::StrikethroughColor(color)
            }
            StyleProperty::StrikethroughOffset(mut offset) => {
                swap(&mut self.style.text.strikethrough_offset, &mut offset);
                StyleProperty::StrikethroughOffset(offset)
            }
            StyleProperty::UnderlineColor(mut color) => {
                swap(&mut self.style.text.underline_color, &mut color);
                StyleProperty::UnderlineColor(color)
            }
            StyleProperty::UnderlineOffset(mut color) => {
                swap(&mut self.style.text.underline_offset, &mut color);
                StyleProperty::UnderlineOffset(color)
            }
        }
    }
}

enum StyleChange {
    Property(StyleProperty),
    StackPush,
}
