use smallvec::SmallVec;

use crate::graphics::ClipRect;
use crate::ui::layout::compute::compute_clip_rects;

use super::compute::HorizontalMode;
use super::compute::compute_major_axis_fit_sizes;
use super::compute::compute_major_axis_grow_sizes;
use super::compute::compute_major_axis_offsets;
use super::compute::compute_minor_axis_fit_sizes;
use super::compute::compute_minor_axis_grow_sizes;
use super::compute::compute_minor_axis_offsets;
use super::compute::compute_overlay_positions;
use super::compute::compute_text_heights;
use super::types::Alignment;
use super::types::LayoutDirection;
use super::types::Padding;
use super::types::Position;
use super::types::Size;

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

pub(super) type NodeIndexArray = SmallVec<[UiElementId; 8]>;

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

#[cfg(test)]
mod tests {
    use super::super::types::Size::*;
    use super::super::types::{AxisAnchor, OverlayPosition};
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
