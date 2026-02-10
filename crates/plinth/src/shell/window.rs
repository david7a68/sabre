use std::borrow::Cow;

use slotmap::new_key_type;

use super::Input;

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: Cow<'static, str>,
    pub width: u32,
    pub height: u32,
}

new_key_type! {
    pub(super) struct ViewportId;
}

pub(super) struct Viewport {
    pub(super) input: Input,
    pub(super) config: WindowConfig,
}
