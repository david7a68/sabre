use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::color::Color;

use super::texture::Texture;
use super::texture::TextureId;
use super::texture::TextureLoadError;
use super::texture::TextureManager;

const VERTICES_PER_PRIMITIVE: u32 = 6;

#[derive(Debug)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,

    pub uvwh: Option<[f32; 4]>,
    pub texture: Option<Texture>,
}

impl Primitive {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            color,
            uvwh: None,
            texture: None,
        }
    }

    #[must_use]
    pub fn with_texture(mut self, texture: Texture, rect: impl Into<Option<[f32; 4]>>) -> Self {
        self.uvwh = rect.into();
        self.texture = Some(texture);
        self
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
        texture_id: TextureId,
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
    texture_manager: Rc<RefCell<TextureManager>>,
    return_sender: mpsc::Sender<CanvasStorage>,
}

impl Canvas {
    pub(super) fn new(
        mut storage: CanvasStorage,
        texture_manager: Rc<RefCell<TextureManager>>,
        return_sender: mpsc::Sender<CanvasStorage>,
    ) -> Self {
        storage.clear_color = None;
        storage.commands.clear();
        storage.primitives.clear();
        storage.textures.clear();

        let white_pixel = texture_manager.borrow().white_pixel();

        storage.commands.push(DrawCommand::Draw {
            texture_id: white_pixel.id(),
            num_vertices: 0,
        });
        storage.textures.insert(white_pixel.id(), white_pixel);

        Self {
            storage,
            texture_manager,
            return_sender,
        }
    }

    #[must_use]
    pub(crate) fn primitives(&self) -> &[GpuPrimitive] {
        &self.storage.primitives
    }

    #[must_use]
    pub fn commands(&self) -> &[DrawCommand] {
        &self.storage.commands
    }

    #[must_use]
    pub fn texture(&self, id: TextureId) -> Option<&Texture> {
        self.storage.textures.get(&id)
    }

    #[must_use]
    pub fn clear_color(&self) -> Option<Color> {
        self.storage.clear_color
    }

    pub fn clear(&mut self, clear_color: impl Into<Option<Color>>) {
        self.storage.clear_color = clear_color.into();
    }

    pub fn load_texture(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.texture_manager.borrow_mut().load(path)
    }

    pub fn draw(&mut self, primitive: Primitive) {
        let Primitive {
            point,
            size,
            color,
            uvwh,
            texture,
        } = primitive;

        let texture = texture.unwrap_or_else(|| self.texture_manager.borrow().white_pixel());
        let uvwh = {
            let original = uvwh.unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let textured = texture.uvwh();

            [
                original[0] + textured[0],
                original[1] + textured[1],
                original[2] * textured[2],
                original[3] * textured[3],
            ]
        };

        let DrawCommand::Draw {
            texture_id: prev_texture,
            num_vertices,
        } = self.storage.commands.last_mut().unwrap();

        if *prev_texture == texture.id() {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            self.storage.textures.insert(texture.id(), texture.clone());

            self.storage.commands.push(DrawCommand::Draw {
                texture_id: texture.id(),
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }

        let prim = GpuPrimitive {
            point,
            size,
            uvwh,
            color,
        };

        self.storage.primitives.push(prim);
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
