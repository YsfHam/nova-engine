use std::{any::{TypeId, type_name, type_name_of_val}, collections::HashMap};

use crate::assets::{Asset, handle::{WeakHandle, Handle, GenericHandle}};

#[derive(Debug)]
pub struct DuplicatedDefaultAssetError {
    pub asset_name: &'static str,
    pub key_name: String,
}

pub struct DefaultAssets {
    assets: HashMap<String, GenericHandle>,
}

impl DefaultAssets {

    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn insert<A: Asset>(&mut self, key: impl DefaultAssetsKey, handle: Handle<A>) -> Result<(), DuplicatedDefaultAssetError> {
        let key_name = key.key();
        let old_value = self.assets.insert(key_name, handle.into());
        if old_value.is_some() {
            Err(DuplicatedDefaultAssetError {
                asset_name: type_name::<A>(),
                key_name: format!("{}:{}", type_name_of_val(&key), key.as_str()),
            })
        }
        else {
            Ok(())
        }
    }

    pub fn get<A: Asset>(&self, key: impl DefaultAssetsKey) -> Option<WeakHandle<A>> {
        let generic_handle = self.assets.get(&key.key())?;
        generic_handle.weak().try_into_handle::<A>().ok()
    }

    pub fn expect<A: Asset>(&self, key: impl DefaultAssetsKey) -> WeakHandle<A> {
        let debug_key_type_name = type_name_of_val(&key);
        let key_name = key.as_str();
        self.get(key)
        .unwrap_or_else(|| panic!("Asset not found for key {}:{}", debug_key_type_name, key_name))
    }
}

pub trait DefaultAssetsKey: 'static {
    fn as_str(&self) -> &'static str;

    fn key(&self) -> String {
        format!("Type_{:?}:{}", TypeId::of::<Self>(), self.as_str())
    }
}

pub enum CoreDefaultAssets {
    WhiteTexture,
}

impl DefaultAssetsKey for CoreDefaultAssets {

    fn as_str(&self) -> &'static str {
        match self {
            CoreDefaultAssets::WhiteTexture => "WhiteTexture",
        }
    }
}