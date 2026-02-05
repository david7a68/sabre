use std::borrow::Cow;

use smallvec::SmallVec;

use crate::graphics::Color;
use crate::graphics::FontStack;

use super::style::PropertyKey;
use super::style::StateFlags;
use super::style::Style;
use super::style::StyleError;
use super::style::StyleId;
use super::style::StyleProperty;
use super::style::StyleRegistry;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleClass {
    Panel = 0,
    Button,
    Label,
}

impl StyleClass {
    /// Number of style class variants. Update when adding new variants.
    pub const COUNT: usize = 3;
}

pub struct Theme {
    well_known_classes: [StyleId; StyleClass::COUNT],
    styles: StyleRegistry,
}

impl Theme {
    pub fn new() -> Self {
        let styles = StyleRegistry::default();

        let default_style = styles.default_style_id();

        Self {
            styles,
            well_known_classes: [default_style; StyleClass::COUNT],
        }
    }

    /// Gets the style assigned to a style class.
    pub fn get(&self, class: StyleClass) -> &Style {
        let styled_id = self.well_known_classes[class as usize];
        self.styles.get(styled_id).unwrap()
    }

    /// Gets the style ID assigned to a style class.
    pub fn get_id(&self, class: StyleClass) -> StyleId {
        self.well_known_classes[class as usize]
    }

    /// Assigns a style to a style class.
    pub fn set(&mut self, class: StyleClass, style_id: StyleId) {
        self.well_known_classes[class as usize] = style_id;
    }

    /// Sets properties on the default style.
    ///
    /// All styles inherit from the default style, so this is a convenient
    /// way to set global defaults.
    pub fn set_base_style(
        &mut self,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) {
        let default_style_id = self.styles.default_style_id();
        self.styles.update(default_style_id, properties);
    }

    /// Creates a new inline style and assigns it to a style class.
    ///
    /// This is a convenience method for creating and assigning a style in one step.
    pub fn set_class_properties(
        &mut self,
        class: StyleClass,
        parent: Option<StyleId>,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) -> Result<StyleId, StyleError> {
        let style = self.create_style(parent, properties)?;
        self.set(class, style);
        Ok(style)
    }

    /// Resolves a property for a specific style class and state combination.
    pub fn resolve<K: PropertyKey>(&self, style: StyleClass, state: StateFlags) -> K::Value {
        let style_id = self.well_known_classes[style as usize];
        self.styles.resolve::<K>(style_id, state)
    }

    /// Resolves a property for a specific style ID and state combination.
    pub fn resolve_style<K: PropertyKey>(&self, style_id: StyleId, state: StateFlags) -> K::Value {
        self.styles.resolve::<K>(style_id, state)
    }

    /// Creates a new style with the given parent and properties.
    ///
    /// The style can then be assigned to one or more `StyleClass`es using
    /// `Theme::set`.
    pub fn create_style(
        &mut self,
        parent: Option<StyleId>,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) -> Result<StyleId, StyleError> {
        let parent = parent.unwrap_or_else(|| self.styles.default_style_id());
        self.styles.register(Some(parent), properties)
    }

    /// Modifies an existing style by replacing its properties.
    ///
    pub fn update_style(
        &mut self,
        style_id: StyleId,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) {
        self.styles.update(style_id, properties)
    }

    pub(crate) fn push_text_defaults(
        &self,
        style_id: StyleId,
        state: StateFlags,
        builder: &mut parley::RangedBuilder<Color>,
    ) {
        use parley::StyleProperty as Prop;

        let style = self.styles.get(style_id).unwrap();

        builder.push_default(Prop::Brush(style.text_color.get(state)));
        builder.push_default(Prop::FontSize(style.font_size.get(state) as f32));
        builder.push_default(Prop::FontStyle(style.font_style.get(state).into()));
        builder.push_default(Prop::FontWeight(parley::FontWeight::new(
            style.font_weight.get(state) as f32,
        )));
        builder.push_default(Prop::StrikethroughBrush(Some(
            style.strikethrough_color.get(state),
        )));
        builder.push_default(Prop::StrikethroughOffset(Some(
            style.strikethrough_offset.get(state),
        )));
        builder.push_default(Prop::UnderlineBrush(Some(style.underline_color.get(state))));
        builder.push_default(Prop::UnderlineOffset(Some(
            style.underline_offset.get(state),
        )));

        let font = style.font.get(state);

        let features = font
            .features
            .iter()
            .map(|f| (*f).into())
            .collect::<SmallVec<[_; 16]>>();

        builder.push_default(Prop::FontFeatures(parley::FontSettings::List(
            features.as_slice().into(),
        )));

        match &font.family {
            FontStack::Source(cow) => {
                builder.push_default(Prop::FontStack(parley::FontStack::Source(cow.clone())));
            }
            FontStack::Single(font_family) => {
                builder.push_default(Prop::FontStack(parley::FontStack::Single(
                    font_family.clone().into(),
                )));
            }
            FontStack::List(cow) => {
                let families = cow
                    .iter()
                    .cloned()
                    .map(|f| f.into())
                    .collect::<SmallVec<[parley::FontFamily; 4]>>();
                builder.push_default(Prop::FontStack(parley::FontStack::List(Cow::Borrowed(
                    &families,
                ))));
            }
        }
    }

    pub(crate) fn apply_plain_editor_styles(
        &self,
        style_id: StyleId,
        state: StateFlags,
        editor: &mut parley::PlainEditor<Color>,
    ) {
        use parley::StyleProperty as Prop;

        let style = self.styles.get(style_id).unwrap();
        let styles = editor.edit_styles();

        styles.insert(Prop::Brush(style.text_color.get(state)));
        styles.insert(Prop::FontSize(style.font_size.get(state) as f32));
        styles.insert(Prop::FontStyle(style.font_style.get(state).into()));
        styles.insert(Prop::FontWeight(parley::FontWeight::new(
            style.font_weight.get(state) as f32,
        )));
        styles.insert(Prop::StrikethroughBrush(Some(
            style.strikethrough_color.get(state),
        )));
        styles.insert(Prop::StrikethroughOffset(Some(
            style.strikethrough_offset.get(state),
        )));
        styles.insert(Prop::UnderlineBrush(Some(style.underline_color.get(state))));
        styles.insert(Prop::UnderlineOffset(Some(
            style.underline_offset.get(state),
        )));

        let font = style.font.get(state);

        let features: Vec<_> = font.features.iter().map(|f| (*f).into()).collect();

        styles.insert(Prop::FontFeatures(parley::FontSettings::List(
            features.into(),
        )));

        match &font.family {
            FontStack::Source(cow) => {
                styles.insert(Prop::FontStack(parley::FontStack::Source(cow.clone())));
            }
            FontStack::Single(font_family) => {
                styles.insert(Prop::FontStack(parley::FontStack::Single(
                    font_family.clone().into(),
                )));
            }
            FontStack::List(cow) => {
                let families: Vec<parley::FontFamily> =
                    cow.iter().cloned().map(|f| f.into()).collect();
                styles.insert(Prop::FontStack(parley::FontStack::List(families.into())));
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}
