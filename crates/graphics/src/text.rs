use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Arc;

use parley::AlignmentOptions;
use parley::FontContext;
use parley::GlyphRun;
use parley::Layout;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use smallvec::SmallVec;
use swash::FontRef;
use swash::GlyphId;
use swash::scale::Render;
use swash::scale::ScaleContext;
use swash::scale::Source;
use swash::scale::StrikeWith;
use swash::scale::image::Content;
use swash::scale::image::Image;
use swash::zeno::Format;
use swash::zeno::Vector;
use tracing::instrument;

use crate::Color;
use crate::Primitive;
use crate::Texture;
use crate::draw::CanvasStorage;
use crate::texture::TextureManager;

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub align: TextAlignment,
    pub font_color: Color,
    pub font_size: f32,
    pub font_style: FontStyle,
    pub font_weight: FontWeight,
    pub font: Arc<FontStack<'static>>,
    pub strikethrough_color: Option<Color>,
    pub strikethrough_offset: Option<f32>,
    pub underline_color: Option<Color>,
    pub underline_offset: Option<f32>,
}

impl TextStyle {
    pub fn as_defaults(&self, builder: &mut parley::RangedBuilder<Color>) {
        builder.push_default(parley::StyleProperty::Brush(self.font_color));
        builder.push_default(parley::StyleProperty::FontSize(self.font_size));
        builder.push_default(parley::StyleProperty::FontStyle(self.font_style.into()));
        builder.push_default(parley::StyleProperty::FontWeight(self.font_weight.into()));

        match self.font.as_ref() {
            FontStack::Source(cow) => {
                builder.push_default(parley::StyleProperty::FontStack(parley::FontStack::Source(
                    cow.clone(),
                )));
            }
            FontStack::Single(font_family) => {
                builder.push_default(parley::StyleProperty::FontStack(parley::FontStack::Single(
                    font_family.clone().into(),
                )));
            }
            FontStack::List(cow) => {
                let families = cow
                    .iter()
                    .cloned()
                    .map(|f| f.into())
                    .collect::<SmallVec<[parley::FontFamily; 4]>>();
                builder.push_default(parley::StyleProperty::FontStack(parley::FontStack::List(
                    Cow::Borrowed(&families),
                )));
            }
        }

        builder.push_default(parley::StyleProperty::StrikethroughBrush(
            self.strikethrough_color,
        ));
        builder.push_default(parley::StyleProperty::StrikethroughOffset(
            self.strikethrough_offset,
        ));
        builder.push_default(parley::StyleProperty::UnderlineBrush(self.underline_color));
        builder.push_default(parley::StyleProperty::UnderlineOffset(
            self.underline_offset,
        ));
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            align: TextAlignment::Start,
            font_color: Color::BLACK,
            font_size: 32.0,
            font_style: FontStyle::Normal,
            font_weight: FontWeight::NORMAL,
            font: Arc::new(FontStack::Source(Cow::Borrowed("system-ui"))),
            strikethrough_color: None,
            strikethrough_offset: None,
            underline_color: None,
            underline_offset: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TextAlignment {
    Start,
    Center,
    End,
    Justify,
}

impl From<TextAlignment> for parley::Alignment {
    fn from(value: TextAlignment) -> Self {
        match value {
            TextAlignment::Start => Self::Start,
            TextAlignment::Center => Self::Middle,
            TextAlignment::End => Self::End,
            TextAlignment::Justify => Self::Justified,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FontStyle {
    Normal,
    Italic,
}

impl From<FontStyle> for parley::FontStyle {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => parley::FontStyle::Normal,
            FontStyle::Italic => parley::FontStyle::Italic,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FontStack<'a> {
    Source(Cow<'a, str>),
    Single(FontFamily<'a>),
    List(Cow<'a, [FontFamily<'a>]>),
}

#[derive(Clone, Debug)]
pub enum FontFamily<'a> {
    Named(Cow<'a, str>),
    Cursive,
    Emoji,
    FangSong,
    Fantasy,
    Math,
    Monospace,
    SansSerif,
    Serif,
    SystemUi,
    UiMonospace,
    UiRounded,
    UiSansSerif,
    UiSerif,
}

impl<'a> From<FontFamily<'a>> for parley::FontFamily<'a> {
    fn from(value: FontFamily<'a>) -> Self {
        match value {
            FontFamily::Named(name) => Self::Named(name),
            FontFamily::Cursive => Self::Generic(parley::GenericFamily::Cursive),
            FontFamily::Emoji => Self::Generic(parley::GenericFamily::Emoji),
            FontFamily::FangSong => Self::Generic(parley::GenericFamily::FangSong),
            FontFamily::Fantasy => Self::Generic(parley::GenericFamily::Fantasy),
            FontFamily::Math => Self::Generic(parley::GenericFamily::Math),
            FontFamily::Monospace => Self::Generic(parley::GenericFamily::Monospace),
            FontFamily::SansSerif => Self::Generic(parley::GenericFamily::SansSerif),
            FontFamily::Serif => Self::Generic(parley::GenericFamily::Serif),
            FontFamily::SystemUi => Self::Generic(parley::GenericFamily::SystemUi),
            FontFamily::UiMonospace => Self::Generic(parley::GenericFamily::UiMonospace),
            FontFamily::UiRounded => Self::Generic(parley::GenericFamily::UiRounded),
            FontFamily::UiSansSerif => Self::Generic(parley::GenericFamily::UiSansSerif),
            FontFamily::UiSerif => Self::Generic(parley::GenericFamily::UiSerif),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FontWeight(pub(crate) f32);

impl FontWeight {
    pub const THIN: Self = FontWeight(100.0);
    pub const EXTRA_LIGHT: Self = FontWeight(200.0);
    pub const LIGHT: Self = FontWeight(300.0);
    pub const SEMILIGHT: Self = FontWeight(350.0);
    pub const NORMAL: Self = FontWeight(400.0);
    pub const MEDIUM: Self = FontWeight(500.0);
    pub const SEMIBOLD: Self = FontWeight(600.0);
    pub const BOLD: Self = FontWeight(700.0);
    pub const EXTRA_BOLD: Self = FontWeight(800.0);
    pub const BLACK: Self = FontWeight(900.0);
    pub const EXTRA_BLACK: Self = FontWeight(950.0);
}

impl From<FontWeight> for parley::FontWeight {
    fn from(value: FontWeight) -> Self {
        parley::FontWeight::new(value.0)
    }
}

#[derive(Clone)]
pub(crate) struct TextSystem {
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

struct TextSystemInner {
    fonts: FontContext,
    layout_cx: LayoutContext<Color>,
    scaler_cx: ScaleContext,

    /// A cache of mappings from glyphs (and their aligned x-offsets) to textures.
    glyph_cache: HashMap<GlyphCacheKey, GlyphCacheEntry>,

    quick_layout: Layout<Color>,
}

impl TextSystemInner {
    fn new() -> Self {
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
    #[instrument(skip(self, canvas, textures, style))]
    fn simple_layout(
        &mut self,
        canvas: &mut CanvasStorage,
        textures: &TextureManager,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        origin: [f32; 2],
        color: Color,
    ) {
        let mut compute = self
            .layout_cx
            .ranged_builder(&mut self.fonts, text, 1.0, false);

        style.as_defaults(&mut compute);

        compute.build_into(&mut self.quick_layout, text);

        self.quick_layout.break_all_lines(max_width);
        self.quick_layout
            .align(max_width, style.align.into(), AlignmentOptions::default());

        let layout: &Layout<Color> = &self.quick_layout;

        for line in layout.lines() {
            for item in line.items() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyphs) => draw_glyph_run(
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

#[derive(Clone, Copy, Debug)]
struct SubpixelAlignment {
    step: u8,
    offset: f32,
}

impl SubpixelAlignment {
    fn new(value: f32) -> Self {
        let scaled = value.fract() * 3.0;
        let step = (scaled.round() % 3.0) as u8;
        let offset = scaled / 3.0;

        Self { step, offset }
    }
}

fn draw_glyph_run(
    scaler_cx: &mut ScaleContext,
    glyph_cache: &mut HashMap<GlyphCacheKey, GlyphCacheEntry>,
    canvas: &mut CanvasStorage,
    textures: &TextureManager,
    glyph_run: &GlyphRun<Color>,
    origin: [f32; 2],
) {
    let mut run_x = glyph_run.offset() + origin[0];
    let run_y = glyph_run.baseline() + origin[1];
    let style = glyph_run.style();
    let color = style.brush;

    let run = glyph_run.run();

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
            size: font_size as u8,
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
                .render_into(&mut scaler, glyph.id, &mut temp_glyph);

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

        canvas.draw(
            textures,
            Primitive::new(
                x.floor() + x_placement.offset + entry.left as f32,
                y.floor() + y_placement.offset - entry.top as f32,
                entry.width as f32,
                entry.height as f32,
                color,
            )
            .with_mask(entry.texture.clone()),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    font_id: u64,
    glyph: GlyphId,
    x_variant: u8,
    y_variant: u8,
    size: u8,
}

struct GlyphCacheEntry {
    texture: Texture,
    width: u8,
    height: u8,
    left: i32,
    top: i32,
}
