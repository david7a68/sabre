use std::borrow::Cow;
use std::sync::OnceLock;

use smallvec::SmallVec;

use crate::graphics::Color;
use crate::graphics::FontStack;

use super::style::BorderWidths;
use super::style::PropertyKey;
use super::style::StateFlags;
use super::style::Style;
use super::style::StyleError;
use super::style::StyleId;
use super::style::StyleProperty;
use super::style::StyleRegistry;

static DEFAULT_FONT_FEATURES: OnceLock<parley::FontSettings<'static, parley::FontFeature>> =
    OnceLock::new();

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleClass {
    Panel = 0,
    Button,
    Label,
    TextEdit,
}

impl StyleClass {
    /// Number of style class variants. Update when adding new variants.
    pub const COUNT: usize = 4;
}

pub struct Theme {
    well_known_classes: [Option<StyleId>; StyleClass::COUNT],
    styles: StyleRegistry,
}

impl Theme {
    pub fn new() -> Self {
        let styles = StyleRegistry::default();

        Self {
            styles,
            well_known_classes: [None; StyleClass::COUNT],
        }
    }

    /// Gets the style assigned to a style class.
    pub fn get(&self, class: StyleClass) -> &Style {
        let styled_id = self.get_id(class);
        self.styles.get(styled_id).unwrap()
    }

    /// Gets the style ID assigned to a style class.
    pub fn get_id(&self, class: StyleClass) -> StyleId {
        self.well_known_classes[class as usize].unwrap_or(self.styles.default_style_id())
    }

    /// Assigns a style to a style class.
    pub fn set(&mut self, class: StyleClass, style_id: StyleId) {
        self.well_known_classes[class as usize] = Some(style_id);
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

    /// Modifies a style class by replacing its properties, registering a new
    /// style if the class doesn't already have one assigned.
    ///
    pub fn set_style_class(
        &mut self,
        class: StyleClass,
        parent: Option<StyleId>,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) -> Result<StyleId, StyleError> {
        let style = if let Some(current) = self.well_known_classes[class as usize] {
            self.update_style(current, properties);
            current
        } else {
            self.create_style(parent, properties)?
        };

        self.set(class, style);
        Ok(style)
    }

    /// Resolves a property for a specific style class and state combination.
    pub fn resolve<K: PropertyKey>(&self, style: StyleClass, state: StateFlags) -> K::Value {
        let style_id =
            self.well_known_classes[style as usize].unwrap_or(self.styles.default_style_id());
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

        let style = self.enumerate_styles(style_id, state, |prop| {
            builder.push_default(prop);
        });

        match &style.font.get(state).family {
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
    ) -> &Style {
        use parley::StyleProperty as Prop;

        let styles = editor.edit_styles();

        let style = self.enumerate_styles(style_id, state, |prop| {
            styles.insert(prop);
        });

        match &style.font.get(state).family {
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

        style
    }

    fn enumerate_styles<'a>(
        &self,
        style_id: StyleId,
        state: StateFlags,
        mut callback: impl FnMut(parley::StyleProperty<'a, Color>),
    ) -> &Style {
        use parley::StyleProperty as Prop;

        let style = self.styles.get(style_id).unwrap();

        callback(Prop::FontFeatures(default_font_features()));
        callback(Prop::Brush(style.text_color.get(state)));
        callback(Prop::FontSize(style.font_size.get(state) as f32));
        callback(Prop::FontStyle(style.font_style.get(state).into()));
        callback(Prop::FontWeight(parley::FontWeight::new(
            style.font_weight.get(state) as f32,
        )));
        callback(Prop::StrikethroughBrush(Some(
            style.strikethrough_color.get(state),
        )));
        callback(Prop::StrikethroughOffset(Some(
            style.strikethrough_offset.get(state),
        )));
        callback(Prop::UnderlineBrush(Some(style.underline_color.get(state))));
        callback(Prop::UnderlineOffset(Some(
            style.underline_offset.get(state),
        )));

        style
    }
}

impl Default for Theme {
    fn default() -> Self {
        default_theme()
    }
}

fn default_font_features() -> parley::FontSettings<'static, parley::FontFeature> {
    DEFAULT_FONT_FEATURES
        .get_or_init(|| {
            let list = Vec::leak(
                parley::FontFeature::parse_list("kern, rlig, dlig, liga, clig, calt").collect(),
            );

            parley::FontSettings::List(Cow::Borrowed(list))
        })
        .clone()
}

fn default_theme() -> Theme {
    let mut theme = Theme::new();

    theme
        .set_style_class(
            StyleClass::Label,
            None,
            [(
                StateFlags::empty(),
                StyleProperty::BorderWidths(BorderWidths {
                    left: 0.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                }),
            )],
        )
        .unwrap();

    theme
}
