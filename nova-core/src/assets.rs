use std::{any::{Any, TypeId}, collections::HashMap};
use crate::assets::{handle::{GenericHandle, Handle}, storage::{AssetStorage, ErasedStorage}};

pub mod handle;
pub mod error;
pub mod defaults;
mod storage;

/// A type stored in the [`AssetsManager`].
///
/// Assets are inserted directly by the user (or constructed from a file
/// when serialization lands). There is no loader registry and no metadata
/// type — the asset owns its own data and identity.
pub trait Asset: 'static {}

/// Owns asset storages keyed by `TypeId`.
///
/// Each asset type gets its own [`AssetStorage<A>`]. Handles are typed
/// (`Handle<A>`) and can be converted to a [`GenericHandle`] for type-erased
/// storage (e.g. in `DrawBatch`). Use [`get_asset_any`] to retrieve an asset
/// from a `GenericHandle` when the concrete type is not known at the call
/// site — the returned `&dyn Any` can be downcast by the caller.
pub struct AssetsManager {
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
}

impl AssetsManager {
    pub(crate) fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    pub fn insert_asset<A: Asset>(&mut self, asset: A) -> Handle<A> {
        let storage = self.get_or_create_storage_mut();
        storage.insert(asset)
    }

    pub fn get_asset<A: Asset>(&self, handle: Handle<A>) -> Option<&A> {
        let storage = self.get_storage()?;
        storage.get(handle)
    }

    pub fn get_asset_mut<A: Asset>(&mut self, handle: Handle<A>) -> Option<&mut A> {
        let storage = self.get_or_create_storage_mut();
        storage.get_mut(handle)
    }

    pub fn remove_asset<A: Asset>(&mut self, handle: Handle<A>) -> Option<A> {
        let storage = self.get_or_create_storage_mut();
        storage.remove(handle)
    }

    /// Retrieves an asset from a [`GenericHandle`] without knowing the concrete
    /// type at the call site. Returns `&dyn Any` for the caller to downcast.
    ///
    /// The `GenericHandle`'s `type_id` must match the stored asset type, and
    /// the generation must match. Returns `None` for stale or unregistered
    /// handles.
    pub fn get_asset_any(&self, handle: GenericHandle) -> Option<&dyn Any> {
        let storage = self.storages.get(&handle.type_id)?;
        storage.get_any(handle)
    }


    fn get_or_create_storage_mut<A: Asset>(&mut self) -> &mut AssetStorage<A> {
        self.storages
            .entry(TypeId::of::<A>())
            .or_insert_with(|| Box::new(AssetStorage::<A>::new()) as Box<dyn ErasedStorage>)
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn get_storage<A: Asset>(&self) -> Option<&AssetStorage<A>> {
        self.storages
            .get(&TypeId::of::<A>())
            .and_then(|any| any.as_any().downcast_ref())
    }
}