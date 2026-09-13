use std::{any::TypeId, fmt::Debug, hash::Hash, marker::PhantomData};

use crate::assets::Asset;

#[derive(Copy, Clone)]
pub struct GenericHandle {
    pub index: u32,
    pub generation: u32,
    pub type_id: TypeId,
}

impl GenericHandle {
    pub fn try_into_handle<A: Asset>(self) -> Result<Handle<A>, ()> {
        if TypeId::of::<A>() != self.type_id {
            Err(())
        } else {
            Ok(Handle::new(self.index, self.generation))
        }
    }
}

impl Debug for GenericHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("type_id", &self.type_id)
            .finish()
    }
}

impl Hash for GenericHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}

impl PartialEq for GenericHandle {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.generation == other.generation
            && self.type_id == other.type_id
    }
}

impl Eq for GenericHandle {}

pub struct Handle<A: Asset> {
    pub(in crate::assets) index: u32,
    pub(in crate::assets) generation: u32,
    _phantom: PhantomData<A>,
}

impl<A: Asset> From<Handle<A>> for GenericHandle {
    fn from(value: Handle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
        }
    }
}

impl<A: Asset> Handle<A> {
    pub(in crate::assets) fn new(index: u32, generation: u32) -> Self {
        Self {
            _phantom: PhantomData,
            index,
            generation,
        }
    }

    pub fn into_generic(self) -> GenericHandle {
        self.into()
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

impl<A: Asset> PartialOrd for Handle<A> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for Handle<A> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index
            .cmp(&other.index)
            .then(self.generation.cmp(&other.generation))
    }
}