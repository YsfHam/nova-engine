use std::{any::TypeId, fmt::Debug, hash::Hash, marker::PhantomData, sync::Arc};

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

/// A type-erased, owning strong handle — the strong counterpart to
/// [`GenericHandle`]. Like [`StrongHandle`], it is `Clone` but not `Copy`,
/// and keeps the asset alive via a shared `Arc<()>` ref-count marker.
///
/// Used by [`DefaultAssets`](crate::assets::defaults::DefaultAssets) and
/// other type-erased collections that need to hold strong references without
/// knowing the concrete asset type at the storage site.
pub struct StrongGenericHandle {
    pub index: u32,
    pub generation: u32,
    pub type_id: TypeId,
    _ref: Arc<()>,
}

impl StrongGenericHandle {
    /// Converts this strong generic handle into a typed [`StrongHandle`].
    /// Returns `Err(())` if the `TypeId` does not match `A`.
    pub fn try_into_strong_handle<A: Asset>(self) -> Result<StrongHandle<A>, ()> {
        if TypeId::of::<A>() != self.type_id {
            Err(())
        } else {
            Ok(StrongHandle::new(self.index, self.generation, self._ref))
        }
    }

    /// Creates a weak [`GenericHandle`] (no ref count) from this strong handle.
    pub fn as_generic_handle(&self) -> GenericHandle {
        GenericHandle {
            index: self.index,
            generation: self.generation,
            type_id: self.type_id,
        }
    }
}

impl<A: Asset> From<StrongHandle<A>> for StrongGenericHandle {
    fn from(value: StrongHandle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
            _ref: value._ref,
        }
    }
}

impl Debug for StrongGenericHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrongGenericHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("type_id", &self.type_id)
            .field("ref_count", &Arc::strong_count(&self._ref))
            .finish()
    }
}

impl Clone for StrongGenericHandle {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            generation: self.generation,
            type_id: self.type_id,
            _ref: Arc::clone(&self._ref),
        }
    }
}

impl Hash for StrongGenericHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}

impl PartialEq for StrongGenericHandle {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.generation == other.generation
            && self.type_id == other.type_id
    }
}

impl Eq for StrongGenericHandle {}

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

/// A strong, owning handle to an asset. Cloning increments the ref count;
/// dropping decrements it. When the last `StrongHandle` is dropped (only the
/// slot's `Arc` remains), the asset becomes eligible for automatic off-loading.
///
/// `StrongHandle` is **not** `Copy` — it must be explicitly cloned or converted
/// to a weak [`Handle`] via [`as_handle`](Self::as_handle).
///
/// Use `StrongHandle` where you want to keep an asset alive (e.g. scene graphs,
/// `DefaultAssets`). Use `Handle` where you only reference an asset without
/// owning it (e.g. `DrawBatch.material`, `SpriteMaterial.texture`).
pub struct StrongHandle<A: Asset> {
    pub(in crate::assets) index: u32,
    pub(in crate::assets) generation: u32,
    /// Shared ref-count marker. The slot holds a clone of this `Arc`;
    /// when `Arc::strong_count` drops to 1 (only the slot remains), the
    /// asset is eligible for off-loading.
    _ref: Arc<()>,
    _phantom: PhantomData<A>,
}

impl<A: Asset> StrongHandle<A> {
    pub(in crate::assets) fn new(index: u32, generation: u32, _ref: Arc<()>) -> Self {
        Self {
            index,
            generation,
            _ref,
            _phantom: PhantomData,
        }
    }

    /// Creates a weak [`Handle`] from this strong handle.
    /// The weak handle does not keep the asset alive.
    pub fn as_handle(&self) -> Handle<A> {
        Handle::new(self.index, self.generation)
    }
}

impl<A: Asset> Clone for StrongHandle<A> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            generation: self.generation,
            _ref: Arc::clone(&self._ref),
            _phantom: PhantomData,
        }
    }
}

impl<A: Asset> From<&StrongHandle<A>> for Handle<A> {
    fn from(value: &StrongHandle<A>) -> Self {
        value.as_handle()
    }
}

impl<A: Asset> From<StrongHandle<A>> for GenericHandle {
    fn from(value: StrongHandle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
        }
    }
}

impl<A: Asset> Debug for StrongHandle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrongHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("ref_count", &Arc::strong_count(&self._ref))
            .finish()
    }
}

impl<A: Asset> Hash for StrongHandle<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<A: Asset> PartialEq for StrongHandle<A> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<A: Asset> Eq for StrongHandle<A> {}

impl<A: Asset> PartialOrd for StrongHandle<A> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for StrongHandle<A> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index
            .cmp(&other.index)
            .then(self.generation.cmp(&other.generation))
    }
}