use std::time::Duration;

use graphics::Canvas;
use graphics::Color;
use graphics::Primitive;
use smallvec::SmallVec;
use tracing::debug;

use crate::input::InputState;
use crate::layout::Layout;
use crate::layout::Size;
use crate::layout::compute_layout;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UiElementId(pub(crate) u16);

pub type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Debug, Default)]
pub struct UiElement {
    pub color: Color,

    pub width: Size,
    pub height: Size,

    // child layout properties
    pub inter_child_padding: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) layout: Layout,
    pub(crate) element: UiElement,

    pub(crate) parent: u16,
    pub(crate) sibling_order: u16,
}

#[derive(Debug, Default)]
pub struct UiContext {
    input: InputState,
    time_delta: Duration,

    nodes: Vec<Node>,
    children: Vec<NodeIndexArray>,
}

impl UiContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_frame(
        &mut self,
        input: InputState,
        time_delta: Duration,
        callback: impl FnOnce(&mut UiBuilder),
    ) -> &mut Self {
        self.nodes.clear();
        self.children.clear();

        self.nodes.push(Node {
            element: UiElement {
                color: Color::WHITE,
                width: input.window_size.width.into(),
                height: input.window_size.height.into(),
                inter_child_padding: 0.0,
            },
            ..Default::default()
        });
        self.children.push(NodeIndexArray::new());

        self.input = input;
        self.time_delta = time_delta;

        let mut recorder = UiBuilder {
            index: 0,
            context: self,
        };

        callback(&mut recorder);

        self
    }

    pub fn finish(&mut self, canvas: &mut Canvas) {
        compute_layout(&mut self.nodes, &self.children, UiElementId(0));

        for node in &self.nodes {
            let layout = &node.layout;

            debug!(?layout, color = ?node.element.color);

            canvas.draw(Primitive::new(
                layout.x.unwrap(),
                layout.y.unwrap(),
                layout.width.unwrap(),
                layout.height.unwrap(),
                node.element.color,
            ));
        }
    }
}

pub struct UiBuilder<'a> {
    index: usize,
    context: &'a mut UiContext,
}

impl UiBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn with_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.nodes[self.index].element.color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.nodes[self.index].element;
        element.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.nodes[self.index].element;
        element.height = height.into();
        self
    }

    pub fn with_spacing(&mut self, spacing: f32) -> &mut Self {
        let element = &mut self.context.nodes[self.index].element;
        element.inter_child_padding = spacing;
        self
    }

    pub fn add_element(&mut self) -> UiElementBuilder {
        let child_index = self.add(self.index);

        UiElementBuilder {
            context: self.context,
            index: child_index,
        }
    }

    pub fn with_element(&mut self, callback: impl FnOnce(&mut UiElementBuilder)) -> &mut Self {
        callback(&mut self.add_element());
        self
    }

    pub fn add_container(&mut self) -> UiBuilder {
        let child_index = self.add(self.index);

        UiBuilder {
            context: self.context,
            index: child_index,
        }
    }

    pub fn with_container(&mut self, callback: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        callback(&mut self.add_container());
        self
    }

    fn add(&mut self, parent: usize) -> usize {
        let child_index = self.context.nodes.len();

        self.context.children[parent].push(UiElementId(child_index as u16));
        let sibling_order = (self.context.children[parent].len() - 1) as u16;

        self.context.nodes.push(Node {
            parent: parent as u16,
            sibling_order,
            ..Default::default()
        });
        self.context.children.push(NodeIndexArray::new());

        child_index
    }
}

pub struct UiElementBuilder<'a> {
    context: &'a mut UiContext,
    index: usize,
}

impl UiElementBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn with_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.nodes[self.index].element.color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.nodes[self.index].element;
        element.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.nodes[self.index].element;
        element.height = height.into();
        self
    }
}
