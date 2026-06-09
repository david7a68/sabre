use std::cell::RefCell;
use std::rc::Rc;

use crate::graphics::Canvas;
use crate::graphics::ClipRect;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;

use super::context::LayoutRect;
use super::style::StateFlags;
use super::style::StyleId;
use super::theme::Theme;

mod editable;
mod non_editable;

pub(crate) use editable::EditableTextContent;
pub use editable::PlainEditableText;
pub use editable::TextEditCommand;
pub use editable::TextEditPaint;
pub use non_editable::TextBuilderExt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Wrap,
}

#[derive(Clone, Default)]
pub struct TextServices {
    inner: Rc<RefCell<TextLayoutContext>>,
}

impl TextServices {
    pub(crate) fn with_context<R>(&self, f: impl FnOnce(&mut TextLayoutContext) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

pub trait EditableText {
    fn handle(&self) -> EditableTextHandle;
}

pub type EditableTextHandle = Rc<RefCell<dyn EditableTextState>>;

pub trait EditableTextState {
    fn raw_text(&self) -> &str;
    fn set_text(&mut self, text: &str);
    fn selected_text(&self) -> Option<&str>;
    fn is_composing(&self) -> bool;
    fn apply_style(&mut self, theme: &Theme, style_id: StyleId, state: StateFlags)
    -> TextEditPaint;
    fn command(&mut self, services: &TextServices, command: TextEditCommand<'_>);
    fn measure(&mut self, services: &TextServices, max_width: f32, alignment: TextAlignment)
    -> f32;
    fn draw(
        &mut self,
        services: &TextServices,
        canvas: &mut Canvas,
        rect: LayoutRect,
        clip: ClipRect,
        paint: TextEditPaint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_editable_text_applies_edit_commands() {
        let services = TextServices::default();
        let text = PlainEditableText::new();
        let handle = text.handle();

        handle.borrow_mut().command(
            &services,
            TextEditCommand::InsertOrReplaceSelection("hello"),
        );
        assert_eq!(text.raw_text(), "hello");

        handle
            .borrow_mut()
            .command(&services, TextEditCommand::SelectAll);
        assert_eq!(text.selected_text().as_deref(), Some("hello"));

        handle
            .borrow_mut()
            .command(&services, TextEditCommand::InsertOrReplaceSelection("bye"));
        assert_eq!(text.raw_text(), "bye");
    }
}
