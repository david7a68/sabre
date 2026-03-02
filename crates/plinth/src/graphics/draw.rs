use std::path::Path;

use crate::graphics::TextureLoadError;
use crate::graphics::color::Color;
use crate::graphics::glyph_cache::GlyphCache;
use crate::graphics::paint::GradientPaint;
use crate::graphics::paint::Paint;
use crate::graphics::shader_data::GpuPaint;
use crate::graphics::shader_data::GpuPrimitive;
use crate::graphics::shader_data::PrimitiveRenderFlags;
use crate::graphics::texture::StorageId;
use crate::graphics::texture::Texture;
use crate::graphics::texture::TextureManager;

use super::shader_data::GpuClip;

const VERTICES_PER_PRIMITIVE: u32 = 6;

#[derive(Debug)]
pub struct Primitive {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub paint: Paint,
    pub border: GradientPaint,
    /// Border widths in the order `[left, top, right, bottom]`.
    pub border_width: [f32; 4],
    pub corner_radii: [f32; 4],
    pub use_nearest_sampling: bool,
}

impl Primitive {
    #[must_use]
    pub fn with_paint(x: f32, y: f32, width: f32, height: f32, paint: Paint) -> Self {
        Self {
            point: [x, y],
            size: [width, height],
            paint,
            border: GradientPaint::vertical_gradient(Color::BLACK, Color::BLACK),
            border_width: [0.0, 0.0, 0.0, 0.0],
            corner_radii: [0.0; 4],
            use_nearest_sampling: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClipRect {
    pub point: [f32; 2],
    pub size: [f32; 2],
    pub fade: [f32; 4],
}

impl ClipRect {
    fn next(&self, next: &ClipRect) -> ClipRect {
        let x1 = self.point[0].max(next.point[0]);
        let y1 = self.point[1].max(next.point[1]);
        let x2 = (self.point[0] + self.size[0]).min(next.point[0] + next.size[0]);
        let y2 = (self.point[1] + self.size[1]).min(next.point[1] + next.size[1]);

        if x2 < x1 || y2 < y1 {
            // No intersection, return an empty rect
            return ClipRect {
                point: [0.0, 0.0],
                size: [0.0, 0.0],
                fade: [0.0; 4],
            };
        }

        ClipRect {
            point: [x1, y1],
            size: [x2 - x1, y2 - y1],
            fade: next.fade,
        }
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
        let white_pixel = self.texture_manager.white_pixel();
        let opaque_pixel = self.texture_manager.opaque_pixel();

        self.storage.reset(
            clear_color,
            white_pixel.storage_id(),
            opaque_pixel.storage_id(),
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
    clips: Vec<GpuClip>,

    clip_stack: Vec<ClipEntry>,

    has_unready_textures: bool,
}

impl CanvasStorage {
    pub fn clear_color(&self) -> Option<Color> {
        self.clear_color
    }

    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    pub fn primitives(&self) -> &[GpuPrimitive] {
        &self.primitives
    }

    pub fn clips(&self) -> &[GpuClip] {
        &self.clips
    }

    pub fn push_clip(&mut self, clip: ClipRect) {
        let next = if let Some(prev_resolved) = self.clip_stack.last() {
            prev_resolved.rect.next(&clip)
        } else {
            clip
        };

        self.clip_stack.push(ClipEntry {
            rect: clip,
            place: None,
        });
    }

    pub fn pop_clip(&mut self) {
        if self.clip_stack.len() > 1 {
            self.clip_stack.pop();
        }
    }

    pub fn reset(
        &mut self,
        clear_color: impl Into<Option<Color>>,
        white: StorageId,
        opaque: StorageId,
    ) {
        self.clear_color = clear_color.into();
        self.has_unready_textures = false;

        self.clips.clear();
        self.clips.push(GpuClip {
            point: [0.0, 0.0],
            extent: [f32::MAX, f32::MAX],
            fade: [0.0; 4],
        });

        self.clip_stack.clear();
        self.clip_stack.push(ClipEntry {
            rect: ClipRect {
                point: [0.0, 0.0],
                size: [f32::MAX, f32::MAX],
                fade: [0.0; 4],
            },
            place: Some(0),
        });

        self.commands.clear();
        self.primitives.clear();
        self.commands.push(DrawCommand::Draw {
            color_storage_id: white,
            alpha_storage_id: opaque,
            num_vertices: 0,
        });
    }

    pub fn push(&mut self, texture_manager: &TextureManager, primitive: Primitive) {
        let Primitive {
            point,
            size,
            paint,
            border,
            border_width,
            corner_radii,
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

        let current_clip = self.clip_stack.last_mut().unwrap();
        let clip_idx = *current_clip.place.get_or_insert_with(|| {
            let place = self.clips.len() as u32;

            self.clips.push(GpuClip {
                point: current_clip.rect.point,
                extent: current_clip.rect.size,
                fade: current_clip.rect.fade,
            });

            place
        });

        self.primitives.push(GpuPrimitive {
            point,
            extent: size,
            background: background_paint,
            border_color: GpuPaint::gradient(
                border.color_a,
                border.color_b,
                border.start,
                border.end,
            ),
            border_width,
            corner_radii,
            control_flags: flags,
            clip_idx,
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

struct ClipEntry {
    rect: ClipRect,
    place: Option<u32>,
}
