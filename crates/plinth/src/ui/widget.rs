use std::hash::Hash;

use bytemuck::NoUninit;
use bytemuck::Pod;
use glamour::Contains;
use glamour::Rect;
use std::mem::size_of;

use crate::ui::Pixels;
use crate::ui::text::TextLayoutId;

use super::Alignment;
use super::LayoutDirection;
use super::UiBuilder;
use super::style::StateFlags;

mod button;
mod dropdown;
mod frame;
mod horizontal_separator;
mod image;
mod label;
pub(crate) mod macros;
mod surface;
mod text_edit;
mod vertical_separator;

pub use button::Button;
pub use dropdown::Dropdown;
pub use dropdown::DropdownItem;
pub use frame::Frame;
pub use horizontal_separator::HorizontalSeparator;
pub use image::Image;
pub use label::Label;
pub use surface::Surface;
pub use text_edit::TextEdit;
pub use vertical_separator::VerticalSeparator;

#[derive(Clone, Copy, Debug)]
pub struct Interaction {
    pub is_activated: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
}

pub trait Container<'a>: Sized {
    fn builder_mut(&mut self) -> &mut UiBuilder<'a>;

    fn child<'this>(&'this mut self) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.builder_mut().child()
    }

    fn named_child<'this>(&'this mut self, name: impl Hash) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.builder_mut().named_child(name)
    }

    fn child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.builder_mut().child_direction(direction);
        self
    }

    fn with_child_direction(mut self, direction: LayoutDirection) -> Self {
        self.child_direction(direction);
        self
    }

    fn child_alignment(&mut self, major: Alignment, minor: Alignment) -> &mut Self {
        self.builder_mut().child_alignment(major, minor);
        self
    }

    fn with_child_alignment(mut self, major: Alignment, minor: Alignment) -> Self {
        self.child_alignment(major, minor);
        self
    }
}

impl<'a> Container<'a> for UiBuilder<'a> {
    fn builder_mut(&mut self) -> &mut UiBuilder<'a> {
        self
    }

    fn child<'this>(&'this mut self) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.child()
    }

    fn named_child<'this>(&'this mut self, name: impl Hash) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.named_child(name)
    }
}

/// Controls when a click is registered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClickBehavior {
    /// Click fires immediately when mouse is pressed (more responsive).
    #[default]
    OnPress,
    /// Click fires when mouse is released while still hovered (standard UI behavior).
    OnRelease,
}

impl Interaction {
    /// Compute interaction state for a widget.
    ///
    /// Returns the interaction result and whether the widget is currently active (being pressed).
    pub fn compute(
        builder: &UiBuilder<'_>,
        behavior: ClickBehavior,
        interest: StateFlags,
    ) -> (Self, StateFlags) {
        let was_focused = builder.is_focused();

        // Layer-aware hit testing: a widget can only be hovered if no higher layer
        // has a widget under the pointer, and no modal overlay blocks this layer.
        // input_block_layer uses strict-less-than so that the modal overlay's own
        // children (which live at the same z_layer) are NOT blocked.
        let layer_blocked = builder.context.active_pointer_layer > builder.layer
            || builder
                .context
                .input_block_layer
                .is_some_and(|bl| builder.layer < bl);

        let (was_active, is_hovered) = builder
            .prev_state()
            .map(|s| {
                (
                    s.was_active,
                    !layer_blocked && s.placement.contains(&builder.input.pointer),
                )
            })
            .unwrap_or_default();

        let is_left_down = builder.input.mouse_state.is_left_down();
        let just_pressed = is_left_down && !was_active;
        let just_released = !is_left_down && was_active;

        let is_activated = match behavior {
            ClickBehavior::OnPress => is_hovered && just_pressed,
            ClickBehavior::OnRelease => is_hovered && just_released,
        };

        let mut state = StateFlags::NORMAL;
        if is_hovered {
            state |= StateFlags::HOVERED & interest;
        }
        if is_hovered && is_left_down {
            state |= StateFlags::PRESSED & interest;
        }
        if is_activated || ((is_hovered || !just_pressed) && was_focused) {
            state |= StateFlags::FOCUSED & interest;
        }

        (
            Self {
                is_activated,
                is_hovered,
                is_focused: state.contains(StateFlags::FOCUSED),
            },
            state,
        )
    }
}

#[repr(C, align(8))]
#[derive(Default)]
pub struct WidgetState {
    pub placement: Rect<Pixels>,

    // Placed immediately after placement to ensure that it is 8-byte aligned
    // for safe storage of any Pod type up to 8 bytes in size.
    custom_data: [u8; 8],

    pub text_layout: Option<TextLayoutId>,

    /// Whether the widget was being actively pressed last frame
    pub was_active: bool,
    /// The z_layer of the node this widget occupied last frame. Used to determine
    /// hit-test priority when multiple layers are present.
    pub layer: u8,
    /// Whether this widget's overlay was modal last frame (blocks input to lower layers).
    pub is_modal: bool,

    custom_data_size: u8,
}

impl WidgetState {
    /// Copy a [Pod] value previously stored with [set_custom_data].
    ///
    /// Returns `None` if no custom data has been written or `size_of::<T>() > 8`.
    pub fn custom_data<T: Pod>(&self) -> Option<T> {
        self.custom_data_ref().copied()
    }

    /// Store a [`NoUninit`] value in `custom_data`. Panics if `size_of::<T>() > 8`.
    pub fn set_custom_data<T: NoUninit>(&mut self, value: T) {
        let bytes = bytemuck::bytes_of(&value);
        let n = bytes.len();
        assert!(n <= 8, "custom_data holds at most 8 bytes, but T is {n}");
        self.custom_data.fill(0);
        self.custom_data[..n].copy_from_slice(bytes);
        self.custom_data_size = n as u8;
    }

    /// Return a reference to a [Pod] value previously stored with
    /// [set_custom_data].
    ///
    /// Returns `None` if no custom data has been written or if `size_of::<T>()`
    /// does not exactly match the size of the stored value.
    pub fn custom_data_ref<T: Pod>(&self) -> Option<&T> {
        if self.custom_data_size == 0 || size_of::<T>() != self.custom_data_size as usize {
            return None;
        }
        Some(bytemuck::from_bytes(&self.custom_data[..size_of::<T>()]))
    }

    /// Return a mutable reference to a [Pod] value previously stored with [set_custom_data].
    ///
    /// Returns `None` if no custom data has been written or if `size_of::<T>()` does
    /// not exactly match the size of the stored value.
    pub fn custom_data_mut<T: Pod>(&mut self) -> Option<&mut T> {
        if self.custom_data_size == 0 || size_of::<T>() != self.custom_data_size as usize {
            return None;
        }
        Some(bytemuck::from_bytes_mut(
            &mut self.custom_data[..size_of::<T>()],
        ))
    }

    pub fn has_custom_data(&self) -> bool {
        self.custom_data_size != 0
    }

    pub fn custom_data_bytes(&self) -> Option<[u8; 8]> {
        if self.custom_data_size == 0 {
            None
        } else {
            let data = &self.custom_data[..self.custom_data_size as usize];
            let mut padded = [0u8; 8];
            padded[..data.len()].copy_from_slice(data);
            Some(padded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_data_field_is_8byte_aligned() {
        let state = WidgetState::default();
        let addr = std::ptr::addr_of!(state.custom_data) as usize;
        assert_eq!(
            addr % 8,
            0,
            "custom_data must be 8-byte aligned for Pod casts"
        );
    }

    #[test]
    fn custom_data_no_write_returns_none() {
        let state = WidgetState::default();
        assert_eq!(state.custom_data::<u64>(), None);
        assert_eq!(state.custom_data_ref::<u64>(), None);
    }

    #[test]
    fn custom_data_u64_roundtrip() {
        let mut state = WidgetState::default();
        state.set_custom_data(0xDEAD_BEEF_DEAD_BEEFu64);
        assert_eq!(state.custom_data::<u64>(), Some(0xDEAD_BEEF_DEAD_BEEFu64));
        assert_eq!(
            state.custom_data_ref::<u64>(),
            Some(&0xDEAD_BEEF_DEAD_BEEFu64)
        );
    }

    #[test]
    fn custom_data_f64_roundtrip() {
        let mut state = WidgetState::default();
        state.set_custom_data(std::f64::consts::PI);
        assert_eq!(state.custom_data::<f64>(), Some(std::f64::consts::PI));
        assert_eq!(state.custom_data_ref::<f64>(), Some(&std::f64::consts::PI));
    }

    #[test]
    fn custom_data_i64_roundtrip() {
        let mut state = WidgetState::default();
        state.set_custom_data(i64::MIN);
        assert_eq!(state.custom_data::<i64>(), Some(i64::MIN));
        assert_eq!(state.custom_data_ref::<i64>(), Some(&i64::MIN));
    }

    #[test]
    fn custom_data_u32_roundtrip() {
        let mut state = WidgetState::default();
        state.set_custom_data(0xDEAD_BEEFu32);
        assert_eq!(state.custom_data::<u32>(), Some(0xDEAD_BEEFu32));
    }

    #[test]
    fn custom_data_size_mismatch_returns_none() {
        let mut state = WidgetState::default();
        state.set_custom_data(42u32);
        // Reads with mismatched sizes must return None.
        assert_eq!(state.custom_data::<u64>(), None);
        assert_eq!(state.custom_data::<[u32; 2]>(), None);
        assert_eq!(state.custom_data::<u8>(), None);
        // Same-size read still works.
        assert_eq!(state.custom_data::<u32>(), Some(42u32));
    }

    #[test]
    fn custom_data_overwrite() {
        let mut state = WidgetState::default();
        state.set_custom_data(1u64);
        state.set_custom_data(0xDEADu64);
        assert_eq!(state.custom_data::<u64>(), Some(0xDEADu64));
    }

    #[test]
    fn custom_data_mut_ref() {
        let mut state = WidgetState::default();
        state.set_custom_data(42u64);
        *state.custom_data_mut::<u64>().unwrap() = 100;
        assert_eq!(state.custom_data::<u64>(), Some(100u64));
    }

    #[test]
    fn custom_data_bytes_zero_padded_after_smaller_overwrite() {
        let mut state = WidgetState::default();
        state.set_custom_data(0x1122_3344_5566_7788u64);
        state.set_custom_data(0xAABB_CCDDu32);

        let mut expected = [0u8; 8];
        expected[..4].copy_from_slice(&0xAABB_CCDDu32.to_ne_bytes());
        assert_eq!(state.custom_data_bytes(), Some(expected));
    }
}
