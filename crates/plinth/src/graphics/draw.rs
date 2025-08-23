use std::collections::HashMap;
use std::path::Path;

use crate::graphics::TextureLoadError;
use crate::graphics::color::Color;
use crate::graphics::glyph_cache::GlyphCache;
use crate::graphics::pipeline::GpuPrimitive;
use crate::graphics::texture::StorageId;
use crate::graphics::texture::Texture;
use crate::graphics::texture::TextureManager;

const VERTICES_PER_PRIMITIVE: u32 = 6;

#[derive(Debug)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,

    pub color_texture: Option<Texture>,
    pub alpha_texture: Option<Texture>,
    pub use_nearest_sampling: bool,
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
            use_nearest_sampling: false,
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

    #[must_use]
    pub fn with_nearest_sampling(mut self) -> Self {
        self.use_nearest_sampling = true;
        self
    }
}

pub struct Canvas {
    storage: CanvasStorage,
    pub(super) texture_manager: TextureManager,
    glyph_cache: GlyphCache,
}

impl Canvas {
    pub(super) fn new(
        storage: CanvasStorage,
        glyph_cache: GlyphCache,
        texture_manager: TextureManager,
    ) -> Self {
        Self {
            storage,
            glyph_cache,
            texture_manager,
        }
    }

    pub(crate) fn storage(&self) -> &CanvasStorage {
        &self.storage
    }

    pub fn is_empty(&self) -> bool {
        self.storage.primitives.is_empty()
    }

    #[must_use]
    pub fn has_unready_textures(&self) -> bool {
        self.storage.has_unready_textures
    }

    pub fn reset(&mut self, clear_color: impl Into<Option<Color>>) {
        self.storage.clear_color = clear_color.into();
        self.storage.commands.clear();
        self.storage.primitives.clear();
        self.storage.textures.clear();
        self.storage.has_unready_textures = false;

        let white_pixel = self.texture_manager.white_pixel();
        let opaque_pixel = self.texture_manager.opaque_pixel();

        self.storage.commands.push(DrawCommand::Draw {
            color_storage_id: white_pixel.storage_id(),
            alpha_storage_id: opaque_pixel.storage_id(),
            num_vertices: 0,
        });

        self.storage
            .textures
            .insert(white_pixel.storage_id(), white_pixel.texture_view().clone());
        self.storage.textures.insert(
            opaque_pixel.storage_id(),
            opaque_pixel.texture_view().clone(),
        );
    }

    pub fn load_texture(&mut self, path: impl AsRef<Path>) -> Result<Texture, TextureLoadError> {
        self.texture_manager.load(path)
    }

    pub fn draw_text_layout(&mut self, layout: &parley::Layout<Color>, origin: [f32; 2]) {
        self.glyph_cache
            .draw(&mut self.storage, &self.texture_manager, layout, origin);
    }

    pub fn draw(&mut self, primitive: Primitive) {
        self.storage.push(&self.texture_manager, primitive);
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

#[derive(Default)]
pub(crate) struct CanvasStorage {
    clear_color: Option<Color>,
    commands: Vec<DrawCommand>,
    primitives: Vec<GpuPrimitive>,

    textures: HashMap<StorageId, wgpu::TextureView>,

    has_unready_textures: bool,
}

impl CanvasStorage {
    pub(crate) fn clear_color(&self) -> Option<Color> {
        self.clear_color
    }

    pub(crate) fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    pub(crate) fn primitives(&self) -> &[GpuPrimitive] {
        &self.primitives
    }

    pub(crate) fn texture_view(&self, id: StorageId) -> Option<&wgpu::TextureView> {
        self.textures.get(&id)
    }

    pub(crate) fn push(&mut self, texture_manager: &TextureManager, primitive: Primitive) {
        let Primitive {
            point,
            size,
            color: color_tint,
            color_texture,
            alpha_texture,
            use_nearest_sampling,
        } = primitive;

        let color_texture = color_texture
            .as_ref()
            .unwrap_or(texture_manager.white_pixel());
        let color_uvwh = color_texture.uvwh();

        let alpha_texture = alpha_texture
            .as_ref()
            .unwrap_or(texture_manager.opaque_pixel());
        let alpha_uvwh = alpha_texture.uvwh();

        if !color_texture.is_ready() || !alpha_texture.is_ready() {
            self.has_unready_textures = true;
            return;
        }

        self.primitives.push(GpuPrimitive {
            point,
            extent: size,
            color_uvwh,
            color_tint,
            alpha_uvwh,
            use_nearest_sampling: if use_nearest_sampling { 1 } else { 0 },
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
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
