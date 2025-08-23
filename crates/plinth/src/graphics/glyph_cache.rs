use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::rc::Rc;

use parley::GlyphRun;
use parley::Layout;
use parley::PositionedLayoutItem;
use parley::swash::FontRef;
use parley::swash::GlyphId;
use parley::swash::scale::Render;
use parley::swash::scale::ScaleContext;
use parley::swash::scale::Source;
use parley::swash::scale::StrikeWith;
use parley::swash::scale::image::Content;
use parley::swash::scale::image::Image;
use parley::swash::zeno::Format;
use parley::swash::zeno::Vector;
use swash as _;
use tracing::instrument; // so that we can enable the "scale" feature

use crate::graphics::Color;
use crate::graphics::Primitive;
use crate::graphics::Texture;
use crate::graphics::draw::CanvasStorage;
use crate::graphics::texture::TextureManager;

#[derive(Clone)]
pub(crate) struct GlyphCache {
    inner: Rc<RefCell<GlyphCacheInner>>,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphCache {
    pub fn new() -> Self {
        let inner = Rc::new(RefCell::new(GlyphCacheInner::new()));
        Self { inner }
    }

    pub fn draw(
        &self,
        canvas: &mut CanvasStorage,
        textures: &TextureManager,
        layout: &Layout<Color>,
        origin: [f32; 2],
    ) {
        self.inner
            .borrow_mut()
            .draw(canvas, textures, layout, origin);
    }
}

struct GlyphCacheInner {
    scaler_cx: ScaleContext,

    /// A cache of mappings from glyphs (and their aligned x-offsets) to textures.
    glyph_cache: HashMap<GlyphCacheKey, GlyphCacheEntry>,

    /// Scratch space for rendering glyphs.
    image_place: Image,
}

impl GlyphCacheInner {
    fn new() -> Self {
        let scaler_cx = ScaleContext::new();

        Self {
            scaler_cx,
            glyph_cache: HashMap::new(),
            image_place: Image::new(),
        }
    }

    #[instrument(skip_all)]
    fn draw(
        &mut self,
        canvas: &mut CanvasStorage,
        textures: &TextureManager,
        layout: &Layout<Color>,
        origin: [f32; 2],
    ) {
        for line in layout.lines() {
            for item in line.items() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyphs) => draw_glyph_run(
                        &mut self.scaler_cx,
                        &mut self.image_place,
                        &mut self.glyph_cache,
                        canvas,
                        textures,
                        &glyphs,
                        origin,
                    ),
                    PositionedLayoutItem::InlineBox(_) => {}
                }
            }
        }
    }
}

const SUBPIXEL_VARIANTS: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
struct SubpixelAlignment {
    step: u8,
    offset: f32,
}

impl SubpixelAlignment {
    fn new(value: f32) -> Self {
        let fraction = value.fract();

        let scaled = fraction * SUBPIXEL_VARIANTS;
        let step = scaled.round() as u8 % SUBPIXEL_VARIANTS as u8;

        Self {
            step,
            offset: fraction,
        }
    }
}

fn draw_glyph_run(
    scaler_cx: &mut ScaleContext,
    temp_glyph: &mut Image,
    glyph_cache: &mut HashMap<GlyphCacheKey, GlyphCacheEntry>,
    canvas: &mut CanvasStorage,
    textures: &TextureManager,
    glyph_run: &GlyphRun<Color>,
    origin: [f32; 2],
) {
    let mut run_x = glyph_run.offset() + origin[0].floor();
    let run_y = glyph_run.baseline() + origin[1].floor();
    let style = glyph_run.style();
    let color = style.brush;

    let run = glyph_run.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();
    let normalized_coords = run.normalized_coords();

    // Convert from parley::Font to swash::FontRef. Should always succeed since
    // parley created and owns the `Font`.
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();

    let mut scaler = scaler_cx
        .builder(font_ref)
        .size(font_size)
        .hint(true)
        .normalized_coords(normalized_coords)
        .build();

    for glyph in glyph_run.glyphs() {
        let x = run_x + glyph.x;
        let y = run_y - glyph.y;
        run_x += glyph.advance;

        // figure out which glyph offset variant to use
        let x_placement = SubpixelAlignment::new(x);
        let y_placement = SubpixelAlignment::new(y);

        let key = GlyphCacheKey {
            font_id: font.data.id(),
            glyph: glyph.id,
            x_variant: x_placement.step,
            y_variant: y_placement.step,
            size: font_size as u16,
        };

        let entry = match glyph_cache.entry(key) {
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
            Entry::Vacant(vacant_entry) => {
                temp_glyph.clear();

                let offset = Vector::new(x_placement.offset, y_placement.offset);

                let success = Render::new(&[
                    Source::ColorOutline(0),
                    Source::ColorBitmap(StrikeWith::BestFit),
                    Source::Bitmap(StrikeWith::BestFit),
                    Source::Outline,
                ])
                .format(Format::Alpha)
                .offset(offset)
                .render_into(&mut scaler, glyph.id, temp_glyph);

                assert!(success);

                if temp_glyph.placement.height == 0 {
                    continue;
                }

                let format = match temp_glyph.content {
                    Content::Color => wgpu::TextureFormat::Rgba8UnormSrgb,
                    Content::Mask => wgpu::TextureFormat::R8Unorm,
                    _ => unimplemented!(),
                };

                let texture = textures.load_from_memory(
                    &temp_glyph.data,
                    temp_glyph.placement.width as u16,
                    format,
                );

                vacant_entry.insert(GlyphCacheEntry {
                    texture,
                    width: temp_glyph.placement.width as u8,
                    height: temp_glyph.placement.height as u8,
                    left: temp_glyph.placement.left,
                    top: temp_glyph.placement.top,
                })
            }
        };

        let glyph_x = (x as i32 + entry.left) as f32;
        let glyph_y = (y as i32 - entry.top) as f32;

        canvas.push(
            textures,
            Primitive::new(
                glyph_x,
                glyph_y,
                entry.width as f32,
                entry.height as f32,
                color,
            )
            .with_mask(entry.texture.clone())
            .with_nearest_sampling(),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    font_id: u64,
    glyph: GlyphId,
    x_variant: u8,
    y_variant: u8,
    // We can't use `f32` here because it is not `Hash`.
    size: u16,
}

struct GlyphCacheEntry {
    texture: Texture,
    width: u8,
    height: u8,
    left: i32,
    top: i32,
}
