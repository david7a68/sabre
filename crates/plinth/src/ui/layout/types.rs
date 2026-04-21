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
