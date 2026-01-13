//! State-dependent property values.
//!
//! [`StatefulProperty`] stores a default value along with optional overrides
//! for specific [`StateFlags`] combinations. Overrides are sorted by
//! specificity (most specific first) for efficient lookup.

use smallvec::SmallVec;

use super::StateFlags;

/// A property value that varies based on widget state.
///
/// Overrides are sorted by specificity (descending bit count) so that
/// more specific state combinations are checked first during lookup.
///
/// # Example
///
/// ```ignore
/// let mut prop = StatefulProperty::new(Color::WHITE);
/// prop.set(StateFlags::HOVERED, Color::LIGHT_GRAY);
/// prop.set(StateFlags::HOVERED | StateFlags::PRESSED, Color::DARK_GRAY);
///
/// // Most specific match wins
/// assert_eq!(prop.get(StateFlags::HOVERED | StateFlags::PRESSED), Color::DARK_GRAY);
/// assert_eq!(prop.get(StateFlags::HOVERED), Color::LIGHT_GRAY);
/// assert_eq!(prop.get(StateFlags::NORMAL), Color::WHITE);
/// ```
#[derive(Clone, Debug)]
pub struct StatefulProperty<T: Clone> {
    default: T,
    /// Overrides sorted by specificity (most specific first) for O(1) best-case lookup.
    overrides: SmallVec<[(StateFlags, T); 4]>,
}

impl<T: Clone> StatefulProperty<T> {
    /// Create a new stateful property with the given default value.
    pub fn new(default: T) -> Self {
        Self {
            default,
            overrides: SmallVec::new(),
        }
    }

    /// Set a value for the given state flags.
    ///
    /// If an override for these exact flags already exists, it is replaced.
    /// Otherwise, a new override is inserted in specificity order (most
    /// specific first, based on bit count).
    pub fn set(&mut self, flags: StateFlags, value: T) {
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
    ///
    /// Scans overrides in specificity order and returns the first match
    /// (where the override's flags are a subset of the query state).
    /// Falls back to the default if no override matches.
    #[inline]
    pub fn get(&self, state: StateFlags) -> T {
        for (flags, value) in &self.overrides {
            if state.contains(*flags) {
                return value.clone();
            }
        }
        self.default.clone()
    }
}

impl<T: Copy + Default> Default for StatefulProperty<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_value_returned_for_any_state() {
        let prop: StatefulProperty<i32> = StatefulProperty::new(42);
        assert_eq!(prop.get(StateFlags::NORMAL), 42);
        assert_eq!(prop.get(StateFlags::HOVERED), 42);
        assert_eq!(prop.get(StateFlags::PRESSED), 42);
    }

    #[test]
    fn override_matches_exact_state() {
        let mut prop = StatefulProperty::new(0);
        prop.set(StateFlags::HOVERED, 100);

        assert_eq!(prop.get(StateFlags::NORMAL), 0);
        assert_eq!(prop.get(StateFlags::HOVERED), 100);
    }

    #[test]
    fn override_matches_superset_state() {
        let mut prop = StatefulProperty::new(0);
        prop.set(StateFlags::HOVERED, 100);

        // HOVERED | PRESSED is a superset of HOVERED, so it matches
        assert_eq!(prop.get(StateFlags::HOVERED | StateFlags::PRESSED), 100);
    }

    #[test]
    fn set_overwrites_existing_flags() {
        let mut prop = StatefulProperty::new(0);
        prop.set(StateFlags::HOVERED, 100);
        prop.set(StateFlags::HOVERED, 200);

        assert_eq!(prop.get(StateFlags::HOVERED), 200);
    }

    #[test]
    fn specificity_ordering_most_specific_wins() {
        let mut prop = StatefulProperty::new(0);
        // Insert in arbitrary order
        prop.set(StateFlags::HOVERED, 1);
        prop.set(StateFlags::HOVERED | StateFlags::PRESSED, 2);
        prop.set(StateFlags::NORMAL, 0);

        // Most specific (2 bits) should win over less specific (1 bit)
        assert_eq!(prop.get(StateFlags::HOVERED | StateFlags::PRESSED), 2);
        assert_eq!(prop.get(StateFlags::HOVERED), 1);
        assert_eq!(prop.get(StateFlags::NORMAL), 0);
    }

    #[test]
    fn normal_state_is_subset_of_all() {
        let mut prop = StatefulProperty::new(0);
        prop.set(StateFlags::NORMAL, 42);

        // NORMAL (empty flags) is a subset of everything
        assert_eq!(prop.get(StateFlags::HOVERED), 42);
        assert_eq!(prop.get(StateFlags::PRESSED), 42);
        assert_eq!(prop.get(StateFlags::HOVERED | StateFlags::PRESSED), 42);
    }

    #[test]
    fn multiple_overrides_correct_precedence() {
        let mut prop = StatefulProperty::new(0);
        prop.set(StateFlags::NORMAL, 1);
        prop.set(StateFlags::HOVERED, 2);
        prop.set(StateFlags::PRESSED, 3);
        prop.set(StateFlags::HOVERED | StateFlags::PRESSED, 4);

        assert_eq!(prop.get(StateFlags::NORMAL), 1);
        assert_eq!(prop.get(StateFlags::HOVERED), 2);
        assert_eq!(prop.get(StateFlags::PRESSED), 3);
        assert_eq!(prop.get(StateFlags::HOVERED | StateFlags::PRESSED), 4);
        // With an extra flag, still matches the 2-bit override
        assert_eq!(
            prop.get(StateFlags::HOVERED | StateFlags::PRESSED | StateFlags::FOCUSED),
            4
        );
    }
}
