use std::borrow::Cow;

use parley::FontContext;
use parley::LayoutContext;

use crate::graphics::Color;

#[derive(Default)]
pub struct TextLayoutContext {
    pub(crate) fonts: FontContext,
    pub(crate) layouts: LayoutContext<Color>,
}

impl TextLayoutContext {
    pub(crate) fn drive<'a>(
        &'a mut self,
        editor: &'a mut parley::PlainEditor<Color>,
    ) -> parley::PlainEditorDriver<'a, Color> {
        editor.driver(&mut self.fonts, &mut self.layouts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
            TextAlignment::Center => Self::Center,
            TextAlignment::End => Self::End,
            TextAlignment::Justify => Self::Justify,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Font {
    pub family: FontStack,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: FontStack::Source(Cow::Borrowed("serif")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontStack {
    Source(Cow<'static, str>),
    Single(FontFamily),
    List(Cow<'static, [FontFamily]>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
