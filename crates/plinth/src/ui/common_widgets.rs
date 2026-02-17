use crate::graphics::Texture;

use super::Size;
use super::widget::Button;
use super::widget::Container;
use super::widget::Frame;
use super::widget::HorizontalSeparator;
use super::widget::Image;
use super::widget::Interaction;
use super::widget::Label;
use super::widget::Surface;
use super::widget::TextEdit;
use super::widget::VerticalSeparator;

pub trait CommonWidgetsExt<'a>: Container<'a> {
    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn frame<'this>(&'this mut self) -> Frame<'this>
    where
        'a: 'this,
    {
        Frame::new(self.builder_mut())
    }

    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn with_frame(&mut self, callback: impl FnOnce(Frame<'_>)) -> &mut Self {
        let container = self.frame();
        callback(container);
        self
    }

    fn image(&mut self, texture: &Texture, width: Size) {
        Image::new(self.builder_mut(), texture)
            .with_width(width)
            .finish()
    }

    fn surface<'this>(&'this mut self) -> Surface<'this>
    where
        'a: 'this,
    {
        Surface::new(self.builder_mut())
    }

    fn with_surface(&mut self, callback: impl FnOnce(Surface<'_>)) -> &mut Self {
        let panel = self.surface();
        callback(panel);
        self
    }

    fn text_button(&mut self, label: &str) -> Interaction {
        Button::new(self.builder_mut(), Some(label)).finish()
    }

    fn text_edit<'this>(&'this mut self, initial_text: &str, width: f32) -> TextEdit<'this>
    where
        'a: 'this,
    {
        TextEdit::new(self.builder_mut(), Size::Fixed(width)).default_text(initial_text)
    }

    fn label<'this>(&'this mut self, text: &str) -> Label<'this>
    where
        'a: 'this,
    {
        Label::new(self.builder_mut(), text)
    }

    fn horizontal_separator<'this>(&'this mut self) -> HorizontalSeparator<'this>
    where
        'a: 'this,
    {
        HorizontalSeparator::new(self.builder_mut())
    }

    fn vertical_separator<'this>(&'this mut self) -> VerticalSeparator<'this>
    where
        'a: 'this,
    {
        VerticalSeparator::new(self.builder_mut())
    }
}

impl<'a, C: Container<'a>> CommonWidgetsExt<'a> for C {}
