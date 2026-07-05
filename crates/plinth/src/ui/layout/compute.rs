use crate::graphics::ClipRect;

use super::tree::LayoutNode;
use super::tree::NodeIndexArray;
use super::tree::NodeLayout;
use super::tree::UiElementId;
use super::tree::WrapLine;
use super::tree::WrapLines;
use super::types::Alignment;
use super::types::AxisAnchor;
use super::types::ChildWrap;
use super::types::LayoutDirection;
use super::types::OverlayPosition;
use super::types::Position;
use super::types::Size;
use super::types::Size::*;

pub(super) fn compute_major_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &mut [WrapLines],
    node_id: UiElementId,
    parent_limit: Option<f32>,
) -> f32 {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_fit_sizes::<D::Other>(
            nodes,
            children,
            wrap_lines,
            node_id,
            parent_limit,
        );
    }

    let size_spec = D::major_size_spec(node);
    let padding_start = D::major_axis_padding_start(node);
    let padding_end = D::major_axis_padding_end(node);
    let inter_child_padding = node.atom.inter_child_padding;
    let child_wrap = node.atom.child_wrap;
    let child_parent_limit =
        parent_limit.map(|limit| (limit - padding_start - padding_end).max(0.0));

    // Recurse into all children (including out-of-flow so they get their own sizes),
    // but only accumulate sizes from in-flow children into the parent's fit size.
    let mut in_flow_size = 0.0f32;
    let mut in_flow_count = 0u32;

    for child_id in node_children {
        let child_size = compute_major_axis_fit_sizes::<D>(
            nodes,
            children,
            wrap_lines,
            *child_id,
            child_parent_limit,
        );
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            in_flow_size += child_size;
            in_flow_count += 1;
        }
    }

    let child_content_size = if child_wrap == ChildWrap::Wrap {
        let wrap_limit = fit_wrap_limit(size_spec, parent_limit, padding_start + padding_end);
        rebuild_wrap_lines::<D>(nodes, children, wrap_lines, node_id, wrap_limit);
        max_line_major_size(&wrap_lines[node_idx])
    } else {
        wrap_lines[node_idx].lines.clear();
        in_flow_size
            + padding_start
            + padding_end
            + inter_child_padding * in_flow_count.saturating_sub(1) as f32
    };

    let child_sizes = if child_wrap == ChildWrap::Wrap {
        child_content_size + padding_start + padding_end
    } else {
        child_content_size
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
    wrap_lines: &mut [WrapLines],
    node_id: UiElementId,
) {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];
    let node_children = &children[node_id.0 as usize];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_minor_axis_grow_sizes::<D::Other>(nodes, children, wrap_lines, node_id);
    }

    let padding = D::major_axis_padding_start(node) + D::major_axis_padding_end(node);
    let available_major = (D::major_size_result(node) - padding).max(0.0);
    let spacing = node.atom.inter_child_padding;

    if node.atom.child_wrap == ChildWrap::Wrap {
        rebuild_wrap_lines::<D>(
            nodes,
            children,
            wrap_lines,
            node_id,
            finite_limit(available_major),
        );
        let line_count = wrap_lines[node_idx].lines.len();
        for line_index in 0..line_count {
            let line_children = wrap_lines[node_idx].lines[line_index].children.clone();
            distribute_major_grow::<D>(nodes, &line_children, available_major, spacing);
            wrap_lines[node_idx].lines[line_index].major_size =
                line_major_size::<D>(nodes, &line_children, spacing);
        }
        update_wrapped_minor_fit_size::<D>(nodes, &wrap_lines[node_idx], node_idx);
    } else {
        wrap_lines[node_idx].lines.clear();
        let in_flow_children = in_flow_children(nodes, node_children);
        distribute_major_grow::<D>(nodes, &in_flow_children, available_major, spacing);
    }

    // Step 3: Recurse into all children (including out-of-flow so their subtrees are sized).
    for child_id in node_children {
        compute_major_axis_grow_sizes::<D>(nodes, children, wrap_lines, *child_id);
    }
}

pub(super) fn compute_major_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &[WrapLines],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node_idx = node_id.0 as usize;
    let node = &mut nodes[node_idx];

    if node.atom.direction != D::DIRECTION {
        return compute_minor_axis_offsets::<D::Other>(
            nodes,
            children,
            wrap_lines,
            node_id,
            current_offset,
        );
    }

    D::set_major_offset(node, current_offset);

    if node.atom.child_wrap == ChildWrap::Wrap {
        return compute_wrapped_major_axis_offsets::<D>(
            nodes,
            children,
            wrap_lines,
            node_id,
            current_offset,
        );
    }

    let size = D::major_size_result(node);
    let padding_start = D::major_axis_padding_start(node);
    let item_spacing = node.atom.inter_child_padding;
    let padding_end = D::major_axis_padding_end(node);
    let major_align = node.atom.major_align;

    let node_children = &children[node_idx];
    let metrics = in_flow_child_major_metrics::<D>(nodes, node_children);
    let (mut advance, gap) = major_axis_offset_and_gap(
        major_align,
        current_offset,
        size,
        padding_start,
        padding_end,
        item_spacing,
        metrics,
    );

    for &child_id in node_children {
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            advance =
                compute_major_axis_offsets::<D>(nodes, children, wrap_lines, child_id, advance)
                    + gap;
        } else {
            compute_major_axis_offsets::<D>(nodes, children, wrap_lines, child_id, 0.0);
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

        node.text_height = Some(match node.atom.height {
            Fixed(height) => Fixed(height),
            Fit { min, max } => Fixed(text_height.clamp(min, max)),
            Grow => Grow,
            Flex { min, max } => Flex {
                min: text_height.clamp(min, max),
                max,
            },
        });
    }
}

pub(super) fn compute_minor_axis_fit_sizes<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &mut [WrapLines],
    node_id: UiElementId,
    parent_limit: Option<f32>,
) -> f32 {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];

    if node.atom.direction != D::DIRECTION {
        return compute_major_axis_fit_sizes::<D::Other>(
            nodes,
            children,
            wrap_lines,
            node_id,
            parent_limit,
        );
    }

    let size_spec = D::minor_size_spec(node);
    let size_padding = D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);
    let major_size = D::major_size_result(node);
    let major_padding = D::major_axis_padding_start(node) + D::major_axis_padding_end(node);
    let child_parent_limit = parent_limit.map(|limit| (limit - size_padding).max(0.0));
    let child_wrap = node.atom.child_wrap;
    let line_spacing = node.atom.line_spacing;

    // Only in-flow children contribute to the parent's minor-axis fit size.
    let mut max_in_flow_size = 0.0f32;

    for child_id in &children[node_id.0 as usize] {
        let child_size = compute_minor_axis_fit_sizes::<D>(
            nodes,
            children,
            wrap_lines,
            *child_id,
            child_parent_limit,
        );
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            max_in_flow_size = max_in_flow_size.max(child_size);
        }
    }

    let child_sizes = if child_wrap == ChildWrap::Wrap {
        if wrap_lines[node_idx].lines.is_empty()
            && children[node_idx]
                .iter()
                .any(|&child_id| nodes[child_id.0 as usize].atom.position.is_in_flow())
        {
            let available_major = (major_size - major_padding).max(0.0);
            rebuild_wrap_lines::<D>(
                nodes,
                children,
                wrap_lines,
                node_id,
                finite_limit(available_major),
            );
        }
        line_stack_minor_size::<D>(nodes, &wrap_lines[node_idx], line_spacing)
    } else {
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
    wrap_lines: &mut [WrapLines],
    node_id: UiElementId,
) {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];

    if !(node.atom.direction == D::DIRECTION) {
        return compute_major_axis_grow_sizes::<D::Other>(nodes, children, wrap_lines, node_id);
    }

    let remaining_size = D::minor_size_result(node)
        - (D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node));

    if node.atom.child_wrap == ChildWrap::Wrap {
        let line_count = wrap_lines[node_idx].lines.len();
        let line_spacing = node.atom.line_spacing;
        let mut grow_line_count = 0;
        let mut line_sizes = Vec::with_capacity(line_count);

        for line in &wrap_lines[node_idx].lines {
            let has_grow_child = line
                .children
                .iter()
                .any(|&child_id| matches!(D::minor_size_spec(&nodes[child_id.0 as usize]), Grow));
            if has_grow_child {
                grow_line_count += 1;
            }
            line_sizes.push((
                line.children.clone(),
                line_minor_size::<D>(nodes, &line.children),
                has_grow_child,
            ));
        }

        let used_size = line_sizes
            .iter()
            .map(|(_, line_size, _)| *line_size)
            .sum::<f32>()
            + line_spacing * line_count.saturating_sub(1) as f32;
        let grow_line_extra = if grow_line_count > 0 {
            (remaining_size - used_size).max(0.0) / grow_line_count as f32
        } else {
            0.0
        };

        for (line_children, line_minor_size, has_grow_child) in line_sizes {
            let grow_size = line_minor_size + if has_grow_child { grow_line_extra } else { 0.0 };
            for child_id in line_children {
                let child = &mut nodes[child_id.0 as usize];
                if matches!(D::minor_size_spec(child), Grow) {
                    D::set_minor_size(child, grow_size);
                }
            }
        }
    } else {
        for child_id in &children[node_id.0 as usize] {
            if nodes[child_id.0 as usize].atom.position.is_in_flow() {
                let child = &mut nodes[child_id.0 as usize];
                if matches!(D::minor_size_spec(child), Grow) {
                    D::set_minor_size(child, remaining_size);
                }
            }
        }
    }

    for child_id in &children[node_id.0 as usize] {
        compute_minor_axis_grow_sizes::<D>(nodes, children, wrap_lines, *child_id);
    }
}

pub(super) fn compute_minor_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &[WrapLines],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node = &mut nodes[node_id.0 as usize];
    let node_children = &children[node_id.0 as usize];

    if node.atom.direction != D::DIRECTION {
        return compute_major_axis_offsets::<D::Other>(
            nodes,
            children,
            wrap_lines,
            node_id,
            current_offset,
        );
    }

    D::set_minor_offset(node, current_offset);

    if node.atom.child_wrap == ChildWrap::Wrap {
        return compute_wrapped_minor_axis_offsets::<D>(
            nodes,
            children,
            wrap_lines,
            node_id,
            current_offset,
        );
    }

    let size = D::minor_size_result(node);
    let padding_start = D::minor_axis_padding_start(node);
    let padding_end = D::minor_axis_padding_end(node);
    let minor_align = node.atom.minor_align;

    for &child_id in node_children {
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            let child_size = D::minor_size_result(&nodes[child_id.0 as usize]);
            let inset = aligned_minor_offset(
                minor_align,
                current_offset,
                size,
                child_size,
                padding_start,
                padding_end,
            );
            compute_minor_axis_offsets::<D>(nodes, children, wrap_lines, child_id, inset);
        } else {
            compute_minor_axis_offsets::<D>(nodes, children, wrap_lines, child_id, 0.0);
        }
    }

    current_offset + size
}

fn fit_wrap_limit(size_spec: Size, parent_limit: Option<f32>, padding: f32) -> Option<f32> {
    let outer_limit = match size_spec {
        Fixed(size) => finite_limit(size),
        Fit { max, .. } => match (parent_limit.and_then(finite_limit), finite_limit(max)) {
            (Some(parent_limit), Some(max)) => Some(parent_limit.min(max)),
            (Some(parent_limit), None) => Some(parent_limit),
            (None, Some(max)) => Some(max),
            (None, None) => None,
        },
        Flex { max, .. } => finite_limit(max),
        Grow => parent_limit.and_then(finite_limit),
    };

    outer_limit.map(|limit| (limit - padding).max(0.0))
}

fn finite_limit(limit: f32) -> Option<f32> {
    if limit.is_finite() && limit < f32::MAX {
        Some(limit.max(0.0))
    } else {
        None
    }
}

#[derive(Clone, Copy)]
struct ChildMajorMetrics {
    count: usize,
    size: f32,
}

impl ChildMajorMetrics {
    fn spaced_size(self, spacing: f32) -> f32 {
        self.size + spacing * self.count.saturating_sub(1) as f32
    }
}

fn child_major_metrics<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    children: &[UiElementId],
) -> ChildMajorMetrics {
    ChildMajorMetrics {
        count: children.len(),
        size: child_major_size::<D>(nodes, children),
    }
}

fn in_flow_child_major_metrics<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    children: &[UiElementId],
) -> ChildMajorMetrics {
    let mut metrics = ChildMajorMetrics {
        count: 0,
        size: 0.0,
    };

    for &child_id in children {
        if nodes[child_id.0 as usize].atom.position.is_in_flow() {
            metrics.count += 1;
            metrics.size += D::major_size_result(&nodes[child_id.0 as usize]);
        }
    }

    metrics
}

fn major_axis_offset_and_gap(
    alignment: Alignment,
    current_offset: f32,
    size: f32,
    padding_start: f32,
    padding_end: f32,
    item_spacing: f32,
    metrics: ChildMajorMetrics,
) -> (f32, f32) {
    let available_major = (size - padding_start - padding_end).max(0.0);
    let line_major_size = metrics.spaced_size(item_spacing);

    match alignment {
        Alignment::Start => (current_offset + padding_start, item_spacing),
        Alignment::Center => (
            current_offset + padding_start + ((available_major - line_major_size) / 2.0).round(),
            item_spacing,
        ),
        Alignment::End => (
            current_offset + size - padding_end - line_major_size,
            item_spacing,
        ),
        Alignment::Justify if metrics.count > 1 => (
            current_offset + padding_start,
            item_spacing.max((available_major - metrics.size) / (metrics.count - 1) as f32),
        ),
        Alignment::Justify => (current_offset + padding_start, item_spacing),
    }
}

fn aligned_minor_offset(
    alignment: Alignment,
    current_offset: f32,
    size: f32,
    child_size: f32,
    padding_start: f32,
    padding_end: f32,
) -> f32 {
    match alignment {
        Alignment::Start | Alignment::Justify => current_offset + padding_start,
        Alignment::Center => (current_offset + (size - child_size).max(0.0) / 2.0).round(),
        Alignment::End => current_offset + (size - child_size - padding_end).max(0.0),
    }
}

fn in_flow_children(nodes: &[LayoutNode], children: &[UiElementId]) -> NodeIndexArray {
    children
        .iter()
        .copied()
        .filter(|child_id| nodes[child_id.0 as usize].atom.position.is_in_flow())
        .collect()
}

fn rebuild_wrap_lines<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &mut [WrapLines],
    node_id: UiElementId,
    available_major: Option<f32>,
) {
    let node_idx = node_id.0 as usize;
    let spacing = nodes[node_idx].atom.inter_child_padding;
    let should_wrap = available_major.is_some();
    let limit = available_major.unwrap_or(f32::MAX);
    let lines = &mut wrap_lines[node_idx].lines;

    lines.clear();
    let mut line = WrapLine::default();

    for &child_id in &children[node_idx] {
        if !nodes[child_id.0 as usize].atom.position.is_in_flow() {
            continue;
        }

        let child_size = D::major_size_result(&nodes[child_id.0 as usize]);
        let next_line_size = if line.children.is_empty() {
            child_size
        } else {
            line.major_size + spacing + child_size
        };

        if should_wrap && !line.children.is_empty() && next_line_size > limit {
            lines.push(line);
            line = WrapLine::default();
        }

        if line.children.is_empty() {
            line.major_size = child_size;
        } else {
            line.major_size += spacing + child_size;
        }
        line.children.push(child_id);
    }

    if !line.children.is_empty() {
        lines.push(line);
    }
}

fn max_line_major_size(wrap_lines: &WrapLines) -> f32 {
    wrap_lines
        .lines
        .iter()
        .map(|line| line.major_size)
        .fold(0.0, f32::max)
}

fn line_major_size<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    children: &[UiElementId],
    spacing: f32,
) -> f32 {
    let child_count = children.len();
    let child_size = child_major_size::<D>(nodes, children);
    child_size + spacing * child_count.saturating_sub(1) as f32
}

fn child_major_size<D: LayoutDirectionExt>(nodes: &[LayoutNode], children: &[UiElementId]) -> f32 {
    children
        .iter()
        .map(|child_id| D::major_size_result(&nodes[child_id.0 as usize]))
        .sum()
}

fn line_minor_size<D: LayoutDirectionExt>(nodes: &[LayoutNode], children: &[UiElementId]) -> f32 {
    children
        .iter()
        .map(|child_id| D::minor_size_result(&nodes[child_id.0 as usize]))
        .fold(0.0, f32::max)
}

fn line_stack_minor_size<D: LayoutDirectionExt>(
    nodes: &[LayoutNode],
    wrap_lines: &WrapLines,
    line_spacing: f32,
) -> f32 {
    let line_count = wrap_lines.lines.len();
    if line_count == 0 {
        return 0.0;
    }

    wrap_lines
        .lines
        .iter()
        .map(|line| line_minor_size::<D>(nodes, &line.children))
        .sum::<f32>()
        + line_spacing * line_count.saturating_sub(1) as f32
}

fn distribute_major_grow<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[UiElementId],
    available_major: f32,
    spacing: f32,
) {
    let child_count = children.len();
    if child_count == 0 {
        return;
    }

    let mut grow_children = NodeIndexArray::new();
    let mut remaining_size = available_major - spacing * child_count.saturating_sub(1) as f32;

    for &child_id in children {
        let child = &nodes[child_id.0 as usize];
        remaining_size -= D::major_size_result(child);

        match D::major_size_spec(child) {
            Fixed(_) | Fit { .. } => {}
            Flex { .. } | Grow => grow_children.push(child_id),
        }
    }

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
}

fn update_wrapped_minor_fit_size<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    wrap_lines: &WrapLines,
    node_idx: usize,
) {
    let node = &nodes[node_idx];
    let line_spacing = node.atom.line_spacing;
    let padding = D::minor_axis_padding_start(node) + D::minor_axis_padding_end(node);
    let content_size = line_stack_minor_size::<D>(nodes, wrap_lines, line_spacing);

    if let Fit { min, max } = D::minor_size_spec(node) {
        D::set_minor_size(
            &mut nodes[node_idx],
            (content_size + padding).clamp(min, max),
        );
    }
}

fn compute_wrapped_major_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &[WrapLines],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];
    let size = D::major_size_result(node);
    let padding_start = D::major_axis_padding_start(node);
    let padding_end = D::major_axis_padding_end(node);
    let item_spacing = node.atom.inter_child_padding;
    let major_align = node.atom.major_align;

    for &child_id in &children[node_idx] {
        if !nodes[child_id.0 as usize].atom.position.is_in_flow() {
            compute_major_axis_offsets::<D>(nodes, children, wrap_lines, child_id, 0.0);
        }
    }

    for line in &wrap_lines[node_idx].lines {
        let (mut advance, gap) = major_axis_offset_and_gap(
            major_align,
            current_offset,
            size,
            padding_start,
            padding_end,
            item_spacing,
            child_major_metrics::<D>(nodes, &line.children),
        );

        for &child_id in &line.children {
            advance =
                compute_major_axis_offsets::<D>(nodes, children, wrap_lines, child_id, advance)
                    + gap;
        }
    }

    current_offset + size
}

fn compute_wrapped_minor_axis_offsets<D: LayoutDirectionExt>(
    nodes: &mut [LayoutNode],
    children: &[NodeIndexArray],
    wrap_lines: &[WrapLines],
    node_id: UiElementId,
    current_offset: f32,
) -> f32 {
    let node_idx = node_id.0 as usize;
    let node = &nodes[node_idx];
    let size = D::minor_size_result(node);
    let padding_start = D::minor_axis_padding_start(node);
    let padding_end = D::minor_axis_padding_end(node);
    let line_spacing = node.atom.line_spacing;
    let line_align = node.atom.line_align;
    let minor_align = node.atom.minor_align;
    let line_count = wrap_lines[node_idx].lines.len();

    for &child_id in &children[node_idx] {
        if !nodes[child_id.0 as usize].atom.position.is_in_flow() {
            compute_minor_axis_offsets::<D>(nodes, children, wrap_lines, child_id, 0.0);
        }
    }

    if line_count == 0 {
        return current_offset + size;
    }

    let mut line_sizes = Vec::with_capacity(line_count);
    for line in &wrap_lines[node_idx].lines {
        line_sizes.push(line_minor_size::<D>(nodes, &line.children));
    }

    let line_size_sum = line_sizes.iter().copied().sum::<f32>();
    let stack_size = line_size_sum + line_spacing * line_count.saturating_sub(1) as f32;
    let available_minor = (size - padding_start - padding_end).max(0.0);

    let (mut line_offset, gap) = match line_align {
        Alignment::Start => (current_offset + padding_start, line_spacing),
        Alignment::Center => (
            current_offset + padding_start + ((available_minor - stack_size) / 2.0).round(),
            line_spacing,
        ),
        Alignment::End => (
            current_offset + size - padding_end - stack_size,
            line_spacing,
        ),
        Alignment::Justify if line_count > 1 => (
            current_offset + padding_start,
            line_spacing.max((available_minor - line_size_sum) / (line_count - 1) as f32),
        ),
        Alignment::Justify => (current_offset + padding_start, line_spacing),
    };

    for (line, line_size) in wrap_lines[node_idx].lines.iter().zip(line_sizes) {
        for &child_id in &line.children {
            let child_size = D::minor_size_result(&nodes[child_id.0 as usize]);
            let child_offset =
                aligned_minor_offset(minor_align, line_offset, line_size, child_size, 0.0, 0.0);
            compute_minor_axis_offsets::<D>(nodes, children, wrap_lines, child_id, child_offset);
        }

        line_offset += line_size + gap;
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
        node.text_height.unwrap_or(node.atom.height)
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
        node.text_height.unwrap_or(node.atom.height)
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
    parent: &NodeLayout,
    child: &NodeLayout,
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
