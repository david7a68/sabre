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
        + D::major_axis_padding_start(&node.spec)
        + D::major_axis_padding_end(&node.spec);

    let run = |nodes: &mut [LayoutNode], limit: f32| {
        let mut total_size = empty_size;
        let mut grow_children = NodeIndexArray::new();

        for child in &children[node_id.0 as usize] {
            if let Some(size) = D::major_size_result(&nodes[child.0 as usize].result) {
                total_size += size;
            } else if matches!(D::major_size_spec(&nodes[child.0 as usize].spec), Grow) {
                grow_children.push(*child);
            } else {
                compute_major_axis_flex_sizes::<D>(nodes, children, *child);
                total_size += D::major_size_result(&nodes[child.0 as usize].result).unwrap();
            }
        }

        if !grow_children.is_empty() {
            info!("there are {} grow children", grow_children.len());

            let grow_size = (limit - total_size).max(0.0) / grow_children.len() as f32;

            for child in &grow_children {
                let child_node = &mut nodes[child.0 as usize];
                D::set_major_size(&mut child_node.result, grow_size);
            }

            total_size = limit;
        }

        total_size
    };

    if let Size::Fit { min, max } = D::major_size_spec(&node.spec) {
        let total_size = run(nodes, max);
        let node = &mut nodes[node_id.0 as usize];
        D::set_major_size(&mut node.result, total_size.clamp(min, max));
    } else if let Some(size) = D::major_size_result(&node.result) {
        run(nodes, size);
    } else {
        warn!(
            "Node {node_id:?} has no {} size set and is not a flex container",
            D::string()
        );
        let node = &mut nodes[node_id.0 as usize];
        D::set_major_size(&mut node.result, 0.0);
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

    D::set_major_offset(&mut node.result, current_offset);

    let size = D::major_size_result(&node.result).unwrap();
    let padding = node.spec.inter_child_padding;

    let mut advance = current_offset + D::major_axis_padding_start(&node.spec);
    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        D::set_major_offset(&mut child_node.result, advance);
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

    if let Size::Fit { min, max } = D::minor_size_spec(&node.spec) {
        for child in &children[node_id.0 as usize] {
            let child_size = if let Some(h) = D::minor_size_result(&nodes[child.0 as usize].result)
            {
                h
            } else {
                compute_minor_axis_flex_sizes::<D>(nodes, children, *child);
                D::minor_size_result(&nodes[child.0 as usize].result).unwrap()
            };

            total_size = total_size.max(child_size);
        }

        let node = &mut nodes[node_id.0 as usize];

        total_size +=
            D::minor_axis_padding_start(&node.spec) + D::minor_axis_padding_end(&node.spec);

        D::set_minor_size(&mut node.result, total_size.clamp(min, max));
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
        return compute_major_axis_offsets::<D::Other>(nodes, children, node_id, current_offset);
    }

    D::set_minor_offset(&mut node.result, current_offset);

    let size = D::minor_size_result(&node.result).unwrap();
    let inset = current_offset + D::major_axis_padding_start(&node.spec);

    for child_id in &children[node_id.0 as usize] {
        let child_node = &mut nodes[child_id.0 as usize];

        D::set_minor_offset(&mut child_node.result, inset);
        compute_minor_axis_offsets::<D>(nodes, children, *child_id, inset);
    }

    inset + size
}

trait LayoutDirectionExt {
    type Other: LayoutDirectionExt;
    const DIRECTION: LayoutDirection;

    fn string() -> &'static str;

    fn major_size_spec(spec: &LayoutNodeSpec) -> Size;
    fn minor_size_spec(spec: &LayoutNodeSpec) -> Size;

    fn set_major_size(result: &mut LayoutNodeResult, size: f32);
    fn set_minor_size(result: &mut LayoutNodeResult, size: f32);

    fn major_size_result(result: &LayoutNodeResult) -> Option<f32>;
    fn minor_size_result(result: &LayoutNodeResult) -> Option<f32>;

    fn set_major_offset(result: &mut LayoutNodeResult, offset: f32);
    fn set_minor_offset(result: &mut LayoutNodeResult, offset: f32);

    fn major_axis_padding_start(spec: &LayoutNodeSpec) -> f32;
    fn major_axis_padding_end(spec: &LayoutNodeSpec) -> f32;

    fn minor_axis_padding_start(spec: &LayoutNodeSpec) -> f32;
    fn minor_axis_padding_end(spec: &LayoutNodeSpec) -> f32;
}

struct HorizontalMode;

impl LayoutDirectionExt for HorizontalMode {
    type Other = VerticalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Horizontal;

    fn string() -> &'static str {
        "horizontal"
    }

    fn major_size_spec(spec: &LayoutNodeSpec) -> Size {
        spec.width
    }

    fn minor_size_spec(spec: &LayoutNodeSpec) -> Size {
        spec.height
    }

    fn set_major_size(result: &mut LayoutNodeResult, size: f32) {
        result.width = Some(size);
    }

    fn set_minor_size(result: &mut LayoutNodeResult, size: f32) {
        result.height = Some(size);
    }

    fn major_size_result(result: &LayoutNodeResult) -> Option<f32> {
        result.width
    }

    fn minor_size_result(result: &LayoutNodeResult) -> Option<f32> {
        result.height
    }

    fn set_major_offset(result: &mut LayoutNodeResult, offset: f32) {
        result.x = Some(offset);
    }

    fn set_minor_offset(result: &mut LayoutNodeResult, offset: f32) {
        result.y = Some(offset);
    }

    fn major_axis_padding_start(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.left
    }

    fn major_axis_padding_end(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.right
    }

    fn minor_axis_padding_start(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.top
    }

    fn minor_axis_padding_end(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.bottom
    }
}

struct VerticalMode;

impl LayoutDirectionExt for VerticalMode {
    type Other = HorizontalMode;
    const DIRECTION: LayoutDirection = LayoutDirection::Vertical;

    fn string() -> &'static str {
        "vertical"
    }

    fn major_size_spec(spec: &LayoutNodeSpec) -> Size {
        spec.height
    }

    fn minor_size_spec(spec: &LayoutNodeSpec) -> Size {
        spec.width
    }

    fn set_major_size(spec: &mut LayoutNodeResult, size: f32) {
        spec.height = Some(size);
    }

    fn set_minor_size(spec: &mut LayoutNodeResult, size: f32) {
        spec.width = Some(size);
    }

    fn major_size_result(result: &LayoutNodeResult) -> Option<f32> {
        result.height
    }

    fn minor_size_result(result: &LayoutNodeResult) -> Option<f32> {
        result.width
    }

    fn set_major_offset(result: &mut LayoutNodeResult, offset: f32) {
        result.y = Some(offset);
    }

    fn set_minor_offset(result: &mut LayoutNodeResult, offset: f32) {
        result.x = Some(offset);
    }

    fn major_axis_padding_start(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.top
    }

    fn major_axis_padding_end(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.bottom
    }

    fn minor_axis_padding_start(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.left
    }

    fn minor_axis_padding_end(spec: &LayoutNodeSpec) -> f32 {
        spec.inner_padding.right
    }
}

impl std::fmt::Debug for VerticalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vertical")
    }
}
