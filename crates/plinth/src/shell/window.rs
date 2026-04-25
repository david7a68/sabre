use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: Cow<'static, str>,
    pub width: u32,
    pub height: u32,
}
