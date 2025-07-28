use tracing::instrument;
use tracing::warn;

use crate::ui::NodeIndexArray;
use crate::ui::UiElementId;

pub use Size::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    Fit { min: f32, max: f32 },
    Grow,
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
pub(crate) struct LayoutNodeSpec {
    pub width: Size,
    pub height: Size,
    pub major_align: Alignment,
    pub minor_align: Alignment,
    pub direction: LayoutDirection,
    pub inner_padding: Padding,
    pub inter_child_padding: f32,
}

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeResult {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub(crate) trait LayoutInfo {
    fn spec(&self) -> &LayoutNodeSpec;
    fn result(&self) -> &LayoutNodeResult;
    fn result_mut(&mut self) -> &mut LayoutNodeResult;
}

pub(crate) fn compute_layout<T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    fn compute<D: LayoutDirectionExt, T: LayoutInfo>(
        nodes: &mut [T],
        children: &[NodeIndexArray],
        node_id: UiElementId,
    ) {
        compute_major_axis_fit_sizes::<D, T>(nodes, children, node_id);
        compute_major_axis_grow_sizes::<D, T>(nodes, children, node_id);
        compute_minor_axis_fit_sizes::<D, T>(nodes, children, node_id);
        compute_minor_axis_grow_sizes::<D, T>(nodes, children, node_id);
        compute_major_axis_offsets::<D, T>(nodes, children, node_id, 0.0);
        compute_minor_axis_offsets::<D, T>(nodes, children, node_id, 0.0);
    }

    match nodes[node_id.0 as usize].spec().direction {
        LayoutDirection::Horizontal => compute::<HorizontalMode, T>(nodes, children, node_id),
        LayoutDirection::Vertical => compute::<VerticalMode, T>(nodes, children, node_id),
    }
}

fn compute_major_axis_fit_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> f32 {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.spec().direction == D::DIRECTION) {
        return compute_minor_axis_fit_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let size_spec = D::major_size_spec(node);

    let child_sizes = {
        let mut total_size = get_major_axis_empty_size::<D, T>(node, node_children);

        for child_id in node_children {
            total_size += compute_major_axis_fit_sizes::<D, T>(nodes, children, *child_id);
        }

        total_size
    };

    let size = match size_spec {
        Size::Fixed(size) => size,
        Size::Fit { min, max } => child_sizes.clamp(min, max),
        Size::Grow => {
            // Grow is handled in the offsets phase
            0.0
        }
    };

    D::set_major_size(&mut nodes[node_id.0 as usize], size);
    size
}

fn compute_major_axis_grow_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.spec().direction == D::DIRECTION) {
        return compute_minor_axis_grow_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let mut grow_children = NodeIndexArray::new();
    let mut remaining_size =
        D::major_size_result(node) - get_major_axis_empty_size::<D, T>(node, node_children);

    // Step 1: Find all the children that can grow and the amount of space they
    // can take up.
    for child_id in node_children {
        let child = &nodes[child_id.0 as usize];

        let child_size = D::major_size_result(child);
        remaining_size -= child_size;

        match D::major_size_spec(child) {
            Fixed(_) | Fit { .. } => {} // already computed
            Grow => grow_children.push(*child_id),
        }
    }

    // Step 2: Distribute the remaining size evenly among the grow children.
    while remaining_size > 0.0 && !grow_children.is_empty() {
        let even_size = remaining_size / grow_children.len() as f32;

        // For each grow child, distribute the available grow size evenly
        // between all of them, unless it exceeds their max size. If that
        // happens, continue to distribute the unallocated size subsequent
        // iterations.
        grow_children.retain(|child_id| {
            let child = &mut nodes[child_id.0 as usize];
            let child_size = D::major_size_result(child);

            match D::major_size_spec(child) {
                Fixed(_) => unreachable!(),
                Fit { max, .. } => {
                    let tentative_size = child_size + even_size;

                    let (is_done, actual_size) = if tentative_size > max {
                        (true, max)
                    } else {
                        (false, child_size + even_size)
                    };

                    D::set_major_size(child, actual_size);
                    remaining_size -= actual_size - child_size;

                    // Stop growing the child if it has reached its max size
                    is_done
                }
                Grow => {
                    D::set_major_size(child, child_size + even_size);
                    remaining_size -= even_size;

                    // Grow children are always considered to have space
                    true
                }
            }
        });
    }

    // Step 3: Call recursively for each child.
    for child_id in node_children {
        compute_major_axis_grow_sizes::<D, T>(nodes, children, *child_id);
    }
}

fn compute_major_axis_offsets<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    if node.spec().direction != D::DIRECTION {
        return compute_minor_axis_offsets::<D::Other, T>(nodes, children, node_id, current_offset);
    }

    D::set_major_offset(node, current_offset);

    let size = D::major_size_result(node);

    let padding_start = D::major_axis_padding_start(node);
    let padding_internal = node.spec().inter_child_padding;
    let padding_end = D::major_axis_padding_end(node);

    let node_children = &children[node_id.0 as usize];
    match node.spec().major_align {
        Alignment::Start => {
            let mut advance = current_offset + padding_start;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::Center => {
            // start with all the reserved space for padding
            let mut content_size = get_major_axis_empty_size::<D, T>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            let half_unused_space = (size - content_size) / 2.0;

            let mut advance = current_offset + padding_start + half_unused_space;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::End => {
            // start with all the reserved space for padding from the end (without the start padding)
            let mut content_size =
                padding_end + get_inter_child_padding::<D, T>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            let mut advance = current_offset + size - content_size;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
        Alignment::Justify if node_children.len() > 1 => {
            // start with all the reserved space for padding
            let mut content_size = get_major_axis_empty_size::<D, T>(node, node_children);

            for child_id in node_children {
                content_size += D::major_size_result(&nodes[child_id.0 as usize]);
            }

            // The amount to pad between children, valuing at least the
            // configured inter-child padding
            let internal_padding =
                padding_internal.max((size - content_size) / (node_children.len() - 1) as f32);

            let mut advance = current_offset + padding_start;
            for child_id in node_children {
                advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance)
                    + internal_padding;
            }
        }
        Alignment::Justify => {
            // Justified layouts with a single child are treated as start-aligned.
            let mut advance = current_offset + padding_start;

            for child_id in node_children {
                advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance)
                    + padding_internal;
            }
        }
    }

    current_offset + size
}

fn compute_minor_axis_fit_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> f32 {
    let node = &nodes[node_id.0 as usize];

    if node.spec().direction != D::DIRECTION {
        return compute_major_axis_fit_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let size_spec = D::minor_size_spec(node);
    let size_padding = D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);

    let child_sizes = {
        let mut total_size = 0.0f32;

        for child in &children[node_id.0 as usize] {
            let child_size = compute_minor_axis_fit_sizes::<D, T>(nodes, children, *child);
            total_size = total_size.max(child_size);
        }

        total_size
    };

    let size = match size_spec {
        Fixed(size) => size,
        Fit { min, max } => (child_sizes + size_padding).clamp(min, max),
        Grow => 0.0, // Grow is handled later
    };

    D::set_minor_size(&mut nodes[node_id.0 as usize], size);
    size
}

fn compute_minor_axis_grow_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if !(node.spec().direction == D::DIRECTION) {
        return compute_major_axis_grow_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let remaining_size = D::minor_size_result(node)
        - (D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node));

    for child_id in &children[node_id.0 as usize] {
        let child = &mut nodes[child_id.0 as usize];

        if matches!(D::minor_size_spec(child), Grow) {
            D::set_minor_size(child, remaining_size);
        }

        compute_minor_axis_grow_sizes::<D, T>(nodes, children, *child_id);
    }
}

#[instrument(skip(nodes, children), fields(direction = D::string()))]
fn compute_minor_axis_offsets<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if node.spec().direction != D::DIRECTION {
        return compute_major_axis_offsets::<D::Other, T>(nodes, children, node_id, current_offset);
    }

    D::set_minor_offset(node, current_offset);

    let size = D::minor_size_result(node);
    let padding_start = D::minor_axis_padding_start(node);
    let padding_end = D::minor_axis_padding_end(node);

    match node.spec().minor_align {
        // Justified layouts don't make sense in the minor axis, so we treat
        // them as start-aligned.
        Alignment::Start | Alignment::Justify => {
            let inset = current_offset + padding_start;

            for child_id in node_children {
                compute_minor_axis_offsets::<D, T>(nodes, children, *child_id, inset);
            }
        }
        Alignment::Center => {
            // Center on a per-child basis.
            for child_id in node_children {
                let child = &mut nodes[child_id.0 as usize];
                let child_size = D::minor_size_result(child);

                // Ignore the padding for centering since the child has already
                // been sized appropriately.
                let inset = current_offset + (size - child_size).max(0.0) / 2.0;

                compute_minor_axis_offsets::<D, T>(nodes, children, *child_id, inset);
            }
        }
        Alignment::End => {
            for child_id in node_children {
                let child = &mut nodes[child_id.0 as usize];
                let child_size = D::minor_size_result(child);

                let inset = current_offset + (size - child_size - padding_end).max(0.0);

                compute_minor_axis_offsets::<D, T>(nodes, children, *child_id, inset);
            }
        }
    }

    current_offset + size
}

fn get_inter_child_padding<D: LayoutDirectionExt, T: LayoutInfo>(
    node: &T,
    children: &NodeIndexArray,
) -> f32 {
    node.spec().inter_child_padding * (children.len().saturating_sub(1)) as f32
}

fn get_major_axis_empty_size<D: LayoutDirectionExt, T: LayoutInfo>(
    node: &T,
    children: &NodeIndexArray,
) -> f32 {
    get_inter_child_padding::<D, T>(node, children)
        + D::major_axis_padding_start(node)
        + D::major_axis_padding_end(node)
}

trait LayoutDirectionExt {
    type Other: LayoutDirectionExt;
    const DIRECTION: LayoutDirection;

    fn string() -> &'static str;

    fn major_size_spec<T: LayoutInfo>(node: &T) -> Size;
    fn minor_size_spec<T: LayoutInfo>(node: &T) -> Size;

    fn set_major_size<T: LayoutInfo>(node: &mut T, size: f32);
    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32);

    fn major_size_result<T: LayoutInfo>(node: &T) -> f32;
    fn minor_size_result<T: LayoutInfo>(node: &T) -> f32;

    fn set_major_offset<T: LayoutInfo>(node: &mut T, offset: f32);
    fn set_minor_offset<T: LayoutInfo>(node: &mut T, offset: f32);

    fn major_axis_padding_start<T: LayoutInfo>(node: &T) -> f32;
    fn major_axis_padding_end<T: LayoutInfo>(node: &T) -> f32;

    fn minor_axis_padding_start<T: LayoutInfo>(node: &T) -> f32;
    fn minor_axis_padding_end<T: LayoutInfo>(node: &T) -> f32;
}

struct HorizontalMode;

impl std::fmt::Debug for HorizontalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "horizontal")
    }
}

impl LayoutDirectionExt for HorizontalMode {
    type Other = VerticalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Horizontal;

    fn string() -> &'static str {
        "horizontal"
    }

    fn major_size_spec<T: LayoutInfo>(node: &T) -> Size {
        node.spec().width
    }

    fn minor_size_spec<T: LayoutInfo>(node: &T) -> Size {
        node.spec().height
    }

    fn set_major_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().width = size;
    }

    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().height = size;
    }

    fn major_size_result<T: LayoutInfo>(node: &T) -> f32 {
        node.result().width
    }

    fn minor_size_result<T: LayoutInfo>(node: &T) -> f32 {
        node.result().height
    }

    fn set_major_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().x = offset;
    }

    fn set_minor_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().y = offset;
    }

    fn major_axis_padding_start<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.left
    }

    fn major_axis_padding_end<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.right
    }

    fn minor_axis_padding_start<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.top
    }

    fn minor_axis_padding_end<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.bottom
    }
}

struct VerticalMode;

impl LayoutDirectionExt for VerticalMode {
    type Other = HorizontalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Vertical;

    fn string() -> &'static str {
        "vertical"
    }

    fn major_size_spec<T: LayoutInfo>(node: &T) -> Size {
        node.spec().height
    }

    fn minor_size_spec<T: LayoutInfo>(node: &T) -> Size {
        node.spec().width
    }

    fn set_major_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().height = size;
    }

    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().width = size;
    }

    fn major_size_result<T: LayoutInfo>(node: &T) -> f32 {
        node.result().height
    }

    fn minor_size_result<T: LayoutInfo>(node: &T) -> f32 {
        node.result().width
    }

    fn set_major_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().y = offset;
    }

    fn set_minor_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().x = offset;
    }

    fn major_axis_padding_start<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.top
    }

    fn major_axis_padding_end<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.bottom
    }

    fn minor_axis_padding_start<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.left
    }

    fn minor_axis_padding_end<T: LayoutInfo>(node: &T) -> f32 {
        node.spec().inner_padding.right
    }
}

impl std::fmt::Debug for VerticalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vertical")
    }
}
