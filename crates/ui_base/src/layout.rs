use tracing::debug;
use tracing::info;
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
    pub alignment: Alignment,
    pub direction: LayoutDirection,
    pub inner_padding: Padding,
    pub inter_child_padding: f32,
}

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeResult {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub(crate) trait LayoutInfo {
    fn spec(&self) -> &LayoutNodeSpec;
    fn result(&self) -> &LayoutNodeResult;
    fn result_mut(&mut self) -> &mut LayoutNodeResult;
    fn parent_idx(&self) -> Option<UiElementId>;
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
        compute_fixed_sizes(nodes, children, node_id);
        compute_major_axis_flex_sizes::<D, T>(nodes, children, node_id);
        compute_minor_axis_flex_sizes::<D, T>(nodes, children, node_id);
        compute_major_axis_offsets::<D, T>(nodes, children, node_id, 0.0);
        compute_minor_axis_offsets::<D, T>(nodes, children, node_id, 0.0);
    }

    match nodes[node_id.0 as usize].spec().direction {
        LayoutDirection::Horizontal => compute::<HorizontalMode, T>(nodes, children, node_id),
        LayoutDirection::Vertical => compute::<VerticalMode, T>(nodes, children, node_id),
    }
}

fn compute_fixed_sizes<T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &mut nodes[node_id.0 as usize];

    if let Size::Fixed(w) = node.spec().width {
        node.result_mut().width = Some(w)
    };

    if let Size::Fixed(h) = node.spec().height {
        node.result_mut().height = Some(h)
    };

    for child in &children[node_id.0 as usize] {
        compute_fixed_sizes(nodes, children, *child);
    }
}

fn compute_major_axis_flex_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if !(node.spec().direction == D::DIRECTION) {
        return compute_minor_axis_flex_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let empty_size = node.spec().inter_child_padding
        * (children[node_id.0 as usize].len().saturating_sub(1)) as f32
        + D::major_axis_padding_start(node)
        + D::major_axis_padding_end(node);

    if let Size::Fit { min, max } = D::major_size_spec(node) {
        let effective_max = if max >= f32::MAX {
            find_parent_constraint::<D, T>(nodes, children, node_id).unwrap_or(f32::MAX)
        } else {
            max
        };

        let (total_size, _) = compute_major_axis_children_with_limit::<D, T>(
            nodes,
            children,
            node_id,
            empty_size,
            effective_max,
        );

        D::set_major_size(
            &mut nodes[node_id.0 as usize],
            total_size.clamp(min, effective_max.min(max)),
        );
    } else if let Some(size) = D::major_size_result(node) {
        compute_major_axis_children_with_limit::<D, T>(nodes, children, node_id, empty_size, size);
    } else {
        warn!(
            "Node {node_id:?} has no {} size set and is not a flex container",
            D::string()
        );
        D::set_major_size(&mut nodes[node_id.0 as usize], 0.0);
    }
}

fn compute_major_axis_children_with_limit<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    empty_size: f32,
    size_limit: f32,
) -> (f32, bool) {
    let mut total_size = empty_size;
    let mut grow_children = NodeIndexArray::new();

    // Calculate sizes for non-grow children and collect grow children
    for child in &children[node_id.0 as usize] {
        let child_node = &nodes[child.0 as usize];
        if let Some(size) = D::major_size_result(child_node) {
            total_size += size;
        } else if matches!(D::major_size_spec(child_node), Grow) {
            grow_children.push(*child);
        } else {
            compute_major_axis_flex_sizes::<D, T>(nodes, children, *child);
            total_size += D::major_size_result(&nodes[child.0 as usize]).unwrap();
        }
    }

    let has_grow_children = !grow_children.is_empty();

    // Handle grow children if any
    if has_grow_children {
        let effective_limit = if size_limit >= f32::MAX {
            total_size
        } else {
            size_limit
        };

        let grow_size = (effective_limit - total_size).max(0.0) / grow_children.len() as f32;

        for child in &grow_children {
            D::set_major_size(&mut nodes[child.0 as usize], grow_size);
        }

        total_size = effective_limit;
    }

    (total_size, has_grow_children)
}

#[instrument(skip(nodes, children), fields(direction = D::string()))]
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

    let size = D::major_size_result(node).unwrap();
    let padding = node.spec().inter_child_padding;

    let mut advance = current_offset + D::major_axis_padding_start(node);
    for child_id in &children[node_id.0 as usize] {
        D::set_major_offset(&mut nodes[child_id.0 as usize], advance);
        advance = compute_major_axis_offsets::<D, T>(nodes, children, *child_id, advance) + padding;
    }

    current_offset + size
}

fn compute_minor_axis_flex_sizes<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &mut [T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if node.spec().direction != D::DIRECTION {
        return compute_major_axis_flex_sizes::<D::Other, T>(nodes, children, node_id);
    }

    let mut total_size = 0.0f32;

    if let Size::Fit { min, max } = D::minor_size_spec(node) {
        for child in &children[node_id.0 as usize] {
            let child_size = if let Some(size) = D::minor_size_result(&nodes[child.0 as usize]) {
                size
            } else {
                compute_minor_axis_flex_sizes::<D, T>(nodes, children, *child);
                D::minor_size_result(&nodes[child.0 as usize]).unwrap()
            };

            total_size = total_size.max(child_size);
        }

        let node = &mut nodes[node_id.0 as usize];

        total_size += D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);
        D::set_minor_size(node, total_size.clamp(min, max));
    } else {
        for child in &children[node_id.0 as usize] {
            compute_minor_axis_flex_sizes::<D, T>(nodes, children, *child);
        }
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

    if node.spec().direction != D::DIRECTION {
        return compute_major_axis_offsets::<D::Other, T>(nodes, children, node_id, current_offset);
    }

    D::set_minor_offset(node, current_offset);

    let size = D::minor_size_result(node).unwrap();
    let inset = current_offset + D::minor_axis_padding_start(node);

    for child_id in &children[node_id.0 as usize] {
        D::set_minor_offset(&mut nodes[child_id.0 as usize], inset);
        compute_minor_axis_offsets::<D, T>(nodes, children, *child_id, inset);
    }

    inset + size
}

fn find_parent_constraint<D: LayoutDirectionExt, T: LayoutInfo>(
    nodes: &[T],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> Option<f32> {
    let parent_id = nodes[node_id.0 as usize].parent_idx()?;
    let parent_node = &nodes[parent_id.0 as usize];

    // Determine the parent's available size in the major axis
    D::major_size_result(parent_node).or_else(|| {
        if let Size::Fit { max, .. } = D::major_size_spec(parent_node)
            && max < f32::MAX
        {
            let available_space = max
                - D::major_axis_padding_start(parent_node)
                - D::major_axis_padding_end(parent_node)
                - parent_node.spec().inter_child_padding
                    * children[parent_id.0 as usize].len() as f32;

            Some(available_space.max(0.0))
        } else {
            None
        }
    })
}

trait LayoutDirectionExt {
    type Other: LayoutDirectionExt;
    const DIRECTION: LayoutDirection;

    fn string() -> &'static str;

    fn major_size_spec<T: LayoutInfo>(node: &T) -> Size;
    fn minor_size_spec<T: LayoutInfo>(node: &T) -> Size;

    fn set_major_size<T: LayoutInfo>(node: &mut T, size: f32);
    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32);

    fn major_size_result<T: LayoutInfo>(node: &T) -> Option<f32>;
    fn minor_size_result<T: LayoutInfo>(node: &T) -> Option<f32>;

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
        node.result_mut().width = Some(size);
    }

    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().height = Some(size);
    }

    fn major_size_result<T: LayoutInfo>(node: &T) -> Option<f32> {
        node.result().width
    }

    fn minor_size_result<T: LayoutInfo>(node: &T) -> Option<f32> {
        node.result().height
    }

    fn set_major_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().x = Some(offset);
    }

    fn set_minor_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().y = Some(offset);
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
        node.result_mut().height = Some(size);
    }

    fn set_minor_size<T: LayoutInfo>(node: &mut T, size: f32) {
        node.result_mut().width = Some(size);
    }

    fn major_size_result<T: LayoutInfo>(node: &T) -> Option<f32> {
        node.result().height
    }

    fn minor_size_result<T: LayoutInfo>(node: &T) -> Option<f32> {
        node.result().width
    }

    fn set_major_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().y = Some(offset);
    }

    fn set_minor_offset<T: LayoutInfo>(node: &mut T, offset: f32) {
        node.result_mut().x = Some(offset);
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
