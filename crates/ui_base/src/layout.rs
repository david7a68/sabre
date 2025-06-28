use crate::ui::NodeIndexArray;
use crate::ui::UiElementId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    Fit { min: f32, max: f32 },
    Grow,
}

pub use Size::*;

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

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeSpec {
    pub width: Size,
    pub height: Size,
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
}

pub(crate) fn compute_layout(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    compute_fixed_sizes(nodes, children, node_id);

    compute_flex_widths(nodes, children, node_id);

    compute_x_offsets(nodes, children, node_id, 0.0);

    compute_flex_heights(nodes, children, node_id);

    compute_y_offsets(nodes, children, node_id, 0.0);
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

fn compute_flex_widths(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];
    let inter_child_padding = node.spec.inter_child_padding;
    let inner_padding = node.spec.inner_padding;

    // this is now the total width of all fixed children and required padding
    let mut total_width = inter_child_padding
        * (children[node_id.0 as usize].len().saturating_sub(1)) as f32
        + inner_padding.left
        + inner_padding.right;

    if let Size::Fit { min, max } = node.spec.width {
        let mut grow_children = NodeIndexArray::new();

        for child in &children[node_id.0 as usize] {
            if let Some(w) = nodes[child.0 as usize].result.width {
                total_width += w;
            } else if matches!(nodes[child.0 as usize].spec.width, Grow) {
                grow_children.push(*child);
            } else {
                compute_flex_widths(nodes, children, *child);
                total_width += nodes[child.0 as usize].result.width.unwrap();
            }
        }

        // compute the width for children with grow sizing
        if !grow_children.is_empty() {
            let grow_width = (max - total_width).max(0.0) / grow_children.len() as f32;

            for child in &grow_children {
                let child_node = &mut nodes[child.0 as usize];
                child_node.result.width = Some(grow_width);
            }

            total_width = max;
        }

        let node = &mut nodes[node_id.0 as usize];
        node.result.width = Some(total_width.clamp(min, max));
    } else if let Some(width) = node.result.width {
        let mut grow_children = NodeIndexArray::new();

        for child in &children[node_id.0 as usize] {
            if matches!(nodes[child.0 as usize].spec.width, Grow) {
                grow_children.push(*child);
            } else {
                compute_flex_widths(nodes, children, *child);
            }

            total_width += nodes[child.0 as usize].result.width.unwrap_or_default();
        }

        if !grow_children.is_empty() {
            let grow_width = (width - total_width).max(0.0) / grow_children.len() as f32;

            for child in &grow_children {
                let child_node = &mut nodes[child.0 as usize];
                child_node.result.width = Some(grow_width);
            }
        }
    } else {
        panic!("Node {node_id:?} has no width set and is not a flex container",);
    }
}

fn compute_x_offsets(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_x: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];

    node.result.x = Some(current_x);

    let width = node.result.width.unwrap();
    let padding = node.spec.inter_child_padding;

    let mut advance = current_x + node.spec.inner_padding.left;
    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        child_node.result.x = Some(advance);
        advance = compute_x_offsets(nodes, children, *child_id, advance) + padding;
    }

    current_x + width
}

fn compute_flex_heights(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];

    let mut total_height = 0.0f32;

    if let Size::Fit { min, max } = node.spec.height {
        for child in &children[node_id.0 as usize] {
            let child_height = if let Some(h) = nodes[child.0 as usize].result.height {
                h
            } else {
                compute_flex_heights(nodes, children, *child);
                nodes[child.0 as usize].result.height.unwrap()
            };

            total_height = total_height.max(child_height);
        }

        let node = &mut nodes[node_id.0 as usize];

        total_height += node.spec.inner_padding.top + node.spec.inner_padding.bottom;

        node.result.height = Some(total_height.clamp(min, max));
    } else {
        for child in &children[node_id.0 as usize] {
            compute_flex_heights(nodes, children, *child);
        }
    }
}

fn compute_y_offsets(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_y: f32,
) {
    let node = &mut nodes[node_id.0 as usize];

    node.result.y = Some(current_y);
    let y_inset = current_y + node.spec.inner_padding.top;

    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        child_node.result.y = Some(y_inset);
        compute_y_offsets(nodes, children, *child_id, y_inset);
    }
}
