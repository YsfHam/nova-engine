use std::{any::{Any, TypeId}, collections::HashMap};

use crate::{assets::{Asset, error::AssetError}, graphics::render::RenderContextRef};

/// Context handed to every [`AssetLoader::load`] call.
///
/// Carries everything a loader needs to build an asset:
/// - `render_ctx` — for GPU resource creation (kept as `Rc<RefCell<…>>` so a
///   loader can borrow it mutably if ever needed).
/// - `assets` — an immutable reference to the [`AssetsManager`](crate::assets::AssetsManager),
///   so a loader can **retrieve already-loaded dependencies** by handle
///   (e.g. a `MaterialTemplateLoader` resolves its `Shader` dependencies via
///   `ctx.assets.get_asset(handle)`). Loaders do *not* load new assets from
///   here — dependency loading (resolving paths → handles) is the caller's
///   job and will be automated when serialization lands.
///
/// `LoadContext` is a short-lived, stack-allocated object: it borrows the
/// manager for the duration of a single `load()` call and is dropped before
/// the manager inserts the resulting asset (see `AssetsManager::load`).
pub struct LoadContext<'a> {
    pub render_ctx: RenderContextRef,
    pub assets: &'a crate::assets::AssetsManager,
}

pub(crate) struct AssetLoadersStorage {
    /// One loader per asset type. The asset's `TypeId` is the key.
    loaders_by_asset_type: HashMap<TypeId, usize>,
    loaders: Vec<Box<dyn ErasedLoader>>,
}

impl AssetLoadersStorage {
    pub(crate) fn new() -> Self {
        Self {
            loaders: vec![],
            loaders_by_asset_type: HashMap::new(),
        }
    }

    pub(crate) fn add<L: AssetLoader>(&mut self, loader: L) {
        let asset_type_id = TypeId::of::<L::Asset>();
        let loader_index = self.loaders.len();

        // If a loader for this asset type already exists, overwrite the
        // stored index so subsequent lookups resolve to the new loader. The
        // old `Box` stays in `loaders` (unreferenced) rather than compacting
        // the vec — loaders are registered rarely, so the wasted slot is
        // negligible and compaction would invalidate any indices held
        // elsewhere.
        self.loaders_by_asset_type.insert(asset_type_id, loader_index);

        self.loaders.push(Box::new(loader));
    }

    /// Returns the loader for asset type `A`. Immutable borrow — loaders are
    /// stateless (`AssetLoader::load` takes `&self`), so no mutable access is
    /// needed to run one.
    pub(crate) fn get<A: Asset>(&self) -> Result<&dyn ErasedLoader, AssetError> {
        self.loaders_by_asset_type.get(&TypeId::of::<A>())
            .copied()
            .ok_or(AssetError::LoaderNotFound)
            .map(|index| self.loaders[index].as_ref())
    }
}

/// Loads an [`Asset`] from its [`Asset::Metadata`].
///
/// Loaders are **stateless**: `load` takes `&self` (not `&mut self`). If a
/// loader ever needs internal caching, give it interior mutability
/// (`RefCell`/`Mutex`). The loader receives the fully-resolved runtime
/// metadata — including [`Handle`](crate::assets::handle::Handle)s to
/// dependencies — and the [`LoadContext`] for any already-loaded dependency
/// retrieval (`ctx.assets.get_asset(handle)`).
///
/// Each asset type has exactly one loader implementation (registered via
/// [`AssetsManager::register_loader`](crate::assets::AssetsManager::register_loader)).
pub trait AssetLoader: 'static {
    type Asset: Asset;

    fn load(&self, metadata: <Self::Asset as Asset>::Metadata, ctx: &LoadContext<'_>) -> Result<Self::Asset, AssetError>;
}

/// Type-erased loader boundary. The `metadata` argument is a boxed
/// `A::Metadata` downcast inside [`ErasedLoader::load_erased`].
pub(crate) trait ErasedLoader: 'static {
    fn load_erased(&self, metadata: Box<dyn Any>, ctx: &LoadContext<'_>) -> Result<Box<dyn Any>, AssetError>;
}

impl<L: AssetLoader> ErasedLoader for L {
    fn load_erased(&self, metadata: Box<dyn Any>, ctx: &LoadContext<'_>) -> Result<Box<dyn Any>, AssetError> {
        let metadata = metadata
            .downcast::<<L::Asset as Asset>::Metadata>()
            .map_err(|_| AssetError::MetadataTypeMismatch)?;
        let asset = self.load(*metadata, ctx)?;
        Ok(Box::new(asset))
    }
}