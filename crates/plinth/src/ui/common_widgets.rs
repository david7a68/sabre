use crate::graphics::Texture;

use super::Size;
use super::widget::Button;
use super::widget::Container;
use super::widget::Dropdown;
use super::widget::DropdownItem;
use super::widget::EdgeRegionConfig;
use super::widget::EdgeRegionState;
use super::widget::EditableTextBuffer;
use super::widget::Frame;
use super::widget::HorizontalSeparator;
use super::widget::Image;
use super::widget::Interaction;
use super::widget::Label;
use super::widget::SectionPanelConfig;
use super::widget::SectionPanelState;
use super::widget::SplitPaneConfig;
use super::widget::SplitPaneState;
use super::widget::Surface;
use super::widget::TextEdit;
use super::widget::TextEditorState;
use super::widget::VerticalSeparator;

pub trait CommonWidgetsExt<'a>: Container<'a> {
    fn split_pane(
        &mut self,
        state: &mut SplitPaneState,
        config: SplitPaneConfig,
        first: impl FnOnce(&mut super::UiBuilder<'_>),
        second: impl FnOnce(&mut super::UiBuilder<'_>),
    ) -> &mut Self {
        state.show(self.builder_mut(), config, first, second);
        self
    }

    fn edge_region(
        &mut self,
        state: &mut EdgeRegionState,
        config: EdgeRegionConfig,
        region: impl FnOnce(&mut super::UiBuilder<'_>),
        remaining: impl FnOnce(&mut super::UiBuilder<'_>),
    ) -> &mut Self {
        state.show(self.builder_mut(), config, region, remaining);
        self
    }

    fn section_panel(
        &mut self,
        title: &str,
        state: &mut SectionPanelState,
        body: impl FnOnce(&mut super::UiBuilder<'_>),
    ) -> &mut Self {
        state.show(
            self.builder_mut(),
            title,
            SectionPanelConfig::default(),
            body,
        );
        self
    }

    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn frame<'this>(&'this mut self) -> Frame<'this>
    where
        'a: 'this,
    {
        Frame::new(self.builder_mut())
    }

    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn with_frame(&mut self, callback: impl FnOnce(Frame<'_>)) -> &mut Self {
        let container = self.frame();
        callback(container);
        self
    }

    fn image(&mut self, texture: &Texture, width: Size) {
        Image::new(self.builder_mut(), texture)
            .with_width(width)
            .finish()
    }

    fn surface<'this>(&'this mut self) -> Surface<'this>
    where
        'a: 'this,
    {
        Surface::new(self.builder_mut())
    }

    fn with_surface(&mut self, callback: impl FnOnce(Surface<'_>)) -> &mut Self {
        let panel = self.surface();
        callback(panel);
        self
    }

    fn text_button(&mut self, label: &str) -> Interaction {
        Button::new(self.builder_mut(), Some(label)).finish()
    }

    fn text_edit<'this, T>(&'this mut self, state: &'this TextEditorState<T>) -> TextEdit<'this, T>
    where
        T: EditableTextBuffer + 'static,
        'a: 'this,
    {
        TextEdit::new(self.builder_mut(), state)
    }

    fn label<'this>(&'this mut self, text: &str) -> Label<'this>
    where
        'a: 'this,
    {
        Label::new(self.builder_mut(), text)
    }

    fn horizontal_separator<'this>(&'this mut self) -> HorizontalSeparator<'this>
    where
        'a: 'this,
    {
        HorizontalSeparator::new(self.builder_mut())
    }

    fn vertical_separator<'this>(&'this mut self) -> VerticalSeparator<'this>
    where
        'a: 'this,
    {
        VerticalSeparator::new(self.builder_mut())
    }

    fn dropdown<D: DropdownItem>(
        &mut self,
        id: &str,
        label: &str,
        selected: Option<usize>,
        items: impl IntoIterator<Item = D>,
    ) -> Option<usize> {
        let mut dropdown = Dropdown::new(self.builder_mut(), id, label);

        for item in items.into_iter() {
            dropdown.item(item);
        }

        let (selected_idx, _) = dropdown.finish();
        selected_idx.or(selected)
    }
}

impl<'a, C: Container<'a>> CommonWidgetsExt<'a> for C {}
