use std::borrow::Cow;
use std::sync::Arc;

use smallvec::SmallVec;

use crate::Color;

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub align: TextAlignment,
    pub font_color: Color,
    pub font_size: f32,
    pub font_style: FontStyle,
    pub font_features: Cow<'static, [FontFeature]>,
    pub font_weight: FontWeight,
    pub font: Arc<FontStack>,
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

        let features = self
            .font_features
            .iter()
            .map(|f| (*f).into())
            .collect::<SmallVec<[_; 16]>>();

        builder.push_default(parley::StyleProperty::FontFeatures(
            parley::FontSettings::List(features.as_slice().into()),
        ));

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
            font_features: Cow::Borrowed(&[
                FontFeature::ContextualLigatures,
                FontFeature::DiscretionaryLigatures,
                FontFeature::Kerning,
                FontFeature::StandardLigatures,
            ]),
            font_weight: FontWeight::NORMAL,
            font: Arc::new(FontStack::Source(Cow::Borrowed("Times New Roman"))),
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
pub enum FontStack {
    Source(Cow<'static, str>),
    Single(FontFamily),
    List(Cow<'static, [FontFamily]>),
}

#[derive(Clone, Debug)]
pub enum FontFamily {
    Named(Cow<'static, str>),
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

impl From<FontFamily> for parley::FontFamily<'static> {
    fn from(value: FontFamily) -> Self {
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

#[derive(Clone, Copy, Debug)]
pub enum FontFeature {
    /// The CLIG feature tag for contextual ligatures.
    ///
    /// Should usually be enabled by default.
    ContextualLigatures,
    /// The GLIG feature tag for discretionary ligatures.
    ///
    /// May be enabled with other ligatures and should usually be disabled by default.
    DiscretionaryLigatures,
    /// The LIGA feature tag for standard ligatures.
    ///
    /// Should usually be enabled by default.
    StandardLigatures,
    /// The RLIG feature tag for required ligatures.
    ///
    /// Required for correct rendering of some scripts.
    RequiredLigatures,
    /// The KERN feature tag for kerning.
    ///
    /// Should usually be enabled by default.
    Kerning,
}

impl From<FontFeature> for parley::style::FontFeature {
    fn from(value: FontFeature) -> Self {
        match value {
            FontFeature::ContextualLigatures => Self::parse("\"clig\" on").unwrap(),
            FontFeature::DiscretionaryLigatures => Self::parse("\"dlig\" on").unwrap(),
            FontFeature::Kerning => Self::parse("\"kern\" on").unwrap(),
            FontFeature::RequiredLigatures => Self::parse("\"rlig\" on").unwrap(),
            FontFeature::StandardLigatures => Self::parse("\"liga\" on").unwrap(),
        }
    }
}
