use slotmap::SecondaryMap;
use slotmap::SlotMap;
use slotmap::new_key_type;
use smallvec::SmallVec;

use super::StateFlags;
use super::Style;
use super::StyleProperty;
use super::properties::STATE_COUNT;

new_key_type! {
    pub struct StyleId;
}

const MAX_STYLE_TREE_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleError {
    ParentNotFound,
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
            return Err(StyleError::ParentNotFound);
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
        if let Some(def) = self.definitions.get_mut(style_id) {
            def.overrides = properties.into_iter().collect();
        }
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
        self.resolved
            .get(style_id)
            .map(|style| K::get(style, state))
            .unwrap_or_else(|| K::get(&Style::default(), state))
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
        // Regenerate this style
        if let Some(def) = self.definitions.get(style_id).cloned() {
            let resolved = self.build_resolved(&def);
            if let Some(slot) = self.resolved.get_mut(style_id) {
                *slot = resolved;
            }
        }

        // Recursively regenerate children
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
/// Uses a lookup cache for O(1) resolution with compound states.
#[derive(Clone, Debug)]
pub(crate) struct StatefulProperty<T: Copy> {
    default: T,
    overrides: SmallVec<[(StateFlags, T); 4]>,
    /// Cache mapping state bits → index into overrides (0xFF = use default)
    cache: [u8; STATE_COUNT],
}

impl<T: Copy> StatefulProperty<T> {
    pub(crate) fn new(default: T) -> Self {
        Self {
            default,
            overrides: SmallVec::new(),
            cache: [0xFF; STATE_COUNT],
        }
    }

    /// Set a value for the given state flags.
    pub(crate) fn set(&mut self, flags: StateFlags, value: T) {
        // Check if we already have an override for these exact flags
        if let Some(existing) = self.overrides.iter_mut().find(|(f, _)| *f == flags) {
            existing.1 = value;
        } else {
            self.overrides.push((flags, value));
        }
        self.rebuild_cache();
    }

    /// Rebuild the lookup cache after modifications.
    fn rebuild_cache(&mut self) {
        for state_bits in 0..STATE_COUNT {
            let state = StateFlags::from_bits_truncate(state_bits as u8);
            self.cache[state_bits] = self.find_best_match(state);
        }
    }

    /// Find the best matching override for a given state.
    /// Returns index into overrides, or 0xFF if no match (use default).
    fn find_best_match(&self, state: StateFlags) -> u8 {
        let mut best_idx: u8 = 0xFF;
        let mut best_count: i32 = -1; // Start at -1 so NORMAL (0 bits) can win

        for (i, (flags, _)) in self.overrides.iter().enumerate() {
            // A match occurs when the queried state contains all the flags of this override
            if state.contains(*flags) {
                let count = flags.bits().count_ones() as i32;
                if count > best_count {
                    best_idx = i as u8;
                    best_count = count;
                }
            }
        }
        best_idx
    }

    /// Get the value for a given state. O(1) lookup.
    #[inline]
    pub(crate) fn get(&self, state: StateFlags) -> T {
        match self.cache[state.bits() as usize] {
            0xFF => self.default,
            idx => self.overrides[idx as usize].1,
        }
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
