use std::cell::RefCell;
use std::rc::Rc;

use parley::Layout;
use rapidhash::v3::rapidhash_v3;
use slotmap::SlotMap;
use slotmap::new_key_type;

use crate::graphics::Canvas;
use crate::graphics::ClipRect;
use crate::graphics::Color;
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

new_key_type! {
    pub(crate) struct StaticTextLayoutId;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Wrap,
}

#[derive(Clone, Default)]
pub struct TextServices {
    inner: Rc<RefCell<TextServicesInner>>,
}

struct TextServicesInner {
    context: TextLayoutContext,
    static_layouts: SlotMap<StaticTextLayoutId, StaticTextLayout>,
    #[cfg(test)]
    static_text_rebuild_count: usize,
}

impl Default for TextServicesInner {
    fn default() -> Self {
        Self {
            context: TextLayoutContext::default(),
            static_layouts: SlotMap::with_key(),
            #[cfg(test)]
            static_text_rebuild_count: 0,
        }
    }
}

struct StaticTextLayout {
    layout: Layout<Color>,
    style_id: StyleId,
    state: StateFlags,
    text_hash: u64,
    prev_width: f32,
    prev_alignment: Option<TextAlignment>,
    prev_overflow: TextOverflow,
    needs_line_break: bool,
}

impl StaticTextLayout {
    fn new() -> Self {
        Self {
            layout: Layout::new(),
            style_id: StyleId::default(),
            state: StateFlags::default(),
            text_hash: 0,
            prev_width: 0.0,
            prev_alignment: None,
            prev_overflow: TextOverflow::Clip,
            needs_line_break: true,
        }
    }
}

impl TextServices {
    pub(crate) fn with_context<R>(&self, f: impl FnOnce(&mut TextLayoutContext) -> R) -> R {
        f(&mut self.inner.borrow_mut().context)
    }

    pub(crate) fn create_static_text_layout(&self) -> StaticTextLayoutId {
        self.inner
            .borrow_mut()
            .static_layouts
            .insert(StaticTextLayout::new())
    }

    pub(crate) fn remove_static_text_layout(&self, id: StaticTextLayoutId) {
        self.inner.borrow_mut().static_layouts.remove(id);
    }

    pub(crate) fn prepare_static_text_layout(
        &self,
        id: StaticTextLayoutId,
        theme: &Theme,
        style_id: StyleId,
        state: StateFlags,
        text: &str,
    ) -> super::Size {
        let text_hash = rapidhash_v3(text.as_bytes());
        let inner = &mut *self.inner.borrow_mut();
        let TextServicesInner {
            context,
            static_layouts,
            #[cfg(test)]
            static_text_rebuild_count,
        } = inner;
        let layout = static_layouts.get_mut(id).unwrap();

        let needs_rebuild =
            layout.style_id != style_id || layout.state != state || layout.text_hash != text_hash;

        if needs_rebuild {
            let mut builder = context
                .layouts
                .ranged_builder(&mut context.fonts, text, 1.0, false);
            theme.push_text_defaults(style_id, state, &mut builder);
            builder.build_into(&mut layout.layout, text);

            layout.style_id = style_id;
            layout.state = state;
            layout.text_hash = text_hash;
            layout.needs_line_break = true;
            #[cfg(test)]
            {
                *static_text_rebuild_count += 1;
            }
        }

        let size = layout.layout.calculate_content_widths();
        super::Flex {
            min: size.min,
            max: size.max,
        }
    }

    pub(crate) fn measure_static_text_layout(
        &self,
        id: StaticTextLayoutId,
        max_width: f32,
        alignment: TextAlignment,
        overflow: TextOverflow,
    ) -> Option<f32> {
        let mut inner = self.inner.borrow_mut();
        let layout = inner.static_layouts.get_mut(id)?;

        let width_changed = layout.prev_width != max_width;
        let alignment_changed = layout.prev_alignment != Some(alignment);
        let overflow_changed = layout.prev_overflow != overflow;

        if layout.needs_line_break || width_changed || overflow_changed {
            match overflow {
                TextOverflow::Clip => {
                    layout.layout.break_all_lines(None);
                }
                TextOverflow::Wrap => {
                    layout.layout.break_all_lines(Some(max_width));
                }
            }
        }

        if layout.needs_line_break || width_changed || alignment_changed || overflow_changed {
            layout
                .layout
                .align(Some(max_width), alignment.into(), Default::default());
            layout.needs_line_break = false;
            layout.prev_width = max_width;
            layout.prev_alignment = Some(alignment);
            layout.prev_overflow = overflow;
        }

        Some(layout.layout.height())
    }

    pub(crate) fn draw_static_text_layout(
        &self,
        id: StaticTextLayoutId,
        canvas: &mut Canvas,
        origin: [f32; 2],
        clip: ClipRect,
    ) {
        if let Some(layout) = self.inner.borrow().static_layouts.get(id) {
            canvas.draw_text_layout(&layout.layout, origin, clip);
        }
    }

    #[cfg(test)]
    pub(crate) fn static_text_layout_count(&self) -> usize {
        self.inner.borrow().static_layouts.len()
    }

    #[cfg(test)]
    pub(crate) fn static_text_rebuild_count(&self) -> usize {
        self.inner.borrow().static_text_rebuild_count
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

    #[test]
    fn plain_editable_text_exposes_raw_text_without_allocation() {
        let text = PlainEditableText::new();
        text.set_text("hello");

        let len = text.with_raw_text(str::len);

        assert_eq!(len, 5);
    }
}
