use std::{fmt::Debug, hash::Hash, marker::PhantomData};

use crate::assets::Asset;

pub struct Handle<A: Asset> {
    pub(in crate::assets) index: u32,
    pub(in crate::assets) generation: u32,
    _phantom: PhantomData<A>,
}

impl<A: Asset> Handle<A> {
    pub(in crate::assets) fn new(index: u32, generation: u32) -> Self {
        Self {
            _phantom: PhantomData,
            index,
            generation,
        }
    }
}

impl<A: Asset> Clone for Handle<A> {
    fn clone(&self) -> Self {
        Self { index: self.index, generation: self.generation, _phantom: self._phantom }
    }
}

impl<A: Asset> Copy for Handle<A> {

}

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<A: Asset> Hash for Handle<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<A: Asset> PartialEq for Handle<A> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<A: Asset> Eq for Handle<A> {
}