use crate::graphics::ClipRect;

use super::tree::LayoutNode;
use super::tree::LayoutNodeResult;
use super::tree::NodeIndexArray;
use super::tree::UiElementId;
use super::types::Alignment;
use super::types::AxisAnchor;
use super::types::LayoutDirection;
use super::types::OverlayPosition;
use super::types::Position;
use super::types::Size;
use super::types::Size::*;

pub(super) fn compute_major_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    parent_limit: Option<f32>,
) -> f32 {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_fit_sizes::<D::Other>(nodes, children, node_id, parent_limit);
    }

    let size_spec = D::major_size_spec(node);
    let padding_start = D::major_axis_padding_start(node);
    let padding_end = D::major_axis_padding_end(node);
    let inter_child_padding = node.atom.inter_child_padding;
    let child_parent_limit =
        parent_limit.map(|limit| (limit - padding_start - padding_end).max(0.0));

    // Recurse into all children (including out-of-flow so they get their own sizes),
    // but only accumulate sizes from in-flow children into the parent's fit size.
    let child_sizes = {
        let mut in_flow_size = 0.0f32;
        let mut in_flow_count = 0u32;

        for child_id in node_children {
            let child_size =
                compute_major_axis_fit_sizes::<D>(nodes, children, *child_id, child_parent_limit);
            if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                in_flow_size += child_size;
                in_flow_count += 1;
            }
        }

        in_flow_size
            + padding_start
            + padding_end
            + inter_child_padding * in_flow_count.saturating_sub(1) as f32
    };

    let mut size = match size_spec {
        Size::Fixed(size) => size,
        Size::Fit { min, max } => child_sizes.clamp(min, max),
        Size::Flex { max, .. } => max,
        Size::Grow => {
            // Grow is handled in the offsets phase
            0.0
        }
    };

    if let Some(limit) = parent_limit {
        size = size.min(limit);
    }

    D::set_major_size(&mut nodes[node_id.0 as usize], size);

    size
}

pub(super) fn compute_major_axis_grow_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
) {
    let node = &nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_grow_sizes::<D::Other>(nodes, children, node_id);
    }

    // Compute remaining size using only in-flow children.
    let in_flow_count = node_children
        .iter()
        .filter(|&&id| nodes[id.0 as usize].atom.position.is_in_flow())
        .count();
    let inter_child = node.atom.inter_child_padding * in_flow_count.saturating_sub(1) as f32;
    let padding = D::major_axis_padding_start(node) + D::major_axis_padding_end(node);
    let mut grow_children = NodeIndexArray::new();
    let mut remaining_size = D::major_size_result(node) - inter_child - padding;

    // Step 1: Find in-flow children that can grow.
    for child_id in node_children {
        let child = &nodes[child_id.0 as usize];
        if !child.atom.position.is_in_flow() {
            continue;
        }

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

        grow_children.retain(|child_id| {
            let child = &mut nodes[child_id.0 as usize];
            let child_size = D::major_size_result(child);

            match D::major_size_spec(child) {
                Fixed(_) | Fit { .. } => false,
                Flex { max, .. } => {
                    let tentative_size = child_size + distributed_size;

                    let (is_done, actual_size) = if tentative_size > max {
                        (true, max)
                    } else {
                        (false, tentative_size)
                    };

                    D::set_major_size(child, actual_size);
                    remaining_size -= actual_size - child_size;

                    !is_done
                }
                Grow if remaining_size > 0.0 => {
                    D::set_major_size(child, child_size + distributed_size);
                    remaining_size -= distributed_size;
                    true
                }
                Grow => false,
            }
        });
    }

    // Step 3: Recurse into all children (including out-of-flow so their subtrees are sized).
    for child_id in node_children {
        compute_major_axis_grow_sizes::<D>(nodes, children, *child_id);
    }
}

pub(super) fn compute_major_axis_offsets<D: LayoutDirectionExt>(
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
    // Copy major_align so the mutable borrow of `nodes` through `node` ends here,
    // allowing the immutable borrow needed by the in_flow_count filter below.
    let major_align = node.atom.major_align;

    let node_children = &children[node_id.0 as usize];

    // Count in-flow children for alignment calculations.
    let in_flow_count = node_children
        .iter()
        .filter(|&&id| nodes[id.0 as usize].atom.position.is_in_flow())
        .count();

    // Recurse into an out-of-flow child with offset 0.0. Its absolute position will be
    // corrected by pass 7.5 (compute_overlay_positions). We still recurse so its own
    // children get their relative offsets computed correctly.
    macro_rules! recurse_out_of_flow {
        ($child_id:expr) => {
            compute_major_axis_offsets::<D>(nodes, children, $child_id, 0.0)
        };
    }

    match major_align {
        Alignment::Start => {
            let mut advance = current_offset + padding_start;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    advance = compute_major_axis_offsets::<D>(nodes, children, child_id, advance)
                        + padding_internal;
                } else {
                    recurse_out_of_flow!(child_id);
                }
            }
        }
        Alignment::Center => {
            let mut in_flow_content_size = padding_start
                + padding_end
                + padding_internal * in_flow_count.saturating_sub(1) as f32;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    in_flow_content_size += D::major_size_result(&nodes[child_id.0 as usize]);
                }
            }

            let half_unused_space = ((size - in_flow_content_size) / 2.0).round();
            let mut advance = current_offset + padding_start + half_unused_space;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    advance = compute_major_axis_offsets::<D>(nodes, children, child_id, advance)
                        + padding_internal;
                } else {
                    recurse_out_of_flow!(child_id);
                }
            }
        }
        Alignment::End => {
            let mut in_flow_content_size =
                padding_end + padding_internal * in_flow_count.saturating_sub(1) as f32;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    in_flow_content_size += D::major_size_result(&nodes[child_id.0 as usize]);
                }
            }

            let mut advance = current_offset + size - in_flow_content_size;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    advance = compute_major_axis_offsets::<D>(nodes, children, child_id, advance)
                        + padding_internal;
                } else {
                    recurse_out_of_flow!(child_id);
                }
            }
        }
        Alignment::Justify if in_flow_count > 1 => {
            let mut in_flow_content_size =
                padding_start + padding_end + padding_internal * (in_flow_count - 1) as f32;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    in_flow_content_size += D::major_size_result(&nodes[child_id.0 as usize]);
                }
            }

            let internal_padding =
                padding_internal.max((size - in_flow_content_size) / (in_flow_count - 1) as f32);

            let mut advance = current_offset + padding_start;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    advance = compute_major_axis_offsets::<D>(nodes, children, child_id, advance)
                        + internal_padding;
                } else {
                    recurse_out_of_flow!(child_id);
                }
            }
        }
        Alignment::Justify => {
            // Justified layout with 0 or 1 in-flow children: treat as start-aligned.
            let mut advance = current_offset + padding_start;
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    advance = compute_major_axis_offsets::<D>(nodes, children, child_id, advance)
                        + padding_internal;
                } else {
                    recurse_out_of_flow!(child_id);
                }
            }
        }
    }

    current_offset + size
}

pub(super) fn compute_text_heights<'a, T: 'a>(
    mut measure_text: impl FnMut(&mut T, f32) -> Option<f32>,
    nodes: impl Iterator<Item = (&'a mut LayoutNode, &'a mut T)>,
) {
    for (node, content) in nodes {
        let Some(text_height) = measure_text(content, node.result.width) else {
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

pub(super) fn compute_minor_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    parent_limit: Option<f32>,
) -> f32 {
    let node = &nodes[node_id.0 as usize];

    if node.atom.direction != D::DIRECTION {
        return compute_major_axis_fit_sizes::<D::Other>(nodes, children, node_id, parent_limit);
    }

    let size_spec = D::minor_size_spec(node);
    let size_padding = D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);
    let child_parent_limit = parent_limit.map(|limit| (limit - size_padding).max(0.0));

    // Only in-flow children contribute to the parent's minor-axis fit size.
    let child_sizes = {
        let mut max_in_flow_size = 0.0f32;

        for child_id in &children[node_id.0 as usize] {
            let child_size =
                compute_minor_axis_fit_sizes::<D>(nodes, children, *child_id, child_parent_limit);
            if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                max_in_flow_size = max_in_flow_size.max(child_size);
            }
        }

        max_in_flow_size
    };

    let mut size = match size_spec {
        Fixed(size) => size,
        Fit { min, max } => (child_sizes + size_padding).clamp(min, max),
        Flex { max, .. } => max,
        Grow => 0.0, // Grow is handled later
    };

    if let Some(limit) = parent_limit {
        size = size.min(limit);
    }

    D::set_minor_size(&mut nodes[node_id.0 as usize], size);
    size
}

pub(super) fn compute_minor_axis_grow_sizes<D: LayoutDirectionExt>(
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
        // Only in-flow children consume the parent's minor-axis remaining space.
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            let child = &mut nodes[child_id.0 as usize];
            if matches!(D::minor_size_spec(child), Grow) {
                D::set_minor_size(child, remaining_size);
            }
        }

        compute_minor_axis_grow_sizes::<D>(nodes, children, *child_id);
    }
}

pub(super) fn compute_minor_axis_offsets<D: LayoutDirectionExt>(
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

            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, inset);
                } else {
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, 0.0);
                }
            }
        }
        Alignment::Center => {
            // Center on a per-child basis (in-flow children only).
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    let child_size = D::minor_size_result(&nodes[child_id.0 as usize]);
                    let inset = (current_offset + (size - child_size).max(0.0) / 2.0).round();
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, inset);
                } else {
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, 0.0);
                }
            }
        }
        Alignment::End => {
            for &child_id in node_children {
                if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                    let child_size = D::minor_size_result(&nodes[child_id.0 as usize]);
                    let inset = current_offset + (size - child_size - padding_end).max(0.0);
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, inset);
                } else {
                    compute_minor_axis_offsets::<D>(nodes, children, child_id, 0.0);
                }
            }
        }
    }

    current_offset + size
}

pub(super) trait LayoutDirectionExt {
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

pub(super) struct HorizontalMode;

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

pub(super) struct VerticalMode;

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

/// Pass 7.5: position out-of-flow nodes using their parent's computed layout result.
///
/// Processes `out_of_flow_nodes` in insertion order (parents before children), which
/// ensures that nested overlays are handled correctly: when a parent overlay is shifted,
/// its children shift with it, and the subsequent delta computed for each child is only
/// the remaining displacement from its tentative in-flow position to its overlay target.
pub(super) fn compute_overlay_positions(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    out_of_flow_nodes: &[(UiElementId, UiElementId)],
    viewport: ClipRect,
) {
    for &(parent_id, child_id) in out_of_flow_nodes {
        let (target_x, target_y) = resolve_overlay_position(nodes, parent_id, child_id, viewport);
        let child = &nodes[child_id.0 as usize];
        let dx = target_x - child.result.x;
        let dy = target_y - child.result.y;
        if dx != 0.0 || dy != 0.0 {
            adjust_subtree_offsets(nodes, children, child_id, dx, dy);
        }
    }
}

/// Compute the target (x, y) for one out-of-flow child, applying viewport-aware flipping.
fn resolve_overlay_position(
    nodes: &[LayoutNode],
    parent_id: UiElementId,
    child_id: UiElementId,
    viewport: ClipRect,
) -> (f32, f32) {
    let parent = &nodes[parent_id.0 as usize].result;
    let child = &nodes[child_id.0 as usize];

    match child.atom.position {
        Position::Absolute { x, y } => (x, y),
        Position::InFlow => (child.result.x, child.result.y), // should not happen
        Position::OutOfFlow(pos) => {
            let (x, y) = compute_anchored_position(parent, &child.result, pos);

            // Apply viewport-aware flipping.
            let x = if pos.flip_x {
                if x < viewport.point[0]
                    || x + child.result.width > viewport.point[0] + viewport.size[0]
                {
                    let flipped = compute_anchored_position(
                        parent,
                        &child.result,
                        OverlayPosition {
                            self_x: flip(pos.self_x),
                            parent_x: flip(pos.parent_x),
                            ..pos
                        },
                    );
                    flipped.0
                } else {
                    x
                }
            } else {
                x
            };

            let y = if pos.flip_y {
                if y < viewport.point[1]
                    || y + child.result.height > viewport.point[1] + viewport.size[1]
                {
                    let flipped = compute_anchored_position(
                        parent,
                        &child.result,
                        OverlayPosition {
                            self_y: flip(pos.self_y),
                            parent_y: flip(pos.parent_y),
                            ..pos
                        },
                    );
                    flipped.1
                } else {
                    y
                }
            } else {
                y
            };

            (x, y)
        }
    }
}

fn compute_anchored_position(
    parent: &LayoutNodeResult,
    child: &LayoutNodeResult,
    pos: OverlayPosition,
) -> (f32, f32) {
    let anchor_x = resolve_anchor(parent.x, parent.width, pos.parent_x);
    let anchor_y = resolve_anchor(parent.y, parent.height, pos.parent_y);
    let self_offset_x = resolve_anchor(0.0, child.width, pos.self_x);
    let self_offset_y = resolve_anchor(0.0, child.height, pos.self_y);
    (
        anchor_x - self_offset_x + pos.offset.0,
        anchor_y - self_offset_y + pos.offset.1,
    )
}

fn resolve_anchor(origin: f32, size: f32, anchor: AxisAnchor) -> f32 {
    match anchor {
        AxisAnchor::Start => origin,
        AxisAnchor::Center => origin + size / 2.0,
        AxisAnchor::End => origin + size,
    }
}

fn flip(anchor: AxisAnchor) -> AxisAnchor {
    match anchor {
        AxisAnchor::Start => AxisAnchor::End,
        AxisAnchor::End => AxisAnchor::Start,
        AxisAnchor::Center => AxisAnchor::Center,
    }
}

/// Recursively shift all nodes in a subtree by (dx, dy).
fn adjust_subtree_offsets(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    dx: f32,
    dy: f32,
) {
    let idx = node_id.0 as usize;
    nodes[idx].result.x += dx;
    nodes[idx].result.y += dy;
    for child_id in children[idx].iter().copied() {
        adjust_subtree_offsets(nodes, children, child_id, dx, dy);
    }
}

pub(super) fn compute_clip_rects(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    node_id: UiElementId,
    current_clip: ClipRect,
    viewport_clip: ClipRect,
) {
    let idx = node_id.0 as usize;

    // Out-of-flow nodes escape their logical parent's clip hierarchy: they start from
    // the viewport clip instead, so they are never clipped by a scroll container ancestor.
    let base_clip = if nodes[idx].atom.position.is_in_flow() {
        current_clip
    } else {
        viewport_clip
    };

    let effective = if nodes[idx].atom.clip_overflow {
        let r = &nodes[idx].result;
        base_clip.next(&ClipRect {
            point: [r.x, r.y],
            size: [r.width, r.height],
        })
    } else {
        base_clip
    };

    nodes[idx].result.effective_clip = effective;
    for child_id in children[idx].iter().copied() {
        compute_clip_rects(nodes, children, child_id, effective, viewport_clip);
    }
}
