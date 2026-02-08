use parley::Layout;
use parley::PlainEditor;
use slotmap::SlotMap;
use slotmap::new_key_type;

use crate::graphics::Color;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;

use super::style::StateFlags;
use super::style::StyleId;

new_key_type! {
    pub struct StaticTextLayoutId;
    pub struct DynamicTextLayoutId;
    pub struct LargeDynamicTextLayoutId;
}

pub struct StaticTextLayout {
    pub layout: Layout<Color>,

    // Cache invalidation tracking: relayout when any of these change
    pub style_id: StyleId,
    pub state: StateFlags,
    pub text_hash: u64,
    pub prev_width: f32,
    pub prev_alignment: Option<TextAlignment>,

    // Track if line breaking and alignment need to be recomputed
    pub needs_line_break: bool,
}

#[allow(dead_code)]
pub struct DynamicTextLayout {
    pub editor: PlainEditor<Color>,

    // Cached style/state to detect when relayout is needed
    pub prev_width: f32,
    pub prev_alignment: Option<TextAlignment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLayoutId {
    Static(StaticTextLayoutId),
    Dynamic(DynamicTextLayoutId),
    LargeDynamic(LargeDynamicTextLayoutId),
}

pub enum TextLayoutMut<'a> {
    Static(&'a mut Layout<Color>),
    Dynamic(&'a mut DynamicTextLayout),
}

pub(crate) struct TextLayoutStorage {
    static_layouts: SlotMap<StaticTextLayoutId, StaticTextLayout>,
    dynamic_layouts: SlotMap<DynamicTextLayoutId, DynamicTextLayout>,
}

impl TextLayoutStorage {
    pub fn new() -> Self {
        Self {
            static_layouts: SlotMap::with_key(),
            dynamic_layouts: SlotMap::with_key(),
        }
    }

    /// Gets an existing static text layout or creates a new one if `layout_id`
    /// is `None`.
    ///
    /// If the `layout_id` is `Some`, this method panics if the ID is not found.
    pub fn get_or_create_static(
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
                    prev_width: 0.0,
                    prev_alignment: None,
                    needs_line_break: true,
                };
                let id = self.static_layouts.insert(layout);
                (id, self.static_layouts.get_mut(id).unwrap())
            }
        }
    }

    /// Gets an existing dynamic text layout or creates a new one if `layout_id`
    /// is `None`.
    ///
    /// If the `layout_id` is `Some`, this method panics if the ID is not found.
    pub fn get_or_create_dynamic(
        &mut self,
        layout_id: Option<DynamicTextLayoutId>,
    ) -> (DynamicTextLayoutId, &mut DynamicTextLayout) {
        match layout_id {
            Some(id) => (id, self.dynamic_layouts.get_mut(id).unwrap()),
            None => {
                let layout = DynamicTextLayout {
                    editor: PlainEditor::new(14.0),
                    prev_width: 0.0,
                    prev_alignment: None,
                };

                let id = self.dynamic_layouts.insert(layout);
                (id, self.dynamic_layouts.get_mut(id).unwrap())
            }
        }
    }

    pub fn remove(&mut self, layout_id: TextLayoutId) {
        match layout_id {
            TextLayoutId::Static(id) => {
                self.static_layouts.remove(id);
            }
            TextLayoutId::Dynamic(id) => {
                self.dynamic_layouts.remove(id);
            }
            TextLayoutId::LargeDynamic(_) => todo!(),
        }
    }

    pub fn break_lines(
        &mut self,
        context: &mut TextLayoutContext,
        layout_id: TextLayoutId,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32> {
        match layout_id {
            TextLayoutId::Static(id) => {
                let text = self.static_layouts.get_mut(id)?;

                let width_changed = text.prev_width != max_width;
                let alignment_changed = text.prev_alignment != Some(alignment);

                if text.needs_line_break || width_changed {
                    text.layout.break_all_lines(Some(max_width));
                }

                if text.needs_line_break || width_changed || alignment_changed {
                    text.layout
                        .align(Some(max_width), alignment.into(), Default::default());
                    text.needs_line_break = false;
                    text.prev_width = max_width;
                    text.prev_alignment = Some(alignment);
                }

                Some(text.layout.height())
            }
            TextLayoutId::Dynamic(id) => {
                let text = self.dynamic_layouts.get_mut(id)?;

                let width_changed = text.prev_width != max_width;
                if width_changed {
                    text.editor.set_width(Some(max_width));
                }

                let alignment_changed = text.prev_alignment != Some(alignment);
                if alignment_changed {
                    text.editor.set_alignment(alignment.into());
                }

                text.prev_width = max_width;
                text.prev_alignment = Some(alignment);

                let layout = text.editor.layout(&mut context.fonts, &mut context.layouts);

                Some(layout.height())
            }
            TextLayoutId::LargeDynamic(_) => todo!(),
        }
    }

    pub fn get_mut<'a>(&'a mut self, layout_id: TextLayoutId) -> Option<TextLayoutMut<'a>> {
        match layout_id {
            TextLayoutId::Static(id) => self
                .static_layouts
                .get_mut(id)
                .map(|l| TextLayoutMut::Static(&mut l.layout)),
            TextLayoutId::Dynamic(id) => {
                self.dynamic_layouts.get_mut(id).map(TextLayoutMut::Dynamic)
            }
            TextLayoutId::LargeDynamic(_) => todo!(),
        }
    }
}

impl Default for TextLayoutStorage {
    fn default() -> Self {
        Self::new()
    }
}
