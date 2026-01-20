use std::path::Path;

use crate::graphics::TextureLoadError;
use crate::graphics::color::Color;
use crate::graphics::glyph_cache::GlyphCache;
use crate::graphics::pipeline::GpuPaint;
use crate::graphics::pipeline::GpuPrimitive;
use crate::graphics::texture::StorageId;
use crate::graphics::texture::Texture;
use crate::graphics::texture::TextureManager;

use super::pipeline::PrimitiveRenderFlags;

const VERTICES_PER_PRIMITIVE: u32 = 6;

/// Defines how a primitive is painted - either with textures or a gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// Paint using sampled textures with a color tint.
    Sampled {
        color_tint: Color,
        color_texture: Option<Texture>,
        alpha_texture: Option<Texture>,
    },
    /// Paint using a linear gradient between two colors.
    /// Points are in normalized coordinates (0.0-1.0) within the primitive bounds.
    Gradient {
        color_a: Color,
        color_b: Color,
        /// Start point of the gradient in normalized coordinates (0.0-1.0).
        start: [f32; 2],
        /// End point of the gradient in normalized coordinates (0.0-1.0).
        end: [f32; 2],
    },
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Sampled {
            color_tint: Color::WHITE,
            color_texture: None,
            alpha_texture: None,
        }
    }
}

impl Paint {
    /// Create a solid color paint.
    pub fn solid(color: Color) -> Self {
        Paint::Sampled {
            color_tint: color,
            color_texture: None,
            alpha_texture: None,
        }
    }

    /// Create a textured paint with an optional color tint.
    pub fn textured(texture: Texture, tint: Color) -> Self {
        Paint::Sampled {
            color_tint: tint,
            color_texture: Some(texture),
            alpha_texture: None,
        }
    }

    /// Create a horizontal gradient from left to right.
    pub fn horizontal_gradient(left: Color, right: Color) -> Self {
        Paint::Gradient {
            color_a: left,
            color_b: right,
            start: [0.0, 0.5],
            end: [1.0, 0.5],
        }
    }

    /// Create a vertical gradient from top to bottom.
    pub fn vertical_gradient(top: Color, bottom: Color) -> Self {
        Paint::Gradient {
            color_a: top,
            color_b: bottom,
            start: [0.5, 0.0],
            end: [0.5, 1.0],
        }
    }

    /// Create a linear gradient with custom start and end points.
    /// Points are in normalized coordinates (0.0-1.0) within the primitive bounds.
    pub fn linear_gradient(color_a: Color, color_b: Color, start: [f32; 2], end: [f32; 2]) -> Self {
        Paint::Gradient {
            color_a,
            color_b,
            start,
            end,
        }
    }
}

#[derive(Debug)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub paint: Paint,
    pub use_nearest_sampling: bool,
}

impl Primitive {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            paint: Paint::solid(color),
            use_nearest_sampling: false,
        }
    }

    #[must_use]
    pub fn with_paint(x: f32, y: f32, width: f32, height: f32, paint: Paint) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            paint,
            use_nearest_sampling: false,
        }
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
        self.storage.has_unready_textures = false;

        let white_pixel = self.texture_manager.white_pixel();
        let opaque_pixel = self.texture_manager.opaque_pixel();

        self.storage.commands.push(DrawCommand::Draw {
            color_storage_id: white_pixel.storage_id(),
            alpha_storage_id: opaque_pixel.storage_id(),
            num_vertices: 0,
        });
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

    pub(crate) fn push(&mut self, texture_manager: &TextureManager, primitive: Primitive) {
        let Primitive {
            point,
            size,
            paint,
            use_nearest_sampling,
        } = primitive;

        let mut flags = PrimitiveRenderFlags::empty();
        flags.set(
            PrimitiveRenderFlags::USE_NEAREST_SAMPLING,
            use_nearest_sampling,
        );

        let (background_paint, color_texture, alpha_texture) = match &paint {
            Paint::Sampled {
                color_tint,
                color_texture,
                alpha_texture,
            } => {
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

                let paint = GpuPaint::sampled(*color_tint, color_uvwh, alpha_uvwh);

                (paint, color_texture, alpha_texture)
            }
            Paint::Gradient {
                color_a,
                color_b,
                start,
                end,
            } => {
                flags.set(PrimitiveRenderFlags::USE_GRADIENT_PAINT, true);

                (
                    GpuPaint::gradient(*color_a, *color_b, *start, *end),
                    texture_manager.white_pixel(),
                    texture_manager.opaque_pixel(),
                )
            }
        };

        self.primitives.push(GpuPrimitive {
            point,
            extent: size,
            background: background_paint,
            border_color: GpuPaint::gradient(Color::RED, Color::BLUE, [0.0, 0.5], [1.0, 0.5]),
            border_width: [2.0, 5.0, 2.0, 8.0],
            control_flags: flags,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
        });

        let DrawCommand::Draw {
            color_storage_id: prev_color_texture_id,
            alpha_storage_id: prev_alpha_texture_id,
            num_vertices,
        } = self.commands.last_mut().unwrap();

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
