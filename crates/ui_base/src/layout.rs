use crate::ui::NodeIndexArray;
use crate::ui::UiElementId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    Fit { min: f32, max: f32 },
    Grow,
}

pub use Size::*;
use tracing::info;
use tracing::warn;

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

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeSpec {
    pub width: Size,
    pub height: Size,
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

#[derive(Debug, Default)]
pub(crate) struct LayoutNode {
    pub spec: LayoutNodeSpec,
    pub result: LayoutNodeResult,
    pub parent_idx: Option<UiElementId>,
}

pub(crate) fn compute_layout(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    fn compute<D: LayoutDirectionExt>(
        nodes: &mut [LayoutNode],
        children: &[NodeIndexArray],
        node_id: UiElementId,
    ) {
        compute_fixed_sizes(nodes, children, node_id);
        compute_major_axis_flex_sizes::<D>(nodes, children, node_id);
        compute_minor_axis_flex_sizes::<D>(nodes, children, node_id);
        compute_major_axis_offsets::<D>(nodes, children, node_id, 0.0);
        compute_minor_axis_offsets::<D>(nodes, children, node_id, 0.0);
    }

    match nodes[node_id.0 as usize].spec.direction {
        LayoutDirection::Horizontal => {
            info!("Computing horizontal root layout");
            compute::<HorizontalMode>(nodes, children, node_id)
        }
        LayoutDirection::Vertical => {
            info!("Computing vertical root layout");
            compute::<VerticalMode>(nodes, children, node_id)
        }
    }
}

fn compute_fixed_sizes(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &mut nodes[node_id.0 as usize];

    if let Size::Fixed(w) = node.spec.width {
        node.result.width = Some(w)
    };

    if let Size::Fixed(h) = node.spec.height {
        node.result.height = Some(h)
    };

    for child in &children[node_id.0 as usize] {
        compute_fixed_sizes(nodes, children, *child);
    }
}

fn compute_major_axis_flex_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if !(node.spec.direction == D::DIRECTION) {
        return compute_minor_axis_flex_sizes::<D::Other>(nodes, children, node_id);
    }

    // this is now the total width of all fixed children and required padding
    //
    // Out here on its lonesome to avoid borrowing `node` for the entire closure
    let empty_size = node.spec.inter_child_padding
        * (children[node_id.0 as usize].len().saturating_sub(1)) as f32
        + D::major_axis_padding_start(node)
        + D::major_axis_padding_end(node);

    let run = |nodes: &mut [LayoutNode], limit: f32| {
        let mut total_size = empty_size;
        let mut grow_children = NodeIndexArray::new();

        for child in &children[node_id.0 as usize] {
            let child_node = &nodes[child.0 as usize];
            if let Some(size) = D::major_size_result(child_node) {
                total_size += size;
            } else if matches!(D::major_size_spec(child_node), Grow) {
                grow_children.push(*child);
            } else {
                compute_major_axis_flex_sizes::<D>(nodes, children, *child);
                let child_node = &nodes[child.0 as usize];
                total_size += D::major_size_result(child_node).unwrap();
            }
        }

        if !grow_children.is_empty() {
            info!("there are {} grow children", grow_children.len());

            let grow_size = (limit - total_size).max(0.0) / grow_children.len() as f32;

            for child in &grow_children {
                let child_node = &mut nodes[child.0 as usize];
                D::set_major_size(child_node, grow_size);
            }

            total_size = limit;
        }

        total_size
    };

    if let Size::Fit { min, max } = D::major_size_spec(node) {
        // Handle the case where max is f32::MAX (infinite) by calculating the actual needed space
        let effective_max = if max >= f32::MAX {
            // For infinite max, first calculate the space needed for non-grow children
            let mut fixed_size = empty_size;
            let mut has_grow_children = false;

            for child in &children[node_id.0 as usize] {
                let child_node = &nodes[child.0 as usize];
                if let Some(size) = D::major_size_result(child_node) {
                    fixed_size += size;
                } else if matches!(D::major_size_spec(child_node), Grow) {
                    has_grow_children = true;
                } else {
                    // Compute child first to get its size
                    compute_major_axis_flex_sizes::<D>(nodes, children, *child);
                    let child_node = &nodes[child.0 as usize];
                    fixed_size += D::major_size_result(child_node).unwrap();
                }
            }

            // If we have grow children and a parent with known size, use the parent size
            if has_grow_children {
                if let Some(parent_size) = find_parent_constraint::<D>(nodes, children, node_id) {
                    parent_size
                } else {
                    // No parent constraint, use the fixed size (grow children get 0)
                    fixed_size
                }
            } else {
                // No grow children, just use the calculated fixed size
                fixed_size
            }
        } else {
            max
        };

        let total_size = run(nodes, effective_max);
        let node = &mut nodes[node_id.0 as usize];
        info!(
            "node: {node_id:?}, axis: {}, mode: fit, size: {total_size}",
            D::string()
        );
        D::set_major_size(node, total_size.clamp(min, effective_max.min(max)));
    } else if let Some(size) = D::major_size_result(node) {
        info!(
            "node: {node_id:?}, axis: {}, mode: fixed, size: {size}",
            D::string()
        );
        run(nodes, size);
    } else {
        warn!(
            "Node {node_id:?} has no {} size set and is not a flex container",
            D::string()
        );
        let node = &mut nodes[node_id.0 as usize];
        D::set_major_size(node, 0.0);
    }
}

fn compute_major_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    if node.spec.direction != D::DIRECTION {
        return compute_minor_axis_offsets::<D::Other>(nodes, children, node_id, current_offset);
    }

    D::set_major_offset(node, current_offset);

    let size = D::major_size_result(node).unwrap();
    let padding = node.spec.inter_child_padding;

    let mut advance = current_offset + D::major_axis_padding_start(node);
    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        D::set_major_offset(child_node, advance);
        advance = compute_major_axis_offsets::<D>(nodes, children, *child_id, advance) + padding;
    }

    current_offset + size
}

fn compute_minor_axis_flex_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    if node.spec.direction != D::DIRECTION {
        return compute_major_axis_flex_sizes::<D::Other>(nodes, children, node_id);
    }

    let mut total_size = 0.0f32;

    if let Size::Fit { min, max } = D::minor_size_spec(node) {
        for child in &children[node_id.0 as usize] {
            let child_node = &nodes[child.0 as usize];
            let child_size = if let Some(h) = D::minor_size_result(child_node) {
                h
            } else {
                compute_minor_axis_flex_sizes::<D>(nodes, children, *child);
                let child_node = &nodes[child.0 as usize];
                D::minor_size_result(child_node).unwrap()
            };

            total_size = total_size.max(child_size);
        }

        let node = &mut nodes[node_id.0 as usize];

        total_size += D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);

        D::set_minor_size(node, total_size.clamp(min, max));
    } else {
        for child in &children[node_id.0 as usize] {
            compute_minor_axis_flex_sizes::<D>(nodes, children, *child);
        }
    }
}

fn compute_minor_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    if node.spec.direction != D::DIRECTION {
        info!("redirect, offset: {current_offset}");
        return compute_major_axis_offsets::<D::Other>(nodes, children, node_id, current_offset);
    }

    D::set_minor_offset(node, current_offset);

    let size = D::minor_size_result(node).unwrap();
    let inset = current_offset + D::minor_axis_padding_start(node);

    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        D::set_minor_offset(child_node, inset);
        compute_minor_axis_offsets::<D>(nodes, children, *child_id, inset);
    }

    inset + size
}

fn find_parent_constraint<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) -> Option<f32> {
    let node = &nodes[node_id.0 as usize];

    if let Some(parent_id) = node.parent_idx {
        let parent_node = &nodes[parent_id.0 as usize];
        let parent_children = &children[parent_id.0 as usize];

        // Determine the parent's available size in the major axis
        let parent_major_size =
            D::major_size_result(parent_node).or_else(|| match D::major_size_spec(parent_node) {
                Size::Fit { max, .. } if max < f32::MAX => Some(max),
                _ => None,
            });

        if let Some(parent_size) = parent_major_size {
            // Subtract padding to get available space for children
            let available_space = parent_size
                - D::major_axis_padding_start(parent_node)
                - D::major_axis_padding_end(parent_node)
                - parent_node.spec.inter_child_padding
                    * (parent_children.len().saturating_sub(1)) as f32;
            return Some(available_space.max(0.0));
        }
    }

    None
}

trait LayoutDirectionExt {
    type Other: LayoutDirectionExt;
    const DIRECTION: LayoutDirection;

    fn string() -> &'static str;

    fn major_size_spec(node: &LayoutNode) -> Size;
    fn minor_size_spec(node: &LayoutNode) -> Size;

    fn set_major_size(node: &mut LayoutNode, size: f32);
    fn set_minor_size(node: &mut LayoutNode, size: f32);

    fn major_size_result(node: &LayoutNode) -> Option<f32>;
    fn minor_size_result(node: &LayoutNode) -> Option<f32>;

    fn set_major_offset(node: &mut LayoutNode, offset: f32);
    fn set_minor_offset(node: &mut LayoutNode, offset: f32);

    fn major_axis_padding_start(node: &LayoutNode) -> f32;
    fn major_axis_padding_end(node: &LayoutNode) -> f32;

    fn minor_axis_padding_start(node: &LayoutNode) -> f32;
    fn minor_axis_padding_end(node: &LayoutNode) -> f32;
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

    fn major_size_spec(node: &LayoutNode) -> Size {
        node.spec.width
    }

    fn minor_size_spec(node: &LayoutNode) -> Size {
        node.spec.height
    }

    fn set_major_size(node: &mut LayoutNode, size: f32) {
        node.result.width = Some(size);
    }

    fn set_minor_size(node: &mut LayoutNode, size: f32) {
        node.result.height = Some(size);
    }

    fn major_size_result(node: &LayoutNode) -> Option<f32> {
        node.result.width
    }

    fn minor_size_result(node: &LayoutNode) -> Option<f32> {
        node.result.height
    }

    fn set_major_offset(node: &mut LayoutNode, offset: f32) {
        node.result.x = Some(offset);
    }

    fn set_minor_offset(node: &mut LayoutNode, offset: f32) {
        node.result.y = Some(offset);
    }

    fn major_axis_padding_start(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.left
    }

    fn major_axis_padding_end(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.right
    }

    fn minor_axis_padding_start(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.top
    }

    fn minor_axis_padding_end(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.bottom
    }
}

struct VerticalMode;

impl LayoutDirectionExt for VerticalMode {
    type Other = HorizontalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Vertical;

    fn string() -> &'static str {
        "vertical"
    }

    fn major_size_spec(node: &LayoutNode) -> Size {
        node.spec.height
    }

    fn minor_size_spec(node: &LayoutNode) -> Size {
        node.spec.width
    }

    fn set_major_size(node: &mut LayoutNode, size: f32) {
        node.result.height = Some(size);
    }

    fn set_minor_size(node: &mut LayoutNode, size: f32) {
        node.result.width = Some(size);
    }

    fn major_size_result(node: &LayoutNode) -> Option<f32> {
        node.result.height
    }

    fn minor_size_result(node: &LayoutNode) -> Option<f32> {
        node.result.width
    }

    fn set_major_offset(node: &mut LayoutNode, offset: f32) {
        node.result.y = Some(offset);
    }

    fn set_minor_offset(node: &mut LayoutNode, offset: f32) {
        node.result.x = Some(offset);
    }

    fn major_axis_padding_start(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.top
    }

    fn major_axis_padding_end(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.bottom
    }

    fn minor_axis_padding_start(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.left
    }

    fn minor_axis_padding_end(node: &LayoutNode) -> f32 {
        node.spec.inner_padding.right
    }
}

impl std::fmt::Debug for VerticalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vertical")
    }
}
