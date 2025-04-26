use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

use bytemuck::Pod;
use bytemuck::Zeroable;

use crate::TextStyle;
use crate::color::Color;
use crate::text::TextSystem;

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

    pub color_uvwh: Option<[f32; 4]>,
    pub color_texture: Option<Texture>,

    pub alpha_uvwh: Option<[f32; 4]>,
    pub alpha_texture: Option<Texture>,
}

impl Primitive {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            color,
            color_uvwh: None,
            color_texture: None,
            alpha_uvwh: None,
            alpha_texture: None,
        }
    }

    #[must_use]
    pub fn with_texture(mut self, texture: Texture, rect: impl Into<Option<[f32; 4]>>) -> Self {
        self.color_uvwh = rect.into();
        self.color_texture = Some(texture);
        self
    }

    pub fn with_mask(mut self, texture: Texture, rect: impl Into<Option<[f32; 4]>>) -> Self {
        self.alpha_uvwh = rect.into();
        self.alpha_texture = Some(texture);
        self
    }
}

#[derive(Debug)]
pub struct TextPrimitive<'a> {
    pub text: &'a str,
    pub style: &'a TextStyle,
    pub max_width: Option<f32>,

    pub point: [f32; 2],
    pub color: Color,

    pub color_uvwh: Option<[f32; 4]>,
    pub color_texture: Option<Texture>,
}

impl<'a> TextPrimitive<'a> {
    pub fn new(text: &'a str, style: &'a TextStyle, x: f32, y: f32, color: Color) -> Self {
        Self {
            text,
            style,
            max_width: None,
            point: [x, y],
            color,
            color_uvwh: None,
            color_texture: None,
        }
    }

    pub fn with_max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn with_texture(mut self, texture: Texture, rect: impl Into<Option<[f32; 4]>>) -> Self {
        self.color_uvwh = rect.into();
        self.color_texture = Some(texture);
        self
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPrimitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color_tint: Color,
    pub color_uvwh: [f32; 4],
    pub alpha_uvwh: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub enum DrawCommand {
    Draw {
        color_texture_id: TextureId,
        alpha_texture_id: TextureId,
        num_vertices: u32,
    },
}

pub struct Canvas {
    storage: CanvasStorage,
    pub(super) texture_manager: TextureManager,
    return_sender: mpsc::Sender<CanvasStorage>,
    text_system: TextSystem,
}

impl Canvas {
    pub(super) fn new(
        mut storage: CanvasStorage,
        text_system: TextSystem,
        texture_manager: TextureManager,
        return_sender: mpsc::Sender<CanvasStorage>,
    ) -> Self {
        storage.clear_color = None;
        storage.commands.clear();
        storage.primitives.clear();
        storage.textures.clear();

        let white_pixel = texture_manager.white_pixel();
        let opaque_pixel = texture_manager.opaque_pixel();

        storage.commands.push(DrawCommand::Draw {
            color_texture_id: white_pixel.id(),
            alpha_texture_id: opaque_pixel.id(),
            num_vertices: 0,
        });
        storage.textures.insert(white_pixel.id(), white_pixel);
        storage.textures.insert(opaque_pixel.id(), opaque_pixel);

        Self {
            storage,
            text_system,
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
        self.texture_manager.load(path)
    }

    pub fn draw_text(&mut self, text: TextPrimitive) {
        self.text_system.simple_layout(
            &mut self.storage,
            &self.texture_manager,
            text.text,
            text.style,
            text.max_width,
            text.point,
            text.color,
        );
    }

    pub fn draw(&mut self, primitive: Primitive) {
        self.storage.draw(&self.texture_manager, primitive);
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

#[derive(Default)]
pub(crate) struct CanvasStorage {
    clear_color: Option<Color>,
    commands: Vec<DrawCommand>,
    primitives: Vec<GpuPrimitive>,

    textures: HashMap<TextureId, Texture>,
}

impl CanvasStorage {
    pub fn draw(&mut self, texture_manager: &TextureManager, primitive: Primitive) {
        let Primitive {
            point,
            size,
            color: color_tint,
            color_uvwh,
            color_texture,
            alpha_uvwh,
            alpha_texture,
        } = primitive;

        let color_texture = color_texture.unwrap_or_else(|| texture_manager.white_pixel());
        let color_uvwh = {
            let original = color_uvwh.unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let textured = color_texture.uvwh();

            [
                original[0] + textured[0],
                original[1] + textured[1],
                original[2] * textured[2],
                original[3] * textured[3],
            ]
        };

        let alpha_texture = alpha_texture.unwrap_or_else(|| texture_manager.opaque_pixel());
        let alpha_uvwh = {
            let original = alpha_uvwh.unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let textured = alpha_texture.uvwh();

            [
                original[0] + textured[0],
                original[1] + textured[1],
                original[2] * textured[2],
                original[3] * textured[3],
            ]
        };

        let DrawCommand::Draw {
            color_texture_id: prev_color_texture,
            alpha_texture_id: prev_alpha_texture,
            num_vertices,
        } = self.commands.last_mut().unwrap();

        let is_same_color_texture = *prev_color_texture == color_texture.id();
        let is_same_alpha_texture = *prev_alpha_texture == alpha_texture.id();

        if is_same_color_texture && is_same_alpha_texture {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            if !is_same_color_texture {
                self.textures
                    .insert(color_texture.id(), color_texture.clone());
            }

            if !is_same_alpha_texture {
                self.textures
                    .insert(alpha_texture.id(), alpha_texture.clone());
            }

            self.commands.push(DrawCommand::Draw {
                color_texture_id: color_texture.id(),
                alpha_texture_id: alpha_texture.id(),
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }

        let prim = GpuPrimitive {
            point,
            size,
            color_uvwh,
            color_tint,
            alpha_uvwh,
        };

        self.primitives.push(prim);
    }
}
