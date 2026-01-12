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
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Try to get a property value, returning None if style doesn't exist.
    #[inline]
    pub fn try_resolve<K: PropertyKey>(
        &self,
        style_id: StyleId,
        state: StateFlags,
    ) -> Option<K::Value> {
        self.resolved
            .get(style_id)
            .map(|style| K::get(style, state))
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

/// A property value that varies based on widget state.
/// Overrides are sorted by specificity (descending) for early-exit lookup.
#[derive(Clone, Debug)]
pub(crate) struct StatefulProperty<T: Copy> {
    default: T,
    /// Overrides sorted by specificity (most specific first) for O(1) best-case lookup.
    overrides: SmallVec<[(StateFlags, T); 4]>,
}

impl<T: Copy> StatefulProperty<T> {
    pub(crate) fn new(default: T) -> Self {
        Self {
            default,
            overrides: SmallVec::new(),
        }
    }

    /// Set a value for the given state flags.
    /// Maintains sort order by specificity (descending).
    pub(crate) fn set(&mut self, flags: StateFlags, value: T) {
        // Check if we already have an override for these exact flags
        if let Some(existing) = self.overrides.iter_mut().find(|(f, _)| *f == flags) {
            existing.1 = value;
            return;
        }

        // Insert in sorted order (most specific first)
        let specificity = flags.bits().count_ones();
        let insert_pos = self
            .overrides
            .iter()
            .position(|(f, _)| f.bits().count_ones() < specificity)
            .unwrap_or(self.overrides.len());

        self.overrides.insert(insert_pos, (flags, value));
    }

    /// Get the value for a given state.
    /// Scans overrides (sorted by specificity) and returns first match.
    #[inline]
    pub(crate) fn get(&self, state: StateFlags) -> T {
        for (flags, value) in &self.overrides {
            if state.contains(*flags) {
                return *value;
            }
        }
        self.default
    }
}

impl<T: Copy + Default> Default for StatefulProperty<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
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
