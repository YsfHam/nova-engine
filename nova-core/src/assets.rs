use std::{any::{Any, TypeId}, cell::RefCell, collections::HashMap, path::Path, rc::Rc};
use crate::{assets::{error::AssetError, handle::Handle, load::{AssetLoader, AssetLoadersStorage, LoadContext}, storage::AssetStorage}, graphics::render::RenderContext};

pub mod handle;
pub mod load;
pub mod error;
mod storage;

pub trait Asset: 'static {}

pub struct AssetsManager {
    storages: HashMap<TypeId, Box<dyn Any>>,
    loaders: AssetLoadersStorage,
    render_ctx: Rc<RefCell<RenderContext>>,
}

impl AssetsManager {
    pub(crate) fn new(render_ctx: Rc<RefCell<RenderContext>>) -> Self {
        Self {
            storages: HashMap::new(),
            loaders: AssetLoadersStorage::new(),
            render_ctx,
        }
    }

    pub fn register_loader<L: AssetLoader>(&mut self, loader: L) {
        self.loaders.add(loader);
    }

    pub fn load<A: Asset, P: AsRef<Path>>(&mut self, path: P) -> Result<Handle<A>, AssetError> {
        let path = path.as_ref();
        let ext = path.extension().ok_or(AssetError::FileMissingExtension)?;
        let ctx = self.load_context();
        let loader = self.loaders.get_by_ext::<A>(&ext.to_string_lossy())?;

        let asset = loader 
            .load_erased(path, &ctx)?
            .downcast::<A>()
            .unwrap()
        ;

        Ok(self.insert_asset(*asset))
    }

    pub fn load_with_hint<A: Asset, L: AssetLoader, P: AsRef<Path>>(&mut self, path: P) -> Result<Handle<A>, AssetError> {
        let ctx = self.load_context();
        let loader = self.loaders.get_by_type::<L>()?;

        let asset = loader 
            .load_erased(path.as_ref(), &ctx)?
            .downcast::<A>()
            .unwrap()
        ;

        Ok(self.insert_asset(*asset))
    }

    pub fn insert_asset<A: Asset>(&mut self, asset: A) -> Handle<A> {
        let storage = self.get_storage_mut();
        storage.insert(asset)
    }

    pub fn get_asset<A: Asset>(&self, handle: Handle<A>) -> Option<&A> {
        let storage = self.get_storage();
        storage.get(handle)
    }

    pub fn get_asset_mut<A: Asset>(&mut self, handle: Handle<A>) -> Option<&mut A> {
        let storage = self.get_storage_mut();
        storage.get_mut(handle)
    }

    pub fn remove_asset<A: Asset>(&mut self, handle: Handle<A>) -> Option<A> {
        let storage = self.get_storage_mut();
        storage.remove(handle)
    }

    fn get_storage_mut<A: Asset>(&mut self) -> &mut AssetStorage<A> {
        self.storages.entry(TypeId::of::<A>())
        .or_insert_with(|| Box::new(AssetStorage::<A>::new()))
        .downcast_mut()
        .unwrap()
    }

    fn get_storage<A: Asset>(&self) -> &AssetStorage<A> {
        self.storages.get(&TypeId::of::<A>())
        .unwrap()
        .downcast_ref()
        .unwrap()
    }

    fn load_context(&self) -> LoadContext {
        LoadContext {
            render_ctx: self.render_ctx.clone(),
        }
    }
}