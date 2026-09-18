use std::any::Any;

use crate::assets::{Asset, error::AssetError};

pub enum AssetState<'a, A: Asset> {
    /// Asset is being loaded on a worker thread.
    Loading,
    /// Asset is ready and available.
    Ready(&'a A),
    /// Asset loading failed. The slot retains this state (and its generation)
    /// so the caller can distinguish "load failed" from "handle invalid".
    Failed(AssetError),
    /// Slot is empty / handle is stale (generation mismatch).
    Empty,
}

pub enum AssetMutState<'a, A: Asset> {
    /// Asset is being loaded on a worker thread.
    Loading,
    /// Asset is ready and available.
    Ready(&'a mut A),
    /// Asset loading failed. The slot retains this state (and its generation)
    /// so the caller can distinguish "load failed" from "handle invalid".
    Failed(AssetError),
    /// Slot is empty / handle is stale (generation mismatch).
    Empty,
}


pub enum AnyAssetState<'a> {
    Loading,
    Ready(&'a dyn Any),
    Failed(AssetError),
    Empty
}

impl<'a> AnyAssetState<'a> {
    pub fn downcast_ref<A: Asset>(&self) -> Option<&A> {
        match self {
            Self::Ready(any) => any.downcast_ref::<A>(),
            _ => None
        }
    }
}

impl<'a, A: Asset> From<AssetState<'a, A>> for AnyAssetState<'a> {
    fn from(value: AssetState<'a, A>) -> Self {
        match value {
            AssetState::Loading => Self::Loading,
            AssetState::Ready(a) => Self::Ready(a as &dyn Any),
            AssetState::Failed(error) => Self::Failed(error),
            AssetState::Empty => Self::Empty,
        }
    }
}

impl<A: Asset> AssetState<'_, A> {
    pub fn get(&self) -> Option<&A> {
        match self {
            AssetState::Ready(asset) => Some(asset),
            _ => None,
        }
    }
}

impl<A: Asset> AssetMutState<'_,  A> {
    pub fn get_mut(&mut self) -> Option<&mut A> {
        match self {
            AssetMutState::Ready(asset) => Some(asset),
            _ => None,
        }
    }
}