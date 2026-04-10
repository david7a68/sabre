use smallvec::SmallVec;

use crate::graphics::ClipRect;

pub use Size::*;

/// Which point along one axis to use when anchoring an overlay to its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisAnchor {
    /// Left edge (x) or top edge (y).
    #[default]
    Start,
    /// Center.
    Center,
    /// Right edge (x) or bottom edge (y).
    End,
}

/// Positions an out-of-flow overlay relative to its parent node's layout result.
///
/// The overlay is placed so that the point `(self_x, self_y)` on the overlay
/// coincides with the point `(parent_x, parent_y)` on the parent, plus `offset`.
///
/// When `flip_x` or `flip_y` is true, the corresponding anchor pair is mirrored
/// (Start↔End) if the overlay would extend outside the viewport on that axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayPosition {
    pub parent_x: AxisAnchor,
    pub parent_y: AxisAnchor,
    pub self_x: AxisAnchor,
    pub self_y: AxisAnchor,
    pub offset: (f32, f32),
    /// Mirror x anchors when the overlay would clip the viewport horizontally.
    /// Use for context menus and submenus.
    pub flip_x: bool,
    /// Mirror y anchors when the overlay would clip the viewport vertically.
    pub flip_y: bool,
}

/// Controls how a node participates in its parent's layout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Position {
    /// Normal flow: participates in parent sizing and sibling alignment.
    #[default]
    InFlow,
    /// Out of flow: does not affect parent/sibling layout; positioned relative
    /// to the parent's computed result in the same frame.
    OutOfFlow(OverlayPosition),
    /// Out of flow: positioned at an explicit screen-space coordinate.
    /// Used for overlays that persist their own position across frames (draggable panels).
    Absolute { x: f32, y: f32 },
}

impl Position {
    #[inline]
    pub fn is_in_flow(self) -> bool {
        matches!(self, Position::InFlow)
    }
}

/// Single-dimension size for UI elements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Fixed(f32),
    /// Size to fit content, with optional min and max constraints.
    Fit {
        min: f32,
        max: f32,
    },
    Grow,
    /// Size to fit container, with optional min and max constraints.
    Flex {
        min: f32,
        max: f32,
    },
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

    pub clip_overflow: bool,

    /// How this node participates in its parent's layout. Defaults to [`Position::InFlow`].
    pub position: Position,
    /// Rendering layer. Nodes with higher `z_layer` render above nodes with lower `z_layer`.
    /// Overlay children automatically receive `parent.z_layer + 1`.
    pub z_layer: u8,
    /// When true, this overlay blocks pointer and keyboard input from reaching any widget
    /// on a lower `z_layer`, regardless of pointer position. Use for modal dialogs.
    pub is_modal: bool,
}

#[derive(Debug, Default)]
pub(crate) struct LayoutNodeResult {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub effective_clip: ClipRect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct UiElementId(pub(crate) u16);

type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Default, Debug)]
pub(crate) struct LayoutNode {
    pub atom: Atom,
    pub result: LayoutNodeResult,
}

pub(crate) struct LayoutTree<T> {
    nodes: Vec<LayoutNode>,
    children: Vec<NodeIndexArray>,

    /// Content associated with nodes, such as text layouts.
    ///
    /// These are stored separately under the assumption that they occur much
    /// less frequently than the nodes themselves, and that they are often much
    /// larger in size.
    content: Vec<T>,

    /// (parent_id, child_id) for every out-of-flow node, in insertion order.
    /// Parents always appear before their children (builder API enforces this),
    /// which is required for correct nested overlay positioning in pass 7.5.
    out_of_flow_nodes: SmallVec<[(UiElementId, UiElementId); 4]>,

    /// Node ids grouped by z_layer (index 0 = base, 1 = first overlay layer, etc.).
    /// Every node is appended here at `add()` time. Used by the renderer to guarantee
    /// correct layer ordering without sorting.
    layer_buckets: SmallVec<[SmallVec<[UiElementId; 64]>; 2]>,
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
            out_of_flow_nodes: SmallVec::new(),
            layer_buckets: SmallVec::new(),
        }
    }

    /// Iterate nodes in layer order (ascending z_layer, creation order within each layer).
    /// This is the correct order for rendering: base layer first, then overlay layers on top.
    pub fn iter_nodes_by_layer(&self) -> impl Iterator<Item = (&LayoutNode, &T)> {
        self.layer_buckets
            .iter()
            .flat_map(|bucket| bucket.iter())
            .map(|&id| (&self.nodes[id.0 as usize], &self.content[id.0 as usize]))
    }

    pub fn atom_mut(&mut self, node: UiElementId) -> &mut Atom {
        &mut self.nodes[node.0 as usize].atom
    }

    pub fn content_mut(&mut self, node: UiElementId) -> &mut T {
        &mut self.content[node.0 as usize]
    }

    pub fn add(&mut self, parent: Option<UiElementId>, atom: Atom, content: T) -> UiElementId {
        let node_id = UiElementId(self.nodes.len() as u16);

        // Populate layer bucket (grow the vec if this z_layer hasn't been seen yet).
        let layer = atom.z_layer as usize;
        if self.layer_buckets.len() <= layer {
            self.layer_buckets.resize_with(layer + 1, SmallVec::new);
        }
        self.layer_buckets[layer].push(node_id);

        // Stash out-of-flow nodes for pass 7.5. Parent id is always known here.
        if !atom.position.is_in_flow()
            && let Some(parent_id) = parent
        {
            self.out_of_flow_nodes.push((parent_id, node_id));
        }

        let node = LayoutNode {
            atom,
            result: LayoutNodeResult::default(),
        };

        self.nodes.push(node);
        self.content.push(content);
        self.children.push(NodeIndexArray::new());

        if let Some(parent_id) = parent {
            self.children[parent_id.0 as usize].push(node_id);
        }

        node_id
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.children.clear();
        self.content.clear();
        self.out_of_flow_nodes.clear();
        self.layer_buckets.clear();
    }

    pub fn compute_layout(&mut self, measure_text: impl FnMut(&mut T, f32) -> Option<f32>) {
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

        compute_major_axis_fit_sizes::<HorizontalMode>(nodes, &self.children, node_id, None);
        compute_major_axis_grow_sizes::<HorizontalMode>(nodes, &self.children, node_id);

        compute_text_heights(measure_text, nodes.iter_mut().zip(self.content.iter_mut()));

        compute_minor_axis_fit_sizes::<HorizontalMode>(nodes, &self.children, node_id, None);
        compute_minor_axis_grow_sizes::<HorizontalMode>(nodes, &self.children, node_id);

        compute_major_axis_offsets::<HorizontalMode>(nodes, &self.children, node_id, 0.0);
        compute_minor_axis_offsets::<HorizontalMode>(nodes, &self.children, node_id, 0.0);

        // Pass 7.5: position out-of-flow (overlay) nodes relative to their parents.
        let viewport_clip = {
            let r = &nodes[0].result;
            ClipRect {
                point: [r.x, r.y],
                size: [r.width, r.height],
            }
        };
        compute_overlay_positions(
            nodes,
            &self.children,
            &self.out_of_flow_nodes,
            viewport_clip,
        );

        compute_clip_rects(
            nodes,
            &self.children,
            node_id,
            ClipRect::default(),
            viewport_clip,
        );
    }
}

fn compute_major_axis_fit_sizes<D: LayoutDirectionExt>(
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

fn compute_text_heights<'a, T: 'a>(
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

fn compute_minor_axis_fit_sizes<D: LayoutDirectionExt>(
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

/// Pass 7.5: position out-of-flow nodes using their parent's computed layout result.
///
/// Processes `out_of_flow_nodes` in insertion order (parents before children), which
/// ensures that nested overlays are handled correctly: when a parent overlay is shifted,
/// its children shift with it, and the subsequent delta computed for each child is only
/// the remaining displacement from its tentative in-flow position to its overlay target.
fn compute_overlay_positions(
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

fn compute_clip_rects(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn node_result(tree: &LayoutTree<()>, id: UiElementId) -> &LayoutNodeResult {
        &tree.nodes[id.0 as usize].result
    }

    // ── Out-of-flow children excluded from parent Fit size ──────────────────

    #[test]
    fn out_of_flow_not_counted_in_parent_fit_width() {
        // Parent: Fit width. One in-flow child (fixed 50px), one out-of-flow (fixed 100px).
        // Parent width must equal 50, not 150.
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(400.0),
                height: Fixed(200.0),
                ..Default::default()
            },
            (),
        );
        let parent = tree.add(
            Some(root),
            Atom {
                width: Fit {
                    min: 0.0,
                    max: f32::MAX,
                },
                height: Fixed(50.0),
                ..Default::default()
            },
            (),
        );
        tree.add(
            Some(parent),
            Atom {
                width: Fixed(50.0),
                height: Fixed(20.0),
                ..Default::default()
            },
            (),
        );
        tree.add(
            Some(parent),
            Atom {
                width: Fixed(100.0),
                height: Fixed(20.0),
                position: Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: false,
                }),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        tree.compute_layout(|_, _| None);
        assert_eq!(
            node_result(&tree, parent).width,
            50.0,
            "parent width should match in-flow child only"
        );
    }

    // ── compute_overlay_positions: dropdown preset ───────────────────────────

    #[test]
    fn overlay_positions_below_parent() {
        // Parent at (10, 10), size (80, 30). Overlay size (80, 120).
        // Anchor: parent (Start,End) -> overlay (Start,Start): overlay top-left at parent bottom-left.
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(800.0),
                height: Fixed(600.0),
                ..Default::default()
            },
            (),
        );
        let parent = tree.add(
            Some(root),
            Atom {
                width: Fixed(80.0),
                height: Fixed(30.0),
                inner_padding: Padding {
                    left: 10.0,
                    top: 10.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            (),
        );
        let overlay = tree.add(
            Some(parent),
            Atom {
                width: Fixed(80.0),
                height: Fixed(120.0),
                position: Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: false,
                }),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        tree.compute_layout(|_, _| None);
        let pr = node_result(&tree, parent);
        let or_ = node_result(&tree, overlay);
        assert_eq!(or_.x, pr.x, "overlay x should align with parent left");
        assert_eq!(
            or_.y,
            pr.y + pr.height,
            "overlay top should be at parent bottom"
        );
    }

    // ── compute_overlay_positions: centered modal ────────────────────────────

    #[test]
    fn overlay_positions_centered_on_parent() {
        // Parent at (0, 0), size (400, 300). Overlay size (200, 150).
        // Anchor (Center,Center)+(Center,Center): overlay center == parent center.
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(400.0),
                height: Fixed(300.0),
                ..Default::default()
            },
            (),
        );
        let overlay = tree.add(
            Some(root),
            Atom {
                width: Fixed(200.0),
                height: Fixed(150.0),
                position: Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Center,
                    parent_y: AxisAnchor::Center,
                    self_x: AxisAnchor::Center,
                    self_y: AxisAnchor::Center,
                    offset: (0.0, 0.0),
                    flip_x: false,
                    flip_y: false,
                }),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        tree.compute_layout(|_, _| None);
        let or_ = node_result(&tree, overlay);
        assert_eq!(or_.x, 100.0, "overlay x should be centered");
        assert_eq!(or_.y, 75.0, "overlay y should be centered");
    }

    // ── Clip rect escape ─────────────────────────────────────────────────────

    #[test]
    fn overlay_escapes_parent_clip() {
        // Parent clips its overflow (100×100 at origin). Out-of-flow child placed below at y=200.
        // Child's effective_clip should be the viewport, not the parent's clip.
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(800.0),
                height: Fixed(600.0),
                ..Default::default()
            },
            (),
        );
        let parent = tree.add(
            Some(root),
            Atom {
                width: Fixed(100.0),
                height: Fixed(100.0),
                clip_overflow: true,
                ..Default::default()
            },
            (),
        );
        let overlay = tree.add(
            Some(parent),
            Atom {
                width: Fixed(80.0),
                height: Fixed(50.0),
                position: Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::Start,
                    parent_y: AxisAnchor::End,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 100.0), // push it well below parent
                    flip_x: false,
                    flip_y: false,
                }),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        tree.compute_layout(|_, _| None);
        let root_r = node_result(&tree, root);
        let ov_r = node_result(&tree, overlay);
        // Overlay clip must be the viewport (root bounds), not clipped to the 100×100 parent.
        assert_eq!(
            ov_r.effective_clip.point,
            [root_r.x, root_r.y],
            "overlay clip should start at viewport origin"
        );
        assert_eq!(
            ov_r.effective_clip.size,
            [root_r.width, root_r.height],
            "overlay clip should match viewport size"
        );
    }

    // ── out_of_flow_nodes stash ──────────────────────────────────────────────

    #[test]
    fn out_of_flow_stash_populated() {
        let mut tree: LayoutTree<()> = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(400.0),
                height: Fixed(300.0),
                ..Default::default()
            },
            (),
        );
        tree.add(
            Some(root),
            Atom {
                width: Fixed(50.0),
                height: Fixed(20.0),
                ..Default::default()
            },
            (),
        ); // in-flow
        let ov_pos = OverlayPosition {
            parent_x: AxisAnchor::Start,
            parent_y: AxisAnchor::End,
            self_x: AxisAnchor::Start,
            self_y: AxisAnchor::Start,
            offset: (0.0, 0.0),
            flip_x: false,
            flip_y: false,
        };
        tree.add(
            Some(root),
            Atom {
                position: Position::OutOfFlow(ov_pos),
                z_layer: 1,
                width: Fixed(80.0),
                height: Fixed(40.0),
                ..Default::default()
            },
            (),
        );
        tree.add(
            Some(root),
            Atom {
                position: Position::OutOfFlow(ov_pos),
                z_layer: 1,
                width: Fixed(60.0),
                height: Fixed(30.0),
                ..Default::default()
            },
            (),
        );
        assert_eq!(
            tree.out_of_flow_nodes.len(),
            2,
            "two out-of-flow children should be stashed"
        );
        assert_eq!(
            tree.layer_buckets.len(),
            2,
            "two z_layers (0 and 1) should exist"
        );
        assert_eq!(
            tree.layer_buckets[0].len(),
            2,
            "root + in-flow child in base layer"
        ); // root + in-flow child
        assert_eq!(
            tree.layer_buckets[1].len(),
            2,
            "two overlay nodes in layer 1"
        );
    }

    // ── Context menu flip ────────────────────────────────────────────────────

    #[test]
    fn context_menu_flips_when_near_right_border() {
        // Viewport: 400×600. A spacer of 350px pushes the parent to x=350, width=40.
        // Overlay (120×200) with anchor (End,Start)+(Start,Start), flip_x true.
        // Without flip: overlay.x = 350+40 = 390; 390+120 = 510 > 400 → flip.
        // Flipped (parent_x: Start, self_x: End): overlay.x = 350 - 120 = 230.
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(400.0),
                height: Fixed(600.0),
                ..Default::default()
            },
            (),
        );
        // Spacer pushes parent to x=350 in the root's horizontal layout.
        tree.add(
            Some(root),
            Atom {
                width: Fixed(350.0),
                height: Fixed(20.0),
                ..Default::default()
            },
            (),
        );
        let parent = tree.add(
            Some(root),
            Atom {
                width: Fixed(40.0),
                height: Fixed(20.0),
                ..Default::default()
            },
            (),
        );
        let overlay = tree.add(
            Some(parent),
            Atom {
                width: Fixed(120.0),
                height: Fixed(200.0),
                position: Position::OutOfFlow(OverlayPosition {
                    parent_x: AxisAnchor::End,
                    parent_y: AxisAnchor::Start,
                    self_x: AxisAnchor::Start,
                    self_y: AxisAnchor::Start,
                    offset: (0.0, 0.0),
                    flip_x: true,
                    flip_y: false,
                }),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        tree.compute_layout(|_, _| None);
        let pr = node_result(&tree, parent);
        let or_ = node_result(&tree, overlay);
        assert_eq!(pr.x, 350.0, "parent should be at x=350 (after spacer)");
        assert_eq!(or_.x, 230.0, "overlay should flip left: 350 - 120 = 230");
    }

    // ── Nested overlay insertion order ───────────────────────────────────────

    #[test]
    fn nested_overlay_correct_positions() {
        // root (800×600) → parent (in-flow, 100×40 at top-left)
        //   → overlay1 (z_layer 1, below parent, 100×80)
        //     → overlay2 (z_layer 2, below overlay1, 100×60)
        let mut tree = LayoutTree::new();
        let root = tree.add(
            None,
            Atom {
                width: Fixed(800.0),
                height: Fixed(600.0),
                ..Default::default()
            },
            (),
        );
        let parent = tree.add(
            Some(root),
            Atom {
                width: Fixed(100.0),
                height: Fixed(40.0),
                ..Default::default()
            },
            (),
        );
        let below = OverlayPosition {
            parent_x: AxisAnchor::Start,
            parent_y: AxisAnchor::End,
            self_x: AxisAnchor::Start,
            self_y: AxisAnchor::Start,
            offset: (0.0, 0.0),
            flip_x: false,
            flip_y: false,
        };
        let ov1 = tree.add(
            Some(parent),
            Atom {
                width: Fixed(100.0),
                height: Fixed(80.0),
                position: Position::OutOfFlow(below),
                z_layer: 1,
                ..Default::default()
            },
            (),
        );
        let ov2 = tree.add(
            Some(ov1),
            Atom {
                width: Fixed(100.0),
                height: Fixed(60.0),
                position: Position::OutOfFlow(below),
                z_layer: 2,
                ..Default::default()
            },
            (),
        );
        // out_of_flow_nodes must have parent entry before child entry.
        assert_eq!(tree.out_of_flow_nodes[0].1, ov1, "ov1 stashed first");
        assert_eq!(tree.out_of_flow_nodes[1].1, ov2, "ov2 stashed second");

        tree.compute_layout(|_, _| None);
        let pr = node_result(&tree, parent);
        let r1 = node_result(&tree, ov1);
        let r2 = node_result(&tree, ov2);
        assert_eq!(r1.x, pr.x, "ov1 x should match parent x");
        assert_eq!(r1.y, pr.y + pr.height, "ov1 y should be below parent");
        assert_eq!(r2.x, r1.x, "ov2 x should match ov1 x");
        assert_eq!(r2.y, r1.y + r1.height, "ov2 y should be below ov1");
    }
}
