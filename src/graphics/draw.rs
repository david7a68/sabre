use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::color::Color;

const VERTICES_PER_PRIMITIVE: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,
}

impl Primitive {
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            color,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DrawCommand {
    Draw { num_vertices: u32 },
}

#[derive(Debug, Default)]
pub struct Canvas {
    clear_color: Option<Color>,
    commands: Vec<DrawCommand>,
    primitives: Vec<Primitive>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            clear_color: None,
            commands: Vec::new(),
            primitives: Vec::new(),
        }
    }

    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }

    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    pub fn clear_color(&self) -> Option<Color> {
        self.clear_color
    }

    pub fn begin(&mut self, clear_color: impl Into<Option<Color>>) {
        self.commands.clear();
        self.primitives.clear();

        self.clear_color = clear_color.into();
    }

    pub fn draw(&mut self, primitive: Primitive) {
        if let Some(DrawCommand::Draw { num_vertices }) = self.commands.last_mut() {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            self.commands.push(DrawCommand::Draw {
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }

        self.primitives.push(primitive);
    }
}
