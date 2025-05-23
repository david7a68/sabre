use crate::ui::Node;
use crate::ui::NodeIndexArray;
use crate::ui::UiElementId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    MinMax { min: f32, max: f32 },
}

pub use Size::*;
use tracing::debug;

impl From<f32> for Size {
    fn from(value: f32) -> Self {
        Size::Fixed(value)
    }
}

impl Default for Size {
    fn default() -> Self {
        Size::MinMax {
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

#[derive(Debug, Default)]
pub(crate) struct Layout {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub(crate) fn compute_layout(
    nodes: &mut [Node],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    debug!(?nodes);
    debug!(?children);

    compute_fixed_sizes(nodes, children, node_id);

    compute_flex_widths(nodes, children, node_id);

    debug!(nodes = ?nodes.iter().map(|n| &n.layout).collect::<Vec<_>>());

    compute_x_offsets(nodes, children, node_id, 0.0);

    compute_flex_heights(nodes, children, node_id);

    compute_y_offsets(nodes, children, node_id, 0.0);

    debug!(?nodes);
}

fn compute_fixed_sizes(nodes: &mut [Node], children: &[NodeIndexArray], node_id: UiElementId) {
    let node = &mut nodes[node_id.0 as usize];

    if let Size::Fixed(w) = node.element.width {
        node.layout.width = Some(w)
    };

    if let Size::Fixed(h) = node.element.height {
        node.layout.height = Some(h)
    };

    for child in &children[node_id.0 as usize] {
        compute_fixed_sizes(nodes, children, *child);
    }
}

fn compute_flex_widths(nodes: &mut [Node], children: &[NodeIndexArray], node_id: UiElementId) {
    let node = &nodes[node_id.0 as usize];

    let mut total_width = 0.0;

    if let Size::MinMax { min, max } = node.element.width {
        for child in &children[node_id.0 as usize] {
            if let Some(w) = nodes[child.0 as usize].layout.width {
                total_width += w;
            } else {
                compute_flex_widths(nodes, children, *child);
                total_width += nodes[child.0 as usize].layout.width.unwrap();
            }
        }

        let node = &mut nodes[node_id.0 as usize];

        total_width += node.element.inter_child_padding
            * (children[node_id.0 as usize].len().saturating_sub(1)) as f32
            + node.element.inner_padding.left
            + node.element.inner_padding.right;

        node.layout.width = Some(total_width.clamp(min, max));
    } else {
        for child in &children[node_id.0 as usize] {
            compute_flex_widths(nodes, children, *child);
        }
    }
}

fn compute_x_offsets(
    nodes: &mut [Node],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_x: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    node.layout.x = Some(current_x);

    let width = node.layout.width.unwrap();
    let padding = node.element.inter_child_padding;

    let mut advance = current_x + node.element.inner_padding.left;
    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        child_node.layout.x = Some(advance);
        advance = compute_x_offsets(nodes, children, *child_id, advance) + padding;
    }

    current_x + width
}

fn compute_flex_heights(nodes: &mut [Node], children: &[NodeIndexArray], node_id: UiElementId) {
    let node = &nodes[node_id.0 as usize];

    let mut total_height = 0.0f32;

    if let Size::MinMax { min, max } = node.element.height {
        for child in &children[node_id.0 as usize] {
            let child_height = if let Some(w) = nodes[child.0 as usize].layout.height {
                w
            } else {
                compute_flex_heights(nodes, children, *child);
                nodes[child.0 as usize].layout.height.unwrap()
            };

            total_height = total_height.max(child_height);
        }

        let node = &mut nodes[node_id.0 as usize];

        total_height += node.element.inner_padding.top + node.element.inner_padding.bottom;

        node.layout.height = Some(total_height.clamp(min, max));
    } else {
        for child in &children[node_id.0 as usize] {
            compute_flex_heights(nodes, children, *child);
        }
    }
}

fn compute_y_offsets(
    nodes: &mut [Node],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_y: f32,
) {
    let node = &mut nodes[node_id.0 as usize];

    node.layout.y = Some(current_y);
    let y_inset = current_y + node.element.inner_padding.top;

    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        child_node.layout.y = Some(y_inset);
        compute_y_offsets(nodes, children, *child_id, y_inset);
    }
}
