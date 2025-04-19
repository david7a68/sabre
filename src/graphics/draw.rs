use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::color::Color;

use super::texture_manager::Texture;
use super::texture_manager::TextureId;
use super::texture_manager::TextureLoadError;
use super::texture_manager::TextureManager;

const VERTICES_PER_PRIMITIVE: u32 = 6;

pub struct DrawTexture {
    pub uv: [f32; 2],
    pub wh: [f32; 2],
    pub texture: Texture,
}

pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,
    pub texture: Option<DrawTexture>,
}

impl Primitive {
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            color,
            texture: None,
        }
    }

    pub fn with_texture(mut self, texture: DrawTexture) -> Self {
        self.texture = Some(texture);
        self
    }

    fn extract(&self) -> GpuPrimitive {
        GpuPrimitive {
            point: self.point,
            size: self.size,
            uvwh: self.texture.as_ref().map_or([0.0; 4], |texture| {
                [texture.uv[0], texture.uv[1], texture.wh[0], texture.wh[1]]
            }),
            texture_id: 0,
            color: self.color,
            padding: [0; 3],
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPrimitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub uvwh: [f32; 4],
    pub color: Color,
    pub texture_id: u32,
    pub padding: [u32; 3],
}

#[derive(Clone, Copy, Debug)]
pub enum DrawCommand {
    Draw { num_vertices: u32 },
}

#[derive(Default)]
pub(crate) struct CanvasStorage {
    clear_color: Option<Color>,
    commands: Vec<DrawCommand>,
    primitives: Vec<GpuPrimitive>,

    textures: Vec<Texture>,
    texture_map: HashMap<TextureId, u16>,
}

pub struct Canvas {
    storage: CanvasStorage,
    texture_manager: TextureManager,
    return_sender: mpsc::Sender<CanvasStorage>,
}

impl Canvas {
    pub(super) fn new(
        storage: CanvasStorage,
        texture_manager: TextureManager,
        return_sender: mpsc::Sender<CanvasStorage>,
    ) -> Self {
        Self {
            storage,
            texture_manager,
            return_sender,
        }
    }

    pub(crate) fn primitives(&self) -> &[GpuPrimitive] {
        &self.storage.primitives
    }

    pub fn commands(&self) -> &[DrawCommand] {
        &self.storage.commands
    }

    pub fn clear_color(&self) -> Option<Color> {
        self.storage.clear_color
    }

    pub fn begin(&mut self, clear_color: impl Into<Option<Color>>) {
        self.storage.commands.clear();
        self.storage.primitives.clear();

        self.storage.clear_color = clear_color.into();
    }

    pub fn load_texture(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.texture_manager.load(path)
    }

    pub fn draw(&mut self, primitive: Primitive) {
        if let Some(DrawCommand::Draw { num_vertices }) = self.storage.commands.last_mut() {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            self.storage.commands.push(DrawCommand::Draw {
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }

        let mut gpu_primitive = primitive.extract();

        gpu_primitive.texture_id = primitive.texture.map_or(0, |texture| {
            let id = texture.texture.id();
            if let Some(index) = self.storage.texture_map.get(&id) {
                *index as u32
            } else {
                let index = self.storage.textures.len() as u16;
                self.storage.textures.push(texture.texture.clone());
                self.storage.texture_map.insert(id, index);
                index as u32
            }
        });

        self.storage.primitives.push(gpu_primitive);
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        if self
            .return_sender
            .send(std::mem::take(&mut self.storage))
            .is_err()
        {
            tracing::warn!("Failed to send canvas storage back to the pool");
        }
    }
}
