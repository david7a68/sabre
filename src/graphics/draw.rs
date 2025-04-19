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
            color: self.color,
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
}

#[derive(Clone, Copy, Debug)]
pub enum DrawCommand {
    Draw {
        texture: TextureId,
        num_vertices: u32,
    },
}

#[derive(Default)]
pub(crate) struct CanvasStorage {
    clear_color: Option<Color>,
    commands: Vec<DrawCommand>,
    primitives: Vec<GpuPrimitive>,

    textures: HashMap<TextureId, Texture>,
}

pub struct Canvas {
    storage: CanvasStorage,
    texture_manager: TextureManager,
    return_sender: mpsc::Sender<CanvasStorage>,
}

impl Canvas {
    pub(super) fn new(
        mut storage: CanvasStorage,
        texture_manager: TextureManager,
        return_sender: mpsc::Sender<CanvasStorage>,
    ) -> Self {
        storage.clear_color = None;
        storage.commands.clear();
        storage.primitives.clear();
        storage.textures.clear();

        storage.commands.push(DrawCommand::Draw {
            texture: TextureId::default(),
            num_vertices: 0,
        });

        let white_pixel = texture_manager.white_pixel();
        storage.textures.insert(white_pixel.id(), white_pixel);

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

    pub fn texture(&self, id: TextureId) -> Option<&Texture> {
        self.storage.textures.get(&id)
    }

    pub fn clear_color(&self) -> Option<Color> {
        self.storage.clear_color
    }

    pub fn clear(&mut self, clear_color: impl Into<Option<Color>>) {
        self.storage.clear_color = clear_color.into();
    }

    pub fn load_texture(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.texture_manager.load(path)
    }

    pub fn draw(&mut self, mut primitive: Primitive) {
        let texture = primitive.texture.take().unwrap_or(DrawTexture {
            uv: [0.0, 0.0],
            wh: [1.0, 1.0],
            texture: self.texture_manager.white_pixel(),
        });

        let DrawCommand::Draw {
            texture: prev_texture,
            num_vertices,
        } = self.storage.commands.last_mut().unwrap();

        if *prev_texture == texture.texture.id() {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            self.storage
                .textures
                .insert(texture.texture.id(), texture.texture.clone());

            self.storage.commands.push(DrawCommand::Draw {
                texture: texture.texture.id(),
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }

        self.storage.primitives.push(primitive.extract());
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
