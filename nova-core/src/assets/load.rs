use std::{any::{Any, TypeId}, collections::HashMap, path::Path, sync::Arc};

use crate::{assets::{Asset, error::AssetError}, graphics::context::GraphicsContext};


pub struct LoadContext {
    pub gfx: Arc<GraphicsContext>,
}

pub(crate) struct AssetLoadersStorage {
    loaders_asset_ext: HashMap<String, HashMap<TypeId, usize>>,
    loaders_map: HashMap<TypeId, usize>,
    loaders: Vec<Box<dyn ErasedLoader>>,
}

impl AssetLoadersStorage {
    pub(crate) fn new() -> Self {
        Self {
            loaders: vec![],
            loaders_map: HashMap::new(),
            loaders_asset_ext: HashMap::new(),
        }
    }

    pub(crate) fn add<L: AssetLoader>(&mut self, loader: L) {
        let extensions = loader.extensions();
        let loader_index = self.loaders.len();

        self.loaders_map.insert(TypeId::of::<L>(), loader_index);

        self.loaders.push(Box::new(loader));

        for ext in extensions {
            self.loaders_asset_ext.entry(ext)
            .or_insert_with(|| HashMap::new())
            .insert(TypeId::of::<L::Asset>(), loader_index);
        }
    }

    pub(crate) fn get_by_ext<A: Asset>(&mut self, ext: &str) -> Result<&mut Box<dyn ErasedLoader>, AssetError> {
        self.loaders_asset_ext.get(ext)
        .ok_or(AssetError::UnsupportedExtension)
        .and_then(|ext_loaders| 
                ext_loaders.get(&TypeId::of::<A>())
                .ok_or(AssetError::LoaderNotFound)
        )
        .map(|&index| self.loaders.get_mut(index).unwrap())
    }

    pub(crate) fn get_by_type<L: AssetLoader>(&mut self) -> Result<&mut Box<dyn ErasedLoader>, AssetError> {
        self.loaders_map.get(&TypeId::of::<L>())
        .ok_or(AssetError::LoaderNotFound)
        .map(|&index| self.loaders.get_mut(index).unwrap())
    }
}

pub trait AssetLoader: 'static {
    type Asset: Asset;

    fn load(&mut self, path: &Path, ctx: &LoadContext) -> Result<Self::Asset, AssetError>;
    fn extensions(&self) -> Vec<String>;
}

pub(crate) trait ErasedLoader: 'static {
    fn load_erased(&mut self, path: &Path, ctx: &LoadContext) -> Result<Box<dyn Any>, AssetError>;
}

impl<L: AssetLoader> ErasedLoader for L {
    fn load_erased(&mut self, path: &Path, ctx: &LoadContext) -> Result<Box<dyn Any>, AssetError> {
        let asset = self.load(path, ctx)?;
        Ok(Box::new(asset))
    }
}