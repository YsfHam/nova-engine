use std::{fmt::Debug, hash::Hash, marker::PhantomData};

use crate::assets::Asset;

pub struct Handle<A: Asset> {
    pub(in crate::assets) id: u64,
    pub(in crate::assets) index: usize,
    _phantom: PhantomData<A>,
}

impl<A: Asset> Handle<A> {
    pub(in crate::assets) fn new(id: u64, index: usize) -> Self {
        Self {
            id,
            index,
            _phantom: PhantomData
        }
    }
}

impl<A: Asset> Clone for Handle<A> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), _phantom: self._phantom.clone(), index: self.index.clone() }
    }
}

impl<A: Asset> Copy for Handle<A> {

}

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").field("id", &self.id).finish()
    }
}

impl<A: Asset> Hash for Handle<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<A: Asset> PartialEq for Handle<A> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<A: Asset> Eq for Handle<A> {
}