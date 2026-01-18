use slotmap::SecondaryMap;
use slotmap::SlotMap;
use slotmap::new_key_type;
use smallvec::SmallVec;

use super::StateFlags;
use super::Style;
use super::StyleProperty;

new_key_type! {
    pub struct StyleId;
}

const MAX_STYLE_TREE_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleError {
    StyleTreeDepthLimitExceeded,
}

pub struct StyleRegistry {
    default_style: StyleId,
    /// Source definitions - kept for regeneration when parent changes
    definitions: SlotMap<StyleId, StyleDef>,
    /// Resolved styles - used at runtime for O(1) property access
    resolved: SecondaryMap<StyleId, Style>,
    /// Child relationships for propagating parent changes
    children: SecondaryMap<StyleId, SmallVec<[StyleId; 4]>>,
}

impl Default for StyleRegistry {
    fn default() -> Self {
        let mut definitions = SlotMap::with_key();
        let mut resolved = SecondaryMap::new();
        let mut children = SecondaryMap::new();

        let default_style = definitions.insert(StyleDef::default());
        resolved.insert(default_style, Style::default());
        children.insert(default_style, SmallVec::new());

        Self {
            default_style,
            definitions,
            resolved,
            children,
        }
    }
}

impl StyleRegistry {
    pub fn default_style_id(&self) -> StyleId {
        self.default_style
    }

    /// Register a new style with optional parent and property overrides.
    /// Returns a StyleId that can be used to access the resolved style.
    pub fn register(
        &mut self,
        parent: Option<StyleId>,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) -> Result<StyleId, StyleError> {
        if let Some(parent) = parent
            && self.definitions.get(parent).is_none()
        {
            panic!("Attempted to register style with parent that does not exist");
        }

        // Check tree depth by walking up the parent chain
        if let Some(mut current) = parent {
            let mut depth = 1;
            while let Some(def) = self.definitions.get(current) {
                if let Some(p) = def.parent {
                    depth += 1;
                    if depth >= MAX_STYLE_TREE_DEPTH {
                        return Err(StyleError::StyleTreeDepthLimitExceeded);
                    }
                    current = p;
                } else {
                    break;
                }
            }
        }

        let def = StyleDef::new(parent, properties);

        // Build resolved style from parent + overrides
        let resolved = self.build_resolved(&def);

        // Insert definition and resolved style
        let id = self.definitions.insert(def);
        self.resolved.insert(id, resolved);
        self.children.insert(id, SmallVec::new());

        // Register as child of parent for regeneration
        if let Some(parent_id) = parent
            && let Some(siblings) = self.children.get_mut(parent_id)
        {
            siblings.push(id);
        }

        Ok(id)
    }

    /// Update a style's overrides and regenerate it and all descendants.
    pub fn update(
        &mut self,
        style_id: StyleId,
        properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) {
        let Some(def) = self.definitions.get_mut(style_id) else {
            panic!("Attempted to update style that does not exist");
        };

        def.overrides = properties.into_iter().collect();
        self.regenerate(style_id);
    }

    /// Get the resolved style for a StyleId.
    #[inline]
    pub fn get(&self, style_id: StyleId) -> Option<&Style> {
        self.resolved.get(style_id)
    }

    /// Type-safe property resolution with default fallback.
    #[inline]
    pub fn resolve<K: PropertyKey>(&self, style_id: StyleId, state: StateFlags) -> K::Value {
        let style = self.resolved.get(style_id).unwrap();
        K::get(style, state)
    }

    /// Build a resolved Style from a StyleDef.
    fn build_resolved(&self, def: &StyleDef) -> Style {
        // Start from parent's resolved style or default
        let mut style = def
            .parent
            .and_then(|p| self.resolved.get(p))
            .cloned()
            .unwrap_or_default();

        // Apply overrides
        style.apply_all(def.overrides.iter().cloned());

        style
    }

    /// Regenerate a style and all its descendants.
    fn regenerate(&mut self, style_id: StyleId) {
        if let Some(def) = self.definitions.get(style_id) {
            let resolved = self.build_resolved(def);
            if let Some(slot) = self.resolved.get_mut(style_id) {
                *slot = resolved;
            }
        }

        if let Some(child_ids) = self.children.get(style_id).cloned() {
            for child_id in child_ids {
                self.regenerate(child_id);
            }
        }
    }
}

/// Trait for type-safe property access. Implemented by zero-sized type keys.
pub trait PropertyKey: crate::sealed::Sealed {
    /// The value type of this property.
    type Value;
    /// Get this property's value from a style for the given state.
    fn get(style: &Style, state: StateFlags) -> Self::Value;
}

/// Source definition for a style - stored for regeneration.
#[derive(Clone, Debug, Default)]
struct StyleDef {
    parent: Option<StyleId>,
    overrides: SmallVec<[(StateFlags, StyleProperty); 16]>,
}

impl StyleDef {
    fn new(
        parent: Option<StyleId>,
        overrides: impl IntoIterator<Item = (StateFlags, StyleProperty)>,
    ) -> Self {
        Self {
            parent,
            overrides: overrides.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::graphics::Color;
    use crate::graphics::Paint;
    use crate::ui::style::{Background, TextColor};

    use super::*;

    // Helper to create colors from 0-255 RGB values
    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    // ==================== Registration Tests ====================

    #[test]
    fn basic_registration_and_resolution() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(255, 0, 0))),
                    ),
                    (StateFlags::NORMAL, StyleProperty::TextColor(rgb(0, 0, 255))),
                ],
            )
            .unwrap();

        let bg: Paint = registry.resolve::<Background>(style, StateFlags::NORMAL);
        let text: Color = registry.resolve::<TextColor>(style, StateFlags::NORMAL);

        assert_eq!(bg, Paint::solid(rgb(255, 0, 0)));
        assert_eq!(text, rgb(0, 0, 255));
    }

    #[test]
    #[should_panic(expected = "parent that does not exist")]
    fn invalid_parent_panics() {
        let mut registry = StyleRegistry::default();

        let mut other_registry = StyleRegistry::default();
        let fake_parent = other_registry.register(None, vec![]).unwrap();

        let _ = registry.register(Some(fake_parent), vec![]);
    }

    #[test]
    fn depth_limit_enforcement() {
        let mut registry = StyleRegistry::default();

        let mut current = registry.register(None, vec![]).unwrap();

        // MAX_STYLE_TREE_DEPTH is 32; we can have 31 levels of children
        for _ in 0..31 {
            current = registry.register(Some(current), vec![]).unwrap();
        }

        let result = registry.register(Some(current), vec![]);
        assert_eq!(result, Err(StyleError::StyleTreeDepthLimitExceeded));
    }

    #[test]
    fn default_style_is_accessible() {
        let registry = StyleRegistry::default();

        let default_id = registry.default_style_id();

        let bg: Paint = registry.resolve::<Background>(default_id, StateFlags::NORMAL);
        let text: Color = registry.resolve::<TextColor>(default_id, StateFlags::NORMAL);

        assert_eq!(bg, Paint::solid(Color::WHITE));
        assert_eq!(text, Color::BLACK);
        assert!(registry.get(default_id).is_some());
    }

    #[test]
    fn child_can_use_default_as_parent() {
        let mut registry = StyleRegistry::default();

        let default_id = registry.default_style_id();

        let child = registry
            .register(
                Some(default_id),
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                )],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(child, StateFlags::NORMAL),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<TextColor>(child, StateFlags::NORMAL),
            Color::BLACK
        );
    }

    // ==================== Inheritance Tests ====================

    #[test]
    fn child_inherits_from_parent() {
        let mut registry = StyleRegistry::default();

        let parent = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                    ),
                    (
                        StateFlags::NORMAL,
                        StyleProperty::TextColor(rgb(255, 255, 255)),
                    ),
                ],
            )
            .unwrap();

        let child = registry
            .register(
                Some(parent),
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                )],
            )
            .unwrap();

        // Background from child, text inherited from parent
        assert_eq!(
            registry.resolve::<Background>(child, StateFlags::NORMAL),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<TextColor>(child, StateFlags::NORMAL),
            rgb(255, 255, 255)
        );
    }

    #[test]
    fn sibling_independence() {
        let mut registry = StyleRegistry::default();

        let parent = registry
            .register(
                None,
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                )],
            )
            .unwrap();

        let sibling1 = registry
            .register(
                Some(parent),
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                )],
            )
            .unwrap();

        let sibling2 = registry
            .register(
                Some(parent),
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
                )],
            )
            .unwrap();

        registry.update(
            sibling1,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
            )],
        );

        assert_eq!(
            registry.resolve::<Background>(sibling1, StateFlags::NORMAL),
            Paint::solid(rgb(150, 150, 150))
        );
        assert_eq!(
            registry.resolve::<Background>(sibling2, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
    }

    // ==================== State Resolution Tests ====================

    #[test]
    fn state_specific_resolution() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
                    ),
                    (
                        StateFlags::PRESSED,
                        StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(150, 150, 150))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::PRESSED),
            Paint::solid(rgb(200, 200, 200))
        );
    }

    #[test]
    fn default_fallback() {
        let mut registry = StyleRegistry::default();
        let style = registry.register(None, vec![]).unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(Color::WHITE)
        );
        assert_eq!(
            registry.resolve::<TextColor>(style, StateFlags::NORMAL),
            Color::BLACK
        );
    }

    #[test]
    fn exact_match_priority() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED | StateFlags::PRESSED,
                        StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED | StateFlags::PRESSED),
            Paint::solid(rgb(200, 200, 200))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(100, 100, 100))
        );
    }

    #[test]
    fn most_specific_subset_match() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED | StateFlags::PRESSED,
                        StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
                    ),
                ],
            )
            .unwrap();

        // 2-bit match beats 1-bit match
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED | StateFlags::PRESSED),
            Paint::solid(rgb(150, 150, 150))
        );
    }

    #[test]
    fn state_fallback_to_normal() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                )],
            )
            .unwrap();

        // NORMAL is empty flags, so it matches any state as a subset
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(50, 50, 50))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::PRESSED),
            Paint::solid(rgb(50, 50, 50))
        );
    }

    #[test]
    fn hovered_does_not_match_normal_query() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![(
                    StateFlags::HOVERED,
                    StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                )],
            )
            .unwrap();

        // HOVERED is not a subset of NORMAL (empty), so falls back to default
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(Color::WHITE)
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(100, 100, 100))
        );
    }

    #[test]
    fn empty_state_returns_default() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::PRESSED,
                        StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::empty()),
            Paint::solid(Color::WHITE)
        );
    }

    #[test]
    fn compound_state_selected_and_hovered() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                    ),
                    (
                        StateFlags::SELECTED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
                    ),
                    (
                        StateFlags::SELECTED | StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(rgb(50, 50, 50))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::SELECTED),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(150, 150, 150))
        );
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::SELECTED | StateFlags::HOVERED),
            Paint::solid(rgb(200, 200, 200))
        );
    }

    #[test]
    fn multiple_properties_same_state() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::TextColor(rgb(255, 255, 255)),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<TextColor>(style, StateFlags::HOVERED),
            rgb(255, 255, 255)
        );
    }

    // ==================== Update & Regeneration Tests ====================

    #[test]
    fn style_update_regenerates() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                )],
            )
            .unwrap();

        registry.update(
            style,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
            )],
        );

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
    }

    #[test]
    fn parent_update_regenerates_children() {
        let mut registry = StyleRegistry::default();

        let parent = registry
            .register(
                None,
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                )],
            )
            .unwrap();

        let child = registry
            .register(
                Some(parent),
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::TextColor(rgb(255, 255, 255)),
                )],
            )
            .unwrap();

        registry.update(
            parent,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
            )],
        );

        assert_eq!(
            registry.resolve::<Background>(child, StateFlags::NORMAL),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            registry.resolve::<TextColor>(child, StateFlags::NORMAL),
            rgb(255, 255, 255)
        );
    }

    #[test]
    fn deep_regeneration() {
        let mut registry = StyleRegistry::default();

        let root = registry
            .register(
                None,
                vec![(
                    StateFlags::NORMAL,
                    StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                )],
            )
            .unwrap();

        let child1 = registry.register(Some(root), vec![]).unwrap();
        let child2 = registry.register(Some(child1), vec![]).unwrap();
        let child3 = registry.register(Some(child2), vec![]).unwrap();

        registry.update(
            root,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
            )],
        );

        assert_eq!(
            registry.resolve::<Background>(child1, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
        assert_eq!(
            registry.resolve::<Background>(child2, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
        assert_eq!(
            registry.resolve::<Background>(child3, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
    }

    #[test]
    fn update_replaces_all_overrides() {
        let mut registry = StyleRegistry::default();

        let style = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(50, 50, 50))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::NORMAL,
                        StyleProperty::TextColor(rgb(255, 255, 255)),
                    ),
                ],
            )
            .unwrap();

        registry.update(
            style,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::Background(Paint::solid(rgb(200, 200, 200))),
            )],
        );

        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::NORMAL),
            Paint::solid(rgb(200, 200, 200))
        );
        // Old hovered override gone
        assert_eq!(
            registry.resolve::<Background>(style, StateFlags::HOVERED),
            Paint::solid(rgb(200, 200, 200))
        );
        // TextColor override removed
        assert_eq!(
            registry.resolve::<TextColor>(style, StateFlags::NORMAL),
            Color::BLACK
        );
    }

    // ==================== Accessor Tests ====================

    #[test]
    fn get_returns_none_for_invalid_id() {
        let registry = StyleRegistry::default();

        let mut other_registry = StyleRegistry::default();
        let fake_id = other_registry.register(None, vec![]).unwrap();

        assert!(registry.get(fake_id).is_none());
    }

    #[test]
    #[should_panic]
    fn resolve_panics_on_invalid_id() {
        let registry = StyleRegistry::default();

        let mut other_registry = StyleRegistry::default();
        let fake_id = other_registry.register(None, vec![]).unwrap();

        let _: Paint = registry.resolve::<Background>(fake_id, StateFlags::NORMAL);
    }

    #[test]
    #[should_panic(expected = "update style that does not exist")]
    fn update_panics_on_invalid_id() {
        let mut registry = StyleRegistry::default();

        let mut other_registry = StyleRegistry::default();
        let fake_id = other_registry.register(None, vec![]).unwrap();

        registry.update(fake_id, vec![]);
    }

    #[test]
    fn direct_style_access() {
        let mut registry = StyleRegistry::default();

        let style_id = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(rgb(100, 100, 100))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(rgb(150, 150, 150))),
                    ),
                ],
            )
            .unwrap();

        let style = registry.get(style_id).unwrap();

        assert_eq!(
            style.background.get(StateFlags::NORMAL),
            Paint::solid(rgb(100, 100, 100))
        );
        assert_eq!(
            style.background.get(StateFlags::HOVERED),
            Paint::solid(rgb(150, 150, 150))
        );
    }
}
