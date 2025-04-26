use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

use parley::AlignmentOptions;
use parley::FontContext;
use parley::FontStack;
use parley::GlyphRun;
use parley::Layout;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use swash::CacheKey;
use swash::FontRef;
use swash::GlyphId;
use swash::scale::Render;
use swash::scale::ScaleContext;
use swash::scale::Source;
use swash::scale::StrikeWith;
use swash::scale::image::Content;
use swash::scale::image::Image;
use swash::zeno::Vector;

use crate::Color;
use crate::Primitive;
use crate::Texture;
use crate::draw::CanvasStorage;
use crate::texture::TextureManager;

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    font_stack: FontStack<'static>,
    font_size: f32,
    align: parley::Alignment,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_stack: FontStack::Source(Cow::Borrowed("Arial, sans-serif")),
            font_size: 16.0,
            align: parley::Alignment::Start,
        }
    }
}

#[derive(Clone)]
pub struct TextSystem {
    inner: Rc<RefCell<TextSystemInner>>,
}

impl TextSystem {
    pub fn new() -> Self {
        let inner = Rc::new(RefCell::new(TextSystemInner::new()));
        Self { inner }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn simple_layout(
        &self,
        canvas: &mut CanvasStorage,
        textures: &TextureManager,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        origin: [f32; 2],
        color: Color,
    ) {
        self.inner
            .borrow_mut()
            .simple_layout(canvas, textures, text, style, max_width, origin, color);
    }
}

struct TextSystemInner {
    fonts: FontContext,
    layout_cx: LayoutContext<Color>,
    scaler_cx: ScaleContext,

    /// A cache of mappings from glyphs (and their aligned x-offsets) to textures.
    glyph_cache: HashMap<GlyphCacheKey, GlyphCacheEntry>,

    quick_layout: Layout<Color>,
}

impl TextSystemInner {
    pub fn new() -> Self {
        let fonts = FontContext::new();
        let layout_cx = LayoutContext::new();
        let scaler_cx = ScaleContext::new();

        let quick_layout = Layout::new();

        Self {
            fonts,
            layout_cx,
            scaler_cx,
            glyph_cache: HashMap::new(),
            quick_layout,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn simple_layout(
        &mut self,
        canvas: &mut CanvasStorage,
        textures: &TextureManager,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        origin: [f32; 2],
        color: Color,
    ) {
        let mut compute = self.layout_cx.ranged_builder(&mut self.fonts, text, 1.0);

        compute.push_default(StyleProperty::FontStack(style.font_stack.clone()));
        compute.push_default(StyleProperty::FontSize(style.font_size));
        compute.push_default(StyleProperty::Brush(color));

        compute.build_into(&mut self.quick_layout, text);

        self.quick_layout.break_all_lines(max_width);
        self.quick_layout
            .align(max_width, style.align, AlignmentOptions::default());

        let layout: &Layout<Color> = &self.quick_layout;
        for line in layout.lines() {
            for item in line.items() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyphs) => draw_run(
                        &mut self.scaler_cx,
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

fn quant_float_4(value: f32) -> (u8, f32) {
    let quant = (value * 40.0).round();
    (quant as u8, quant / 40.0)
}

fn draw_run(
    scaler_cx: &mut ScaleContext,
    glyph_cache: &mut HashMap<GlyphCacheKey, GlyphCacheEntry>,
    canvas: &mut CanvasStorage,
    textures: &TextureManager,
    glyphs: &GlyphRun<Color>,
    origin: [f32; 2],
) {
    let mut run_x = glyphs.offset() + origin[0];
    let run_y = glyphs.baseline() + origin[1];
    let style = glyphs.style();
    let color = style.brush;

    let run = glyphs.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();
    let normalized_coords = run.normalized_coords();

    // Convert from parley::Font to swash::FontRef
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();
    let mut scaler = scaler_cx
        .builder(font_ref)
        .size(font_size)
        .hint(true)
        .normalized_coords(normalized_coords)
        .build();

    let mut temp_glyph = Image::new();

    for glyph in glyphs.glyphs() {
        let x = run_x + glyph.x;
        let y = run_y + glyph.y;
        run_x += glyph.advance;

        // figure out which glyph offset variant to use
        let (x_variant, x_offset) = quant_float_4(glyph.x);
        let (y_variant, y_offset) = quant_float_4(glyph.y);

        match glyph_cache.entry(GlyphCacheKey {
            font: font_ref.key,
            glyph: glyph.id,
            x_variant,
            y_variant,
            size: font_size as u8,
        }) {
            Entry::Occupied(occupied_entry) => {
                let entry = occupied_entry.get();
                canvas.draw(
                    textures,
                    Primitive::new(x, y, entry.width as f32, entry.height as f32, color)
                        .with_mask(entry.texture.clone(), None),
                );
            }
            Entry::Vacant(vacant_entry) => {
                temp_glyph.clear();

                let success = Render::new(&[
                    Source::Bitmap(StrikeWith::BestFit),
                    Source::ColorBitmap(StrikeWith::BestFit),
                    Source::Outline,
                ])
                .format(swash::zeno::Format::Alpha)
                .offset(Vector::new(x_offset, y_offset))
                .render_into(&mut scaler, glyph.id, &mut temp_glyph);

                assert!(success);

                if temp_glyph.placement.height == 0 || temp_glyph.placement.width == 0 {
                    // This is a zero-height glyph, so we can skip it.
                    continue;
                }

                let format = match temp_glyph.content {
                    Content::Color => wgpu::TextureFormat::Rgba8UnormSrgb,
                    Content::Mask => wgpu::TextureFormat::R8Unorm,
                    _ => unimplemented!(),
                };

                let texture = textures.from_memory(
                    &temp_glyph.data,
                    temp_glyph.placement.width as u16,
                    format,
                    None,
                );

                let entry = vacant_entry.insert(GlyphCacheEntry {
                    texture,
                    width: temp_glyph.placement.width as u8,
                    height: temp_glyph.placement.height as u8,
                });

                canvas.draw(
                    textures,
                    Primitive::new(x, y, entry.width as f32, entry.height as f32, color)
                        .with_mask(entry.texture.clone(), None),
                );
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    font: CacheKey,
    glyph: GlyphId,
    x_variant: u8,
    y_variant: u8,
    size: u8,
}

struct GlyphCacheEntry {
    texture: Texture,
    width: u8,
    height: u8,
}
