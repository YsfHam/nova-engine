use std::{any::{Any, TypeId}, collections::HashMap, path::Path};
use crate::{assets::{error::AssetError, handle::Handle, load::{AssetLoader, AssetLoadersStorage, LoadContext}, storage::AssetStorage}, graphics::render::RenderContextRef};

pub mod handle;
pub mod load;
pub mod error;
pub mod resolve;
pub mod defaults;
mod storage;

/// An asset stored in the [`AssetsManager`].
///
/// Each asset type declares a [`Metadata`] type that fully describes how to
/// (re)create the asset: its source data plus any refinement parameters
/// (mip levels, sampler config, shader stage flags, …). The metadata also
/// carries [`Handle`]s to dependent assets (e.g. a `Texture` depends on a
/// `Sampler`), so loading is a single call once dependencies are resolved.
///
/// The asset owns its metadata (accessible via [`Asset::metadata`]) so it is
/// self-describing — this is what makes future serialization, hot-reload and
/// asset deduplication possible: the metadata *is* the asset's identity.
///
/// [`Metadata`]: Self::Metadata
pub trait Asset: 'static {
    /// Fully describes how to create this asset. Stored inside the asset and
    /// used as its identity for serialization / dedup / hot-reload.
    type Metadata: Any + Send + Sync + Clone + 'static;

    /// Returns the metadata this asset was created from.
    fn metadata(&self) -> &Self::Metadata;
}

/// Owns the asset storages and loader registry.
///
/// The storages and loaders are plain owned fields (no interior mutability)
/// — `AssetsManager` is the single owner. [`LoadContext`] borrows it
/// immutably (`&AssetsManager`) for the duration of a `load()` call so
/// loaders can retrieve already-loaded dependencies via `get_asset`. The
/// immutable borrow ends before `load()` inserts the new asset, so the
/// final `&mut self` insert is borrow-checker-safe.
pub struct AssetsManager {
    storages: HashMap<TypeId, Box<dyn Any>>,
    loaders: AssetLoadersStorage,
    render_ctx: RenderContextRef,
}

impl AssetsManager {
    pub(crate) fn new(render_ctx: RenderContextRef) -> Self {
        Self {
            storages: HashMap::new(),
            loaders: AssetLoadersStorage::new(),
            render_ctx,
        }
    }

    pub fn register_loader<L: AssetLoader>(&mut self, loader: L) {
        self.loaders.add(loader);
    }

    /// Loads an asset from fully-resolved runtime metadata.
    ///
    /// The metadata must already contain [`Handle`]s to any dependencies
    /// (e.g. a `TextureMetadata` carries a `Handle<Sampler>`). For the
    /// file-based variant where dependencies are referenced by relative path,
    /// see [`AssetsManager::load_from_file`].
    ///
    pub fn load<A: Asset>(&mut self, metadata: A::Metadata) -> Result<Handle<A>, AssetError> {
        let ctx = LoadContext {
            render_ctx: self.render_ctx.clone(),
            assets: self,
        };
        let loader = self.loaders.get::<A>()?;
        let asset = loader
            .load_erased(Box::new(metadata), &ctx)?
            .downcast::<A>()
            .unwrap();
        let asset = *asset;

        Ok(self.insert_asset(asset))
    }

    /// Loads an asset from a metadata file on disk.
    ///
    /// The metadata file is a serializable description of the asset: it
    /// stores the asset's source plus, for dependencies, **relative paths**
    /// pointing at *other* metadata files. `load_from_file` reads the file,
    /// recursively resolves dependencies (loading each via this same method),
    /// converts the file-form metadata into the runtime form (replacing
    /// paths with `Handle`s) and delegates to [`AssetsManager::load`].
    ///
    /// **Not implemented yet** — serialization lands in a later step. For now
    /// this always returns [`AssetError::NotImplemented`]. Build metadata in
    /// code and call [`AssetsManager::load`] directly.
    pub fn load_from_file<A: Asset>(&mut self, _path: impl AsRef<Path>) -> Result<Handle<A>, AssetError> {
        unimplemented!()
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

    fn get_or_create_storage_mut<A: Asset>(&mut self) -> &mut AssetStorage<A> {
        self.storages
            .entry(TypeId::of::<A>())
            .or_insert_with(|| Box::new(AssetStorage::<A>::new()))
            .downcast_mut()
            .unwrap()
    }

    fn get_storage<A: Asset>(&self) -> Option<&AssetStorage<A>> {
        self.storages
            .get(&TypeId::of::<A>())
            .and_then(|any| any.downcast_ref())
    }
}