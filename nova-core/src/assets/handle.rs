use std::{any::TypeId, fmt::Debug, hash::Hash, marker::PhantomData, sync::Arc};

use crate::assets::Asset;

#[derive(Copy, Clone)]
pub struct WeakGenericHandle {
    pub index: u32,
    pub generation: u32,
    pub type_id: TypeId,
}

impl WeakGenericHandle {
    pub fn try_into_handle<A: Asset>(self) -> Result<WeakHandle<A>, ()> {
        if TypeId::of::<A>() != self.type_id {
            Err(())
        } else {
            Ok(WeakHandle::new(self.index, self.generation))
        }
    }
}

impl Debug for WeakGenericHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("type_id", &self.type_id)
            .finish()
    }
}

impl Hash for WeakGenericHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}

impl PartialEq for WeakGenericHandle {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.generation == other.generation
            && self.type_id == other.type_id
    }
}

impl Eq for WeakGenericHandle {}

/// A type-erased, owning strong handle — the strong counterpart to
/// [`GenericHandle`]. Like [`StrongHandle`], it is `Clone` but not `Copy`,
/// and keeps the asset alive via a shared `Arc<()>` ref-count marker.
///
/// Used by [`DefaultAssets`](crate::assets::defaults::DefaultAssets) and
/// other type-erased collections that need to hold strong references without
/// knowing the concrete asset type at the storage site.
pub struct GenericHandle {
    pub index: u32,
    pub generation: u32,
    pub type_id: TypeId,
    _ref: Arc<()>,
}

impl GenericHandle {
    /// Converts this strong generic handle into a typed [`StrongHandle`].
    /// Returns `Err(())` if the `TypeId` does not match `A`.
    pub fn try_into_handle<A: Asset>(self) -> Result<Handle<A>, ()> {
        if TypeId::of::<A>() != self.type_id {
            Err(())
        } else {
            Ok(Handle::new(self.index, self.generation, self._ref))
        }
    }

    /// Creates a weak [`GenericHandle`] (no ref count) from this strong handle.
    pub fn weak(&self) -> WeakGenericHandle {
        WeakGenericHandle {
            index: self.index,
            generation: self.generation,
            type_id: self.type_id,
        }
    }
}

impl<A: Asset> From<Handle<A>> for GenericHandle {
    fn from(value: Handle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
            _ref: value._ref,
        }
    }
}

impl Debug for GenericHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrongGenericHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("type_id", &self.type_id)
            .field("ref_count", &Arc::strong_count(&self._ref))
            .finish()
    }
}

impl Clone for GenericHandle {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            generation: self.generation,
            type_id: self.type_id,
            _ref: Arc::clone(&self._ref),
        }
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

pub struct WeakHandle<A: Asset> {
    pub(in crate::assets) index: u32,
    pub(in crate::assets) generation: u32,
    _phantom: PhantomData<A>,
}

impl<A: Asset> From<WeakHandle<A>> for WeakGenericHandle {
    fn from(value: WeakHandle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
        }
    }
}

impl<A: Asset> WeakHandle<A> {
    pub(in crate::assets) fn new(index: u32, generation: u32) -> Self {
        Self {
            _phantom: PhantomData,
            index,
            generation,
        }
    }

    pub fn into_generic(self) -> WeakGenericHandle {
        self.into()
    }
}

impl<A: Asset> Clone for WeakHandle<A> {
    fn clone(&self) -> Self {
        Self { index: self.index, generation: self.generation, _phantom: self._phantom }
    }
}

impl<A: Asset> Copy for WeakHandle<A> {

}

impl<A: Asset> Debug for WeakHandle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<A: Asset> Hash for WeakHandle<A> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<A: Asset> PartialEq for WeakHandle<A> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<A: Asset> Eq for WeakHandle<A> {
}

impl<A: Asset> PartialOrd for WeakHandle<A> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for WeakHandle<A> {
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
pub struct Handle<A: Asset> {
    pub(in crate::assets) index: u32,
    pub(in crate::assets) generation: u32,
    /// Shared ref-count marker. The slot holds a clone of this `Arc`;
    /// when `Arc::strong_count` drops to 1 (only the slot remains), the
    /// asset is eligible for off-loading.
    _ref: Arc<()>,
    _phantom: PhantomData<A>,
}

impl<A: Asset> Handle<A> {
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
    pub fn weak(&self) -> WeakHandle<A> {
        WeakHandle::new(self.index, self.generation)
    }
}

impl<A: Asset> Clone for Handle<A> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            generation: self.generation,
            _ref: Arc::clone(&self._ref),
            _phantom: PhantomData,
        }
    }
}

impl<A: Asset> From<&Handle<A>> for WeakHandle<A> {
    fn from(value: &Handle<A>) -> Self {
        value.weak()
    }
}

impl<A: Asset> From<Handle<A>> for WeakGenericHandle {
    fn from(value: Handle<A>) -> Self {
        Self {
            index: value.index,
            generation: value.generation,
            type_id: TypeId::of::<A>(),
        }
    }
}

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrongHandle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .field("ref_count", &Arc::strong_count(&self._ref))
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

impl<A: Asset> Eq for Handle<A> {}

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