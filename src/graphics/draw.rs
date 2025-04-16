use bytemuck::Pod;
use bytemuck::Zeroable;

const TRIANGLES_PER_PRIMITIVE: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Primitive {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub color: [f32; 4],
}

pub enum DrawCommand {
    Clear,
    Draw { num_primitives: u32 },
}

pub struct Canvas {
    clear_color: [f32; 4],
    commands: Vec<DrawCommand>,
    primitives: Vec<Primitive>,
}

impl Canvas {}
