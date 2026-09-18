use std::{
    any::TypeId,
    collections::HashMap,
    sync::mpsc,
};
use crate::assets::{
    error::AssetError, handle::{GenericHandle, Handle, StrongHandle}, load::{LoadJob, LoadResult, erase_supplier, init_load_job_processor}, storage::{AssetStorage, ErasedStorage},
};

pub mod handle;
pub mod error;
pub mod defaults;
mod state;
mod load;
mod storage;
pub use state::*;


/// A type stored in the [`AssetsManager`].
///
/// Assets are inserted directly by the user (or constructed from a file
/// when serialization lands). There is no loader registry and no metadata
/// type — the asset owns its own data and identity.
///
/// `Send + Sync` is required so that assets can be loaded on worker threads
/// and then inserted into the storages from the main thread.
pub trait Asset: Send + Sync + 'static {}


/// Owns asset storages keyed by `TypeId` and a thread pool for async loading.
///
/// Each asset type gets its own [`AssetStorage<A>`]. Handles are typed
/// (`Handle<A>`) and can be converted to a [`GenericHandle`] for type-erased
/// storage (e.g. in `DrawBatch`). Use [`get_asset_any`] to retrieve an asset
/// from a `GenericHandle` when the concrete type is not known at the call
/// site — the returned `&dyn Any` can be downcast by the caller.
///
/// # Async loading
///
/// Call [`load`](Self::load) with a supplier closure to dispatch the load
/// to a worker thread. The method returns a `StrongHandle` immediately
/// (slot in `Loading` state). Call [`drain_loaded`](Self::drain_loaded) each
/// frame to process completed loads — this transitions slots to `Ready` or
/// `Failed`.
pub struct AssetsManager {
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
    /// Sender for dispatching load jobs to the worker thread.
    job_tx: mpsc::Sender<LoadJob>,
    /// Receiver for completed load results.
    load_results: mpsc::Receiver<LoadResult>,
}

impl AssetsManager {
    pub(crate) fn new() -> Self {
        // Single worker thread for now — can be expanded to a pool later.
        let (job_tx, job_rx) = mpsc::channel::<LoadJob>();
        let (result_tx, result_rx) = mpsc::channel::<LoadResult>();

        init_load_job_processor(job_rx, result_tx);

        Self {
            storages: HashMap::new(),
            job_tx,
            load_results: result_rx,
        }
    }

    pub fn insert_asset<A: Asset>(&mut self, asset: A) -> StrongHandle<A> {
        let storage = self.get_or_create_storage_mut();
        storage.insert(asset)
    }

    pub fn get_asset<A: Asset>(&self, handle: Handle<A>) -> AssetState<'_, A> {
        let Some(storage) = 
            self.get_storage()
        else {return AssetState::Empty};
        storage.get(handle)
    }

    pub fn get_asset_mut<A: Asset>(&mut self, handle: Handle<A>) -> AssetMutState<'_, A> {
        let Some(storage) =
            self.get_storage_mut()
        else {return AssetMutState::Empty};
        storage.get_mut(handle)
    }

    /// Retrieves an asset from a [`GenericHandle`] without knowing the concrete
    /// type at the call site. Returns `&dyn Any` for the caller to downcast.
    ///
    /// The `GenericHandle`'s `type_id` must match the stored asset type, and
    /// the generation must match. Returns `None` for stale or unregistered
    /// handles.
    pub fn get_asset_any(&self, handle: GenericHandle) -> AnyAssetState<'_> {
        let Some(storage) = 
            self.storages.get(&handle.type_id)
        else {return AnyAssetState::Empty};
        storage.get_any(handle)
    }

    /// Removes all assets whose ref count has dropped to 1 (only the slot
    /// holds a reference — no `StrongHandle`s remain). Should be called once
    /// per frame (e.g. in `on_update`) to automatically off-load unused assets.
    ///
    /// Returns the total number of assets removed across all storages.
    pub(crate) fn cleanup_offloaded(&mut self) -> usize {
        self.storages
            .values_mut()
            .map(|storage| storage.cleanup_offloaded())
            .sum()
    }

    /// Dispatches an async asset load to a worker thread. Returns a
    /// `StrongHandle` immediately — the slot is in `Loading` state. The
    /// asset data will be filled in by [`drain_loaded`](Self::drain_loaded)
    /// when the load completes.
    ///
    /// The `supplier` closure captures whatever inputs it needs (file paths,
    /// configs, etc.) and is moved to the worker thread. It must be pure CPU
    /// work (file I/O, decoding, parsing) with **no GPU access** — GPU
    /// resource creation happens lazily on the main thread when the asset is
    /// first accessed by the renderer.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let handle: StrongHandle<Texture> = assets_manager.load(|| {
    ///     Texture::from_file("assets/sprite.png", config, sampler)
    ///         .map_err(AssetError::from)
    /// });
    /// ```
    pub fn load<A: Asset>(
        &mut self,
        supplier: impl FnOnce() -> Result<A, AssetError> + Send + 'static,
    ) -> StrongHandle<A> {
        // Reserve a slot in Loading state.
        let storage = self.get_or_create_storage_mut::<A>();
        let strong_handle = storage.reserve_for_load();
        let index = strong_handle.index;
        let generation = strong_handle.generation;
        let type_id = TypeId::of::<A>();

        // Send the job to the worker thread.
        let job = LoadJob {
            supplier: erase_supplier(supplier),
            index,
            generation,
            type_id,
        };
        self.job_tx
            .send(job)
            .expect("asset loader thread has died");

        strong_handle
    }

    /// Drains all completed load results from the worker thread and
    /// transitions the corresponding slots to `Ready` or `Failed`.
    ///
    /// Should be called once per frame (e.g. in `on_update`) before
    /// [`cleanup_offloaded`](Self::cleanup_offloaded) so that newly loaded
    /// assets are available and failed loads are reported.
    ///
    /// Returns the number of loads processed (both successful and failed).
    pub(crate) fn drain_loaded(&mut self) -> usize {
        let mut count = 0;
        while let Ok(result) = self.load_results.try_recv() {
            count += 1;
            match result {
                LoadResult::Ok { type_id, index, generation, asset } => {
                    if let Some(storage) = self.storages.get_mut(&type_id) {
                        storage.complete_load(index, generation, asset);
                    }
                }
                LoadResult::Failed { type_id, index, generation, error } => {
                    if let Some(storage) = self.storages.get_mut(&type_id) {
                        storage.set_failed_any(index, generation, error);
                    }
                }
            }
        }
        count
    }


    fn get_or_create_storage_mut<A: Asset>(&mut self) -> &mut AssetStorage<A> {
        self.storages
            .entry(TypeId::of::<A>())
            .or_insert_with(|| Box::new(AssetStorage::<A>::new()) as Box<dyn ErasedStorage>)
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn get_storage_mut<A: Asset>(&mut self) -> Option<&mut AssetStorage<A>> {
        self.storages
            .get_mut(&TypeId::of::<A>())
            .and_then(|any| any.as_any_mut().downcast_mut())
    }

    fn get_storage<A: Asset>(&self) -> Option<&AssetStorage<A>> {
        self.storages
            .get(&TypeId::of::<A>())
            .and_then(|any| any.as_any().downcast_ref())
    }
}