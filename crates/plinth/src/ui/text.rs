use parley::Layout;
use slotmap::SlotMap;
use slotmap::new_key_type;

use crate::graphics::Color;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;

use super::style::StateFlags;
use super::style::StyleId;

new_key_type! {
    pub struct StaticTextLayoutId;
}

pub struct StaticTextLayout {
    pub layout: Layout<Color>,

    // Cache invalidation tracking: relayout when any of these change
    pub style_id: StyleId,
    pub state: StateFlags,
    pub text_hash: u64,
    pub raw_text: String,
    pub prev_width: f32,
    pub prev_alignment: Option<TextAlignment>,
    pub prev_overflow: TextOverflow,

    // Track if line breaking and alignment need to be recomputed
    pub needs_line_break: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLayoutId {
    Static(StaticTextLayoutId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Wrap,
}

pub enum TextLayoutMut<'a> {
    Static(&'a mut Layout<Color>),
}

pub struct TextLayoutStorage {
    static_layouts: SlotMap<StaticTextLayoutId, StaticTextLayout>,
}

impl TextLayoutStorage {
    pub(crate) fn new() -> Self {
        Self {
            static_layouts: SlotMap::with_key(),
        }
    }

    /// Gets an existing static text layout or creates a new one if `layout_id`
    /// is `None`.
    ///
    /// If the `layout_id` is `Some`, this method panics if the ID is not found.
    pub(crate) fn get_or_create_static(
        &mut self,
        layout_id: Option<StaticTextLayoutId>,
    ) -> (StaticTextLayoutId, &mut StaticTextLayout) {
        match layout_id {
            Some(id) => (id, self.static_layouts.get_mut(id).unwrap()),
            None => {
                let layout = StaticTextLayout {
                    layout: parley::Layout::new(),
                    style_id: Default::default(),
                    state: Default::default(),
                    text_hash: 0,
                    raw_text: String::new(),
                    prev_width: 0.0,
                    prev_alignment: None,
                    prev_overflow: TextOverflow::Clip,
                    needs_line_break: true,
                };
                let id = self.static_layouts.insert(layout);
                (id, self.static_layouts.get_mut(id).unwrap())
            }
        }
    }

    pub(crate) fn remove(&mut self, layout_id: TextLayoutId) {
        match layout_id {
            TextLayoutId::Static(id) => {
                self.static_layouts.remove(id);
            }
        }
    }

    pub(crate) fn break_lines(
        &mut self,
        _context: &mut TextLayoutContext,
        layout_id: TextLayoutId,
        max_width: f32,
        alignment: TextAlignment,
        overflow: TextOverflow,
    ) -> Option<f32> {
        match layout_id {
            TextLayoutId::Static(id) => {
                let text = self.static_layouts.get_mut(id)?;

                let width_changed = text.prev_width != max_width;
                let alignment_changed = text.prev_alignment != Some(alignment);
                let overflow_changed = text.prev_overflow != overflow;

                if text.needs_line_break || width_changed || overflow_changed {
                    match overflow {
                        TextOverflow::Clip => {
                            // Keep text on a single line while still producing drawable line data.
                            text.layout.break_all_lines(None);
                        }
                        TextOverflow::Wrap => {
                            text.layout.break_all_lines(Some(max_width));
                        }
                    }
                }

                if text.needs_line_break || width_changed || alignment_changed || overflow_changed {
                    text.layout.align(alignment.into(), Default::default());
                    text.needs_line_break = false;
                    text.prev_width = max_width;
                    text.prev_alignment = Some(alignment);
                    text.prev_overflow = overflow;
                }

                Some(text.layout.height())
            }
        }
    }

    pub(crate) fn get_mut<'a>(&'a mut self, layout_id: TextLayoutId) -> Option<TextLayoutMut<'a>> {
        match layout_id {
            TextLayoutId::Static(id) => self
                .static_layouts
                .get_mut(id)
                .map(|l| TextLayoutMut::Static(&mut l.layout)),
        }
    }
}

impl Default for TextLayoutStorage {
    fn default() -> Self {
        Self::new()
    }
}
