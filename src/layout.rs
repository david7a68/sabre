use graphics::Color;
use smallvec::SmallVec;

use crate::text::TextAlignment;
pub use Size::*;

/// Single-dimension size for UI elements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    Fit { min: f32, max: f32 },
    Grow,
    Flex { min: f32, max: f32 },
}

impl From<f32> for Size {
    fn from(value: f32) -> Self {
        Size::Fixed(value)
    }
}

impl Default for Size {
    fn default() -> Self {
        Size::Fit {
            min: 0.0,
            max: f32::MAX,
        }
    }
}

impl From<Option<Size>> for Size {
    fn from(size: Option<Size>) -> Self {
        size.unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Padding {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Padding {
    pub fn equal(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayoutDirection {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Justify,
}

#[derive(Debug, Default)]
pub(crate) struct Atom {
    pub width: Size,
    pub height: Size,
    pub inner_padding: Padding,

    // Container layout properties
    pub major_align: Alignment,
    pub minor_align: Alignment,
    pub direction: LayoutDirection,
    pub inter_child_padding: f32,
}

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeResult {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct UiElementId(pub(crate) u16);

type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Default, Debug)]
pub(crate) struct LayoutNode {
    pub atom: Atom,
    pub result: LayoutNodeResult,
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum LayoutNodeContent {
    None,
    Fill {
        color: Color,
    },
    Text {
        layout: parley::Layout<Color>,
        alignment: TextAlignment,
    },
}

impl From<Option<LayoutNodeContent>> for LayoutNodeContent {
    fn from(content: Option<LayoutNodeContent>) -> Self {
        content.unwrap_or(LayoutNodeContent::None)
    }
}

pub(crate) struct LayoutTree<T> {
    nodes: Vec<LayoutNode>,
    children: Vec<NodeIndexArray>,

    /// Content associated with nodes, such as text layouts.
    ///
    /// These are stored separately under the assumption that they occur much
    /// less frequently than the nodes themselves, and that they are often much
    /// larger in size.
    content: Vec<(LayoutNodeContent, Option<T>)>,
}

impl<T> Default for LayoutTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LayoutTree<T> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            children: Vec::new(),
            content: Vec::new(),
        }
    }

    pub fn iter_nodes(
        &self,
    ) -> impl Iterator<Item = (&LayoutNode, &LayoutNodeContent, Option<&T>)> {
        self.nodes
            .iter()
            .zip(
                self.content
                    .iter()
                    .map(|(content, t)| (content, t.as_ref())),
            )
            .map(|(node, (content, reference))| (node, content, reference))
    }

    pub fn atom_mut(&mut self, node: UiElementId) -> &mut Atom {
        &mut self.nodes[node.0 as usize].atom
    }

    pub fn content_mut(&mut self, node: UiElementId) -> &mut LayoutNodeContent {
        &mut self.content[node.0 as usize].0
    }

    pub fn add(
        &mut self,
        parent: Option<UiElementId>,
        atom: Atom,
        content: impl Into<LayoutNodeContent>,
        reference: Option<T>,
    ) -> UiElementId {
        let node_id = UiElementId(self.nodes.len() as u16);

        let node = LayoutNode {
            atom,
            result: LayoutNodeResult::default(),
        };

        self.nodes.push(node);
        self.content.push((content.into(), reference));
        self.children.push(NodeIndexArray::new());

        if let Some(parent_id) = parent {
            self.children[parent_id.0 as usize].push(node_id);
        }

        node_id
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.children.clear();
    }

    pub fn compute_layout(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        let nodes: &mut [LayoutNode] = &mut self.nodes;
        let node_id = UiElementId(0);

        debug_assert_eq!(
            nodes[node_id.0 as usize].atom.direction,
            LayoutDirection::Horizontal,
            "The root node must have a horizontal layout direction"
        );

        compute_major_axis_fit_sizes::<HorizontalMode>(nodes, &self.children, node_id);
        compute_major_axis_grow_sizes::<HorizontalMode>(nodes, &self.children, node_id);

        compute_text_heights(
            &mut |node_id, max_width| {
                let LayoutNodeContent::Text { layout, alignment } =
                    &mut self.content[node_id.0 as usize].0
                else {
                    return None;
                };

                layout.break_all_lines(Some(max_width));
                layout.align(Some(max_width), (*alignment).into(), Default::default());

                Some(layout.height())
            },
            nodes,
        );

        compute_minor_axis_fit_sizes::<HorizontalMode>(nodes, &self.children, node_id);
        compute_minor_axis_grow_sizes::<HorizontalMode>(nodes, &self.children, node_id);

        compute_major_axis_offsets::<HorizontalMode>(nodes, &self.children, node_id, 0.0);
        compute_minor_axis_offsets::<HorizontalMode>(nodes, &self.children, node_id, 0.0);
    }
}

fn compute_major_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> f32 {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_fit_sizes::<D::Other>(nodes, children, node_id);
    }

    let size_spec = D::major_size_spec(node);

    let child_sizes = {
        let mut total_size = get_major_axis_empty_size::<D>(node, node_children);

        for child_id in node_children {
            total_size += compute_major_axis_fit_sizes::<D>(nodes, children, *child_id);
        }

        total_size
    };

    let size = match size_spec {
        Size::Fixed(size) => size,
        Size::Fit { min, max } => child_sizes.clamp(min, max),
        Size::Flex { max, .. } => max,
        Size::Grow => {
            // Grow is handled in the offsets phase
            0.0
        }
    };

    D::set_major_size(&mut nodes[node_id.0 as usize], size);

    size
}

fn compute_major_axis_grow_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_grow_sizes::<D::Other>(nodes, children, node_id);
    }

    let mut grow_children = NodeIndexArray::new();
    let mut remaining_size =
        D::major_size_result(node) - get_major_axis_empty_size::<D>(node, node_children);

    // Step 1: Find all the children that can grow and the amount of space they
    // can take up.
    for child_id in node_children {
        let child = &nodes[child_id.0 as usize];

        let child_size = D::major_size_result(child);
        remaining_size -= child_size;

        match D::major_size_spec(child) {
            Fixed(_) | Fit { .. } => {} // already computed
            Flex { .. } | Grow => grow_children.push(*child_id),
        }
    }

    // Step 2: Distribute the remaining size evenly among the grow children.
    while remaining_size.abs() > 0.5 && !grow_children.is_empty() {
        let distributed_size = remaining_size / grow_children.len() as f32;

        // For each grow child, distribute the available grow size evenly
        // between all of them, unless it exceeds their max size. If that
        // happens, continue to distribute the unallocated size subsequent
        // iterations.
        grow_children.retain(|child_id| {
            let child = &mut nodes[child_id.0 as usize];
            let child_size = D::major_size_result(child);

            match D::major_size_spec(child) {
                Fixed(_) | Fit { .. } => {
                    // Fixed and fit containers do not grow, so we can skip them.
                    false
                }
                Flex { max, .. } => {
                    let tentative_size = child_size + distributed_size;

                    let (is_done, actual_size) = if tentative_size > max {
                        (true, max)
                    } else {
                        (false, tentative_size)
                    };

                    D::set_major_size(child, actual_size);
                    remaining_size -= actual_size - child_size;

                    // Stop growing the child if it has reached its max size
                    !is_done
                }
                Grow if remaining_size > 0.0 => {
                    D::set_major_size(child, child_size + distributed_size);
                    remaining_size -= distributed_size;

                    // Grow children are always considered to have space
                    true
                }
                Grow => false,
            }
        });
    }

    // Step 3: Call recursively for each child.
    for child_id in node_children {
        compute_major_axis_grow_sizes::<D>(nodes, children, *child_id);
    }
}

fn compute_major_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    if node.atom.direction != D::DIRECTION {
        return compute_minor_axis_offsets::<D::Other>(nodes, children, node_id, current_offset);
    }

    D::set_major_offset(node, current_offset);

    let size = D::major_size_result(node);

    let padding_start = D::major_axis_padding_start(node);
    let padding_internal = node.atom.inter_child_padding;
    let padding_end = D::major_axis_padding_end(node);

    let node_children = &children[node_id.0 as usize];
    match node.atom.major_align {
        Alignment::Start => {
            let mut advance = current_offset + padding_start;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::Center => {
            // start with all the reserved space for padding
            let mut content_size = get_major_axis_empty_size::<D>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            let half_unused_space = ((size - content_size) / 2.0).round();

            let mut advance = current_offset + padding_start + half_unused_space;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::End => {
            // start with all the reserved space for padding from the end (without the start padding)
            let mut content_size = padding_end + get_inter_child_padding::<D>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            let mut advance = current_offset + size - content_size;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::Justify if node_children.len() > 1 => {
            // start with all the reserved space for padding
            let mut content_size = get_major_axis_empty_size::<D>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            // The amount to pad between children, valuing at least the
            // configured inter-child padding
            let internal_padding =
                padding_internal.max((size - content_size) / (node_children.len() - 1) as f32);

            let mut advance = current_offset + padding_start;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance)
                    + internal_padding;
            }
        }
        Alignment::Justify => {
            // Justified layouts with a single child are treated as start-aligned.
            let mut advance = current_offset + padding_start;

            for child_id in node_children {
                advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
    }

    current_offset + size
}

fn compute_text_heights(
    measure_text: &mut impl FnMut(UiElementId, f32) -> Option<f32>,
    nodes: &mut [LayoutNode],
) {
    for (id, node) in nodes.iter_mut().enumerate() {
        let id = UiElementId(id as u16);

        let Some(text_height) = measure_text(id, node.result.width) else {
            continue;
        };

        node.atom.height = match node.atom.height {
            Fixed(height) => Fixed(height),
            Fit { min, max } => Fixed(text_height.clamp(min, max)),
            Grow => Grow,
            Flex { min, max } => Flex {
                min: text_height.clamp(min, max),
                max,
            },
        };
    }
}

fn compute_minor_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> f32 {
    let node = &nodes[node_id.0 as usize];

    if node.atom.direction != D::DIRECTION {
        return compute_major_axis_fit_sizes::<D::Other>(nodes, children, node_id);
    }

    let size_spec = D::minor_size_spec(node);
    let size_padding = D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);

    let child_sizes = {
        let mut total_size = 0.0f32;

        for child in &children[node_id.0 as usize] {
            let child_size = compute_minor_axis_fit_sizes::<D>(nodes, children, *child);
            total_size = total_size.max(child_size);
        }

        total_size
    };

    let size = match size_spec {
        Fixed(size) => size,
        Fit { min, max } => (child_sizes + size_padding).clamp(min, max),
        Flex { max, .. } => max,
        Grow => 0.0, // Grow is handled later
    };

    D::set_minor_size(&mut nodes[node_id.0 as usize], size);
    size
}

fn compute_minor_axis_grow_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_major_axis_grow_sizes::<D::Other>(nodes, children, node_id);
    }

    let remaining_size = D::minor_size_result(node)
        - (D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node));

    for child_id in &children[node_id.0 as usize] {
        let child = &mut nodes[child_id.0 as usize];

        if matches!(D::minor_size_spec(child), Grow) {
            D::set_minor_size(child, remaining_size);
        }

        compute_minor_axis_grow_sizes::<D>(nodes, children, *child_id);
    }
}

fn compute_minor_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if node.atom.direction != D::DIRECTION {
        return compute_major_axis_offsets::<D::Other>(nodes, children, node_id, current_offset);
    }

    D::set_minor_offset(node, current_offset);

    let size = D::minor_size_result(node);
    let padding_start = D::minor_axis_padding_start(node);
    let padding_end = D::minor_axis_padding_end(node);

    match node.atom.minor_align {
        // Justified layouts don't make sense in the minor axis, so we treat
        // them as start-aligned.
        Alignment::Start | Alignment::Justify => {
            let inset = current_offset + padding_start;

            for child_id in node_children {
                compute_minor_axis_offsets::<D>(nodes, children, *child_id, inset);
            }
        }
        Alignment::Center => {
            // Center on a per-child basis.
            for child_id in node_children {
                let child = &mut nodes[child_id.0 as usize];
                let child_size = D::minor_size_result(child);

                // Ignore the padding for centering since the child has already
                // been sized appropriately.
                let inset = (current_offset + (size - child_size).max(0.0) / 2.0).round();

                compute_minor_axis_offsets::<D>(nodes, children, *child_id, inset);
            }
        }
        Alignment::End => {
            for child_id in node_children {
                let child = &mut nodes[child_id.0 as usize];
                let child_size = D::minor_size_result(child);

                let inset = current_offset + (size - child_size - padding_end).max(0.0);

                compute_minor_axis_offsets::<D>(nodes, children, *child_id, inset);
            }
        }
    }

    current_offset + size
}

fn get_inter_child_padding<D: LayoutDirectionExt>(
    node: &LayoutNode,
    children: &NodeIndexArray,
) -> f32 {
    node.atom.inter_child_padding * (children.len().saturating_sub(1)) as f32
}

fn get_major_axis_empty_size<D: LayoutDirectionExt>(
    node: &LayoutNode,
    children: &NodeIndexArray,
) -> f32 {
    get_inter_child_padding::<D>(node, children)
        + D::major_axis_padding_start(node)
        + D::major_axis_padding_end(node)
}

trait LayoutDirectionExt {
    type Other: LayoutDirectionExt;
    const DIRECTION: LayoutDirection;

    fn major_size_spec(node: &LayoutNode) -> Size;
    fn minor_size_spec(node: &LayoutNode) -> Size;

    fn set_major_size(node: &mut LayoutNode, size: f32);
    fn set_minor_size(node: &mut LayoutNode, size: f32);

    fn major_size_result(node: &LayoutNode) -> f32;
    fn minor_size_result(node: &LayoutNode) -> f32;

    fn set_major_offset(node: &mut LayoutNode, offset: f32);
    fn set_minor_offset(node: &mut LayoutNode, offset: f32);

    fn major_axis_padding_start(node: &LayoutNode) -> f32;
    fn major_axis_padding_end(node: &LayoutNode) -> f32;

    fn minor_axis_padding_start(node: &LayoutNode) -> f32;
    fn minor_axis_padding_end(node: &LayoutNode) -> f32;
}

struct HorizontalMode;

impl LayoutDirectionExt for HorizontalMode {
    type Other = VerticalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Horizontal;

    fn major_size_spec(node: &LayoutNode) -> Size {
        node.atom.width
    }

    fn minor_size_spec(node: &LayoutNode) -> Size {
        node.atom.height
    }

    fn set_major_size(node: &mut LayoutNode, size: f32) {
        node.result.width = size;
    }

    fn set_minor_size(node: &mut LayoutNode, size: f32) {
        node.result.height = size;
    }

    fn major_size_result(node: &LayoutNode) -> f32 {
        node.result.width
    }

    fn minor_size_result(node: &LayoutNode) -> f32 {
        node.result.height
    }

    fn set_major_offset(node: &mut LayoutNode, offset: f32) {
        node.result.x = offset;
    }

    fn set_minor_offset(node: &mut LayoutNode, offset: f32) {
        node.result.y = offset;
    }

    fn major_axis_padding_start(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.left
    }

    fn major_axis_padding_end(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.right
    }

    fn minor_axis_padding_start(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.top
    }

    fn minor_axis_padding_end(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.bottom
    }
}

struct VerticalMode;

impl LayoutDirectionExt for VerticalMode {
    type Other = HorizontalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Vertical;

    fn major_size_spec(node: &LayoutNode) -> Size {
        node.atom.height
    }

    fn minor_size_spec(node: &LayoutNode) -> Size {
        node.atom.width
    }

    fn set_major_size(node: &mut LayoutNode, size: f32) {
        node.result.height = size;
    }

    fn set_minor_size(node: &mut LayoutNode, size: f32) {
        node.result.width = size;
    }

    fn major_size_result(node: &LayoutNode) -> f32 {
        node.result.height
    }

    fn minor_size_result(node: &LayoutNode) -> f32 {
        node.result.width
    }

    fn set_major_offset(node: &mut LayoutNode, offset: f32) {
        node.result.y = offset;
    }

    fn set_minor_offset(node: &mut LayoutNode, offset: f32) {
        node.result.x = offset;
    }

    fn major_axis_padding_start(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.top
    }

    fn major_axis_padding_end(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.bottom
    }

    fn minor_axis_padding_start(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.left
    }

    fn minor_axis_padding_end(node: &LayoutNode) -> f32 {
        node.atom.inner_padding.right
    }
}
