use std::{any::{Any, TypeId}, cell::RefCell, collections::HashMap, rc::Rc};

use crate::{assets::{Asset, error::AssetError}, graphics::render::RenderContext};


pub struct LoadContext {
    pub render_ctx: Rc<RefCell<RenderContext>>,
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

    pub(crate) fn get<A: Asset>(&mut self) -> Result<&mut Box<dyn ErasedLoader>, AssetError> {
        self.loaders_by_asset_type.get(&TypeId::of::<A>())
            .copied()
            .ok_or(AssetError::LoaderNotFound)
            .map(|index| self.loaders.get_mut(index).unwrap())
    }
}

/// Loads an [`Asset`] from its [`Asset::Metadata`].
///
/// Each asset type has exactly one loader implementation (registered via
/// [`AssetsManager::register_loader`](crate::assets::AssetsManager::register_loader)).
/// The loader receives the fully-resolved runtime metadata — including
/// `Handle`s to dependencies — and produces the asset.
pub trait AssetLoader: 'static {
    type Asset: Asset;

    fn load(&mut self, metadata: <Self::Asset as Asset>::Metadata, ctx: &LoadContext) -> Result<Self::Asset, AssetError>;
}

/// Type-erased loader boundary. The `metadata` argument is a boxed
/// `A::Metadata` downcast inside [`ErasedLoader::load_erased`].
pub(crate) trait ErasedLoader: 'static {
    fn load_erased(&mut self, metadata: Box<dyn Any>, ctx: &LoadContext) -> Result<Box<dyn Any>, AssetError>;
}

impl<L: AssetLoader> ErasedLoader for L {
    fn load_erased(&mut self, metadata: Box<dyn Any>, ctx: &LoadContext) -> Result<Box<dyn Any>, AssetError> {
        let metadata = metadata
            .downcast::<<L::Asset as Asset>::Metadata>()
            .map_err(|_| AssetError::MetadataTypeMismatch)?;
        let asset = self.load(*metadata, ctx)?;
        Ok(Box::new(asset))
    }
}