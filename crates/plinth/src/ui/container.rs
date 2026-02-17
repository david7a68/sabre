use std::hash::Hash;

use super::Alignment;
use super::LayoutDirection;
use super::UiBuilder;

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
