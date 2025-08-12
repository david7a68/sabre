use std::num::NonZeroU32;
use std::time::Duration;

use arrayvec::ArrayVec;
use graphics::Color;
use graphics::Primitive;
use smallvec::SmallVec;

use crate::input::InputState;
use crate::layout::Alignment;
use crate::layout::Flex;
use crate::layout::LayoutDirection;
use crate::layout::LayoutInfo;
use crate::layout::LayoutNodeResult;
use crate::layout::LayoutNodeSpec;
use crate::layout::MeasureText;
use crate::layout::Padding;
use crate::layout::Size;
use crate::layout::compute_layout;
use crate::text::TextAlignment;
use crate::text::TextLayoutContext;
use crate::text::TextStyle;

#[derive(Default)]
pub(crate) struct UiContext {
    input: InputState,
    time_delta: Duration,

    ui_nodes: Vec<UiNode>,

    children: Vec<NodeIndexArray>,

    text_layouts: TextLayoutPool,
}

impl UiContext {
    pub fn begin_frame<'a>(
        &'a mut self,
        text_context: &'a mut TextLayoutContext,
        input: InputState,
        time_delta: Duration,
    ) -> UiBuilder<'a> {
        self.ui_nodes.clear();
        self.children.clear();
        self.text_layouts.clear();

        // Set up the root node.
        self.ui_nodes.push(UiNode {
            color: Color::WHITE,
            layout_text: None,
            layout_spec: LayoutNodeSpec {
                width: input.window_size.width.into(),
                height: input.window_size.height.into(),
                ..Default::default()
            },
            layout_result: LayoutNodeResult::default(),
        });
        self.children.push(NodeIndexArray::new());

        self.input = input;
        self.time_delta = time_delta;

        UiBuilder {
            index: 0,
            context: self,
            text_context,
        }
    }

    pub fn finish(&mut self) -> impl Iterator<Item = DrawCommand<'_>> {
        compute_layout(
            &mut self.text_layouts,
            &mut self.ui_nodes,
            &self.children,
            UiElementId(0),
        );

        self.ui_nodes
            .iter()
            .filter_map(|node| {
                let layout = &node.layout_result;

                if layout.width == 0.0 || layout.height == 0.0 {
                    return None; // Skip empty nodes.
                }

                let mut vec = ArrayVec::<_, 2>::new();

                if node.color != Color::default() {
                    vec.push(DrawCommand::Primitive(Primitive::new(
                        layout.x,
                        layout.y,
                        layout.width,
                        layout.height,
                        node.color,
                    )));
                }

                if let Some(text_id) = node.layout_text
                    && let Some(text_layout) = self.text_layouts.get(text_id)
                {
                    vec.push(DrawCommand::TextLayout(text_layout, [layout.x, layout.y]));
                }

                Some(vec.into_iter())
            })
            .flatten()
    }
}

pub struct UiBuilder<'a> {
    index: usize,
    context: &'a mut UiContext,
    text_context: &'a mut TextLayoutContext,
}

impl UiBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.ui_nodes[self.index].color = color.into();
        self
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.width = width.into();
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.height = height.into();
        self
    }

    pub fn child_major_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.major_align = alignment;
        self
    }

    pub fn child_minor_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.minor_align = alignment;
        self
    }

    pub fn child_alignment(&mut self, major: Alignment, minor: Alignment) -> &mut Self {
        let node = &mut self.context.ui_nodes[self.index];
        node.layout_spec.major_align = major;
        node.layout_spec.minor_align = minor;
        self
    }

    pub fn child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.direction = direction;
        self
    }

    pub fn child_spacing(&mut self, spacing: f32) -> &mut Self {
        self.context.ui_nodes[self.index]
            .layout_spec
            .inter_child_padding = spacing;
        self
    }

    pub fn padding(&mut self, padding: Padding) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.inner_padding = padding;
        self
    }

    pub fn rect(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: impl Into<Color>,
    ) -> &mut Self {
        let node = self.add(self.index);

        let content = &mut self.context.ui_nodes[node];
        content.color = color.into();
        content.layout_spec.width = width.into();
        content.layout_spec.height = height.into();

        self
    }

    pub fn label(
        &mut self,
        text: &str,
        style: &TextStyle,
        height: impl Into<Size>,
        background_color: impl Into<Color>,
    ) -> &mut Self {
        let (id, layout, alignment) = self.context.text_layouts.allocate();

        *alignment = style.align;
        let mut compute =
            self.text_context
                .layouts
                .ranged_builder(&mut self.text_context.fonts, text, 1.0, true);

        style.as_defaults(&mut compute);
        compute.build_into(layout, text);

        let size = layout.calculate_content_widths();

        let node = self.add(self.index);
        let content = &mut self.context.ui_nodes[node];
        content.color = background_color.into();
        content.layout_text = Some(id);
        content.layout_spec.width = Flex {
            min: size.min,
            max: size.max,
        };
        content.layout_spec.height = height.into();

        self
    }

    pub fn container(&mut self) -> UiBuilder {
        let child_index = self.add(self.index);

        UiBuilder {
            context: self.context,
            index: child_index,
            text_context: self.text_context,
        }
    }

    pub fn with_container(&mut self, callback: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        callback(&mut self.container());
        self
    }

    fn add(&mut self, parent: usize) -> usize {
        let child_index = self.context.ui_nodes.len();

        self.context.children[parent].push(UiElementId(child_index as u16));

        self.context.ui_nodes.push(UiNode {
            ..Default::default()
        });

        self.context.children.push(NodeIndexArray::new());

        child_index
    }
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum DrawCommand<'a> {
    Primitive(Primitive),
    TextLayout(&'a parley::Layout<Color>, [f32; 2]),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct UiElementId(pub(crate) u16);

pub(crate) type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Default, Debug)]
struct UiNode {
    color: Color,

    layout_text: Option<TextLayoutId>,
    layout_spec: LayoutNodeSpec,
    layout_result: LayoutNodeResult,
}

impl LayoutInfo for UiNode {
    fn spec(&self) -> &LayoutNodeSpec {
        &self.layout_spec
    }

    fn spec_mut(&mut self) -> &mut LayoutNodeSpec {
        &mut self.layout_spec
    }

    fn result(&self) -> &LayoutNodeResult {
        &self.layout_result
    }

    fn result_mut(&mut self) -> &mut LayoutNodeResult {
        &mut self.layout_result
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TextLayoutId {
    index: u32,
    version: NonZeroU32,
}

struct TextLayoutPoolEntry {
    version: NonZeroU32,
    next: Option<u32>,
    layout: parley::Layout<Color>,
    alignment: TextAlignment,
}

/// Pooling generational allocator for text layouts.
///
/// This isn't meant to be used for larger text blocks because the layouts can
/// be fairly memory-intensive and we don't deallocate them, meaning that they
/// hang around for what could conceivably be the lifetime of the program.
#[derive(Default)]
struct TextLayoutPool {
    entries: Vec<TextLayoutPoolEntry>,
    first_free: Option<u32>,
}

impl TextLayoutPool {
    fn clear(&mut self) {
        self.entries.clear();
        self.first_free = None;
    }

    fn get(&self, id: TextLayoutId) -> Option<&parley::Layout<Color>> {
        let entry = self.entries.get(id.index as usize)?;
        if entry.version == id.version && entry.next.is_none() {
            Some(&entry.layout)
        } else {
            None
        }
    }

    fn get_mut(&mut self, id: TextLayoutId) -> Option<&mut TextLayoutPoolEntry> {
        let entry = self.entries.get_mut(id.index as usize)?;
        if entry.version == id.version && entry.next.is_none() {
            Some(entry)
        } else {
            None
        }
    }

    fn allocate(&mut self) -> (TextLayoutId, &mut parley::Layout<Color>, &mut TextAlignment) {
        let (index, entry) = if let Some(index) = self.first_free.take() {
            let entry = &mut self.entries[index as usize];
            self.first_free = entry.next.take();
            (index, entry)
        } else {
            let index = self.entries.len() as u32;
            self.entries.push(TextLayoutPoolEntry {
                version: NonZeroU32::MIN,
                next: None,
                layout: parley::Layout::new(),
                alignment: TextAlignment::Start,
            });

            (index, self.entries.last_mut().unwrap())
        };

        (
            TextLayoutId {
                index,
                version: entry.version,
            },
            &mut entry.layout,
            &mut entry.alignment,
        )
    }

    #[expect(unused)]
    fn free(&mut self, id: TextLayoutId) {
        let Some(entry) = self.entries.get_mut(id.index as usize) else {
            return; // Invalid ID, nothing to free
        };

        if entry.version != id.version {
            return; // Version mismatch, cannot free
        }

        entry.next = self.first_free;

        // If the index has wrapped around (4 billion entries) and an old ID
        // wraps around to be valid again, it's up to you.
        entry.version = entry
            .version
            .checked_add(1)
            .unwrap_or(NonZeroU32::new(1).unwrap());

        self.first_free = Some(id.index);
    }
}

impl MeasureText<UiNode> for TextLayoutPool {
    fn break_lines(&mut self, node: &UiNode, max_width: f32) -> Option<f32> {
        let text_id = node.layout_text?;
        let entry = self.get_mut(text_id)?;
        entry.layout.break_all_lines(Some(max_width));
        entry.layout.align(
            Some(max_width),
            entry.alignment.into(),
            parley::AlignmentOptions::default(),
        );

        Some(entry.layout.height())
    }
}
