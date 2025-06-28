use std::time::Duration;

use graphics::Canvas;
use graphics::Color;
use graphics::Primitive;
use smallvec::SmallVec;

use crate::input::InputState;
use crate::layout::LayoutNode;
use crate::layout::LayoutNodeResult;
use crate::layout::LayoutNodeSpec;
use crate::layout::Padding;
use crate::layout::Size;
use crate::layout::compute_layout;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UiElementId(pub(crate) u16);

pub type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Debug, Default)]
pub(crate) struct UiElement {
    pub color: Color,
}

#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) element: UiElement,
}

#[derive(Debug, Default)]
pub struct UiContext {
    input: InputState,
    time_delta: Duration,

    ui_nodes: Vec<Node>,
    layout_nodes: Vec<LayoutNode>,

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
        self.ui_nodes.clear();
        self.layout_nodes.clear();
        self.children.clear();

        // Set up the root node.
        self.ui_nodes.push(Node {
            element: UiElement {
                color: Color::WHITE,
            },
        });
        self.layout_nodes.push(LayoutNode {
            spec: LayoutNodeSpec {
                width: input.window_size.width.into(),
                height: input.window_size.height.into(),
                inner_padding: Padding::default(),
                inter_child_padding: 0.0,
            },
            result: LayoutNodeResult::default(),
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
        compute_layout(&mut self.layout_nodes, &self.children, UiElementId(0));

        assert_eq!(self.ui_nodes.len(), self.layout_nodes.len());
        for (node, layout) in self.ui_nodes.iter().zip(&self.layout_nodes) {
            let layout = &layout.result;

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
        self.context.ui_nodes[self.index].element.color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.height = height.into();
        self
    }

    pub fn with_child_spacing(&mut self, spacing: f32) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.inter_child_padding = spacing;
        self
    }

    pub fn with_padding(&mut self, padding: Padding) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.inner_padding = padding;
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
        let child_index = self.context.ui_nodes.len();

        self.context.children[parent].push(UiElementId(child_index as u16));

        self.context.ui_nodes.push(Node::default());
        self.context.layout_nodes.push(LayoutNode::default());
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
        self.context.ui_nodes[self.index].element.color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        let element = &mut self.context.layout_nodes[self.index].spec;
        element.height = height.into();
        self
    }
}
