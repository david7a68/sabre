use parley::Layout;
use parley::PlainEditor;
use slotmap::SlotMap;
use slotmap::new_key_type;

use crate::graphics::Color;
use crate::graphics::TextAlignment;

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
    pub state_flags: StateFlags,
    pub text_hash: u64,
    pub prev_width: f32,
    pub prev_alignment: Option<TextAlignment>,

    // Track if line breaking and alignment need to be recomputed
    pub needs_line_break: bool,
}

#[allow(dead_code)]
pub struct DynamicTextLayout {
    pub editor: PlainEditor<Color>,
    pub layout: Layout<Color>,

    // Cached style/state to detect when relayout is needed
    pub style_id: StyleId,
    pub state_flags: StateFlags,
    pub prev_width: f32,
    pub styles_dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLayoutId {
    Static(StaticTextLayoutId),
    Dynamic(DynamicTextLayoutId),
    LargeDynamic(LargeDynamicTextLayoutId),
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
                    state_flags: Default::default(),
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
                    layout: Layout::new(),
                    style_id: Default::default(),
                    state_flags: Default::default(),
                    prev_width: 0.0,
                    styles_dirty: true,
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
        layout_id: TextLayoutId,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32> {
        match layout_id {
            TextLayoutId::Static(id) => {
                let layout = self.static_layouts.get_mut(id)?;

                let width_changed = layout.prev_width != max_width;
                let alignment_changed = layout.prev_alignment != Some(alignment);

                if layout.needs_line_break || width_changed {
                    layout.layout.break_all_lines(Some(max_width));
                }

                if layout.needs_line_break || width_changed || alignment_changed {
                    layout
                        .layout
                        .align(Some(max_width), alignment.into(), Default::default());
                    layout.needs_line_break = false;
                    layout.prev_width = max_width;
                    layout.prev_alignment = Some(alignment);
                }

                Some(layout.layout.height())
            }
            TextLayoutId::Dynamic(id) => {
                let layout = self.dynamic_layouts.get_mut(id)?;

                let _width_changed = layout.prev_width != max_width;

                // PlainEditor needs contexts - this will be handled elsewhere
                // For now, just return a placeholder
                layout.prev_width = max_width;

                // TODO: Properly update PlainEditor width and get height
                Some(100.0)
            }
            TextLayoutId::LargeDynamic(_) => todo!(),
        }
    }

    pub fn get_layout(&self, layout_id: TextLayoutId) -> Option<&parley::Layout<Color>> {
        match layout_id {
            TextLayoutId::Static(id) => self.static_layouts.get(id).map(|l| &l.layout),
            TextLayoutId::Dynamic(id) => self.dynamic_layouts.get(id).map(|l| &l.layout),
            TextLayoutId::LargeDynamic(_) => todo!(),
        }
    }
}

impl Default for TextLayoutStorage {
    fn default() -> Self {
        Self::new()
    }
}
