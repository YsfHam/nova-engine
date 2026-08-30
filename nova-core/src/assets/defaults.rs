use std::{any::{TypeId, type_name, type_name_of_val}, collections::HashMap};

use crate::assets::{Asset, handle::{GenericHandle, Handle}};

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

    pub fn get<A: Asset>(&self, key: impl DefaultAssetsKey) -> Option<Handle<A>> {
        let generic_handle = self.assets.get(&key.key())?;
        (*generic_handle).try_into().ok()
    }

    pub fn expect<A: Asset>(&self, key: impl DefaultAssetsKey) -> Handle<A> {
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
    DefaultSampler,
    WhiteTexture,
}

impl DefaultAssetsKey for CoreDefaultAssets {

    fn as_str(&self) -> &'static str {
        match self {
            CoreDefaultAssets::DefaultSampler => "DefaultSampler",
            CoreDefaultAssets::WhiteTexture => "WhiteTexture",
        }
    }
}