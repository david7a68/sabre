use std::borrow::Cow;
use std::sync::Arc;

use graphics::Color;
use smallvec::SmallVec;

#[derive(Clone, Debug)]
pub struct TextStyle {
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
    pub(crate) fn as_defaults(&self, builder: &mut parley::RangedBuilder<Color>) {
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
            font_color: Color::BLACK,
            font_size: 16.0,
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
