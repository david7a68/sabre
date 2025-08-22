use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use std::num::NonZeroU64;

use rapidhash::fast::RapidHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(NonZeroU64);

impl WidgetId {
    const SEED: u64 = 1234;

    pub fn new(value: impl Hash) -> Self {
        let mut hasher = RapidHasher::new(Self::SEED);

        value.hash(&mut hasher);

        let value = hasher.finish();
        WidgetId(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
    }

    pub fn then(self, other: impl Hash) -> Self {
        let mut hasher = RapidHasher::new(Self::SEED);

        self.hash(&mut hasher);
        other.hash(&mut hasher);

        let value = hasher.finish();
        WidgetId(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
    }
}

pub(crate) type IdMap<V> = HashMap<WidgetId, V, IdHasherBuilder>;

pub(crate) struct IdHasher {
    value: u64,
}

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.value
    }

    fn write(&mut self, _: &[u8]) {
        unimplemented!()
    }

    fn write_u64(&mut self, i: u64) {
        self.value = i;
    }
}

#[derive(Default)]
pub(crate) struct IdHasherBuilder;

impl std::hash::BuildHasher for IdHasherBuilder {
    type Hasher = IdHasher;

    fn build_hasher(&self) -> Self::Hasher {
        IdHasher { value: 0 }
    }
}
