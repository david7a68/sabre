use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

use crate::TextStyle;
use crate::color::Color;
use crate::pipeline::GpuPrimitive;
use crate::text::TextSystem;
use crate::texture::StorageId;

use super::texture::Texture;
use super::texture::TextureLoadError;
use super::texture::TextureManager;

const VERTICES_PER_PRIMITIVE: u32 = 6;

#[derive(Debug)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,

    pub color_texture: Option<Texture>,
    pub alpha_texture: Option<Texture>,
}

impl Primitive {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            color,
            color_texture: None,
            alpha_texture: None,
        }
    }

    #[must_use]
    pub fn with_texture(mut self, texture: Texture) -> Self {
        self.color_texture = Some(texture);
        self
    }

    pub fn with_mask(mut self, texture: Texture) -> Self {
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
            color_texture: None,
        }
    }

    pub fn with_max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn with_texture(mut self, texture: Texture) -> Self {
        self.color_texture = Some(texture);
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DrawCommand {
    Draw {
        color_storage_id: StorageId,
        alpha_storage_id: StorageId,
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
        storage.has_unready_textures = false;

        let white_pixel = texture_manager.white_pixel();
        let opaque_pixel = texture_manager.opaque_pixel();

        storage.commands.push(DrawCommand::Draw {
            color_storage_id: white_pixel.storage_id(),
            alpha_storage_id: opaque_pixel.storage_id(),
            num_vertices: 0,
        });

        storage
            .textures
            .insert(white_pixel.storage_id(), white_pixel.texture_view().clone());
        storage.textures.insert(
            opaque_pixel.storage_id(),
            opaque_pixel.texture_view().clone(),
        );

        Self {
            storage,
            text_system,
            texture_manager,
            return_sender,
        }
    }

    #[must_use]
    pub fn has_unready_textures(&self) -> bool {
        self.storage.has_unready_textures
    }

    #[must_use]
    pub(crate) fn primitives(&self) -> &[GpuPrimitive] {
        &self.storage.primitives
    }

    #[must_use]
    pub(crate) fn commands(&self) -> &[DrawCommand] {
        &self.storage.commands
    }

    #[must_use]
    pub fn texture_view(&self, id: StorageId) -> Option<&wgpu::TextureView> {
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

    textures: HashMap<StorageId, wgpu::TextureView>,

    has_unready_textures: bool,
}

impl CanvasStorage {
    pub(crate) fn draw(&mut self, texture_manager: &TextureManager, primitive: Primitive) {
        let Primitive {
            point,
            size,
            color: color_tint,
            color_texture,
            alpha_texture,
        } = primitive;

        let color_texture = color_texture
            .as_ref()
            .unwrap_or(texture_manager.white_pixel());
        let color_uvwh = color_texture.uvwh();

        let alpha_texture = alpha_texture
            .as_ref()
            .unwrap_or(texture_manager.opaque_pixel());
        let alpha_uvwh = alpha_texture.uvwh();

        if !color_texture.is_ready() | !alpha_texture.is_ready() {
            self.has_unready_textures = true;
            return;
        }

        self.primitives.push(GpuPrimitive {
            point,
            size,
            color_uvwh,
            color_tint,
            alpha_uvwh,
        });

        let DrawCommand::Draw {
            color_storage_id: prev_color_texture_id,
            alpha_storage_id: prev_alpha_texture_id,
            num_vertices,
        } = self.commands.last_mut().unwrap();

        self.textures.insert(
            color_texture.storage_id(),
            color_texture.texture_view().clone(),
        );

        self.textures.insert(
            alpha_texture.storage_id(),
            alpha_texture.texture_view().clone(),
        );

        if color_texture.storage_id() == *prev_color_texture_id
            && alpha_texture.storage_id() == *prev_alpha_texture_id
        {
            *num_vertices += VERTICES_PER_PRIMITIVE;
        } else {
            self.commands.push(DrawCommand::Draw {
                color_storage_id: color_texture.storage_id(),
                alpha_storage_id: alpha_texture.storage_id(),
                num_vertices: VERTICES_PER_PRIMITIVE,
            });
        }
    }
}
