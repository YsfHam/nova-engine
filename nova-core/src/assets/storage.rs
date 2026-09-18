use std::{any::Any, sync::Arc};

use crate::assets::{AnyAssetState, Asset, AssetMutState, AssetState, error::AssetError, handle::{GenericHandle, Handle, StrongHandle}};

pub(crate) trait ErasedStorage {
    fn get_any<'a>(&'a self, handle: GenericHandle) -> AnyAssetState<'a>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Removes all `Ready` assets whose ref count has dropped to 1 (only the
    /// slot holds a reference). Bumps generation on recycled slots so stale
    /// weak handles stop matching. Returns the number of assets removed.
    fn cleanup_offloaded(&mut self) -> usize;
    /// Completes an async load by inserting the loaded asset (as `Box<dyn Any>`)
    /// into the slot at `(index, generation)`. Returns `true` on success.
    fn complete_load(&mut self, index: u32, generation: u32, asset: Box<dyn Any>) -> bool;
    /// Marks an async load as failed, storing the error. Returns `true` on success.
    fn set_failed_any(&mut self, index: u32, generation: u32, error: AssetError) -> bool;
}

enum SlotState<A: Asset> {
    Loading,
    Ready(A),
    Failed(AssetError),
    Empty,
}

impl<'a, A: Asset> From<&'a SlotState<A>> for AssetState<'a, A> {
    fn from(value: &'a SlotState<A>) -> Self {
        match value {
            SlotState::Loading => Self::Loading,
            SlotState::Ready(asset) => Self::Ready(asset),
            SlotState::Failed(error) => Self::Failed(error.clone()),
            SlotState::Empty => Self::Empty,
        }
    }
}

impl<'a, A: Asset> From<&'a mut SlotState<A>> for AssetMutState<'a, A> {
    fn from(value: &'a mut SlotState<A>) -> Self {
        match value {
            SlotState::Loading => Self::Loading,
            SlotState::Ready(asset) => Self::Ready(asset),
            SlotState::Failed(error) => Self::Failed(error.clone()),
            SlotState::Empty => Self::Empty,
        }
    }
}

struct Slot<A: Asset> {
    next_empty: Option<u32>,
    generation: u32,
    state: SlotState<A>,
    /// Shared ref-count marker. Cloned into every `StrongHandle` issued
    /// from this slot. When `Arc::strong_count` drops to 1 (only this
    /// slot holds a clone), the asset is eligible for off-loading.
    /// Recreated (new `Arc`) every time the slot is reused with a new
    /// generation, so stale `StrongHandle`s from a previous generation
    /// don't interfere with the new asset's ref count.
    ref_marker: Option<Arc<()>>,
}

pub struct AssetStorage<A: Asset> {
    storage: Vec<Slot<A>>,
    empty_slot: Option<u32>,
}

impl<A: Asset> AssetStorage<A> {
    pub fn new() -> Self {
        Self {
            storage: vec![],
            empty_slot: None,
        }
    }

    pub fn insert(&mut self, asset: A) -> StrongHandle<A> {
       let handle = self.reserve_for_load();
       let slot = self.storage.get_mut(handle.index as usize).unwrap();
       slot.state = SlotState::Ready(asset);
       handle
    }

    pub fn get(&self, handle: Handle<A>) -> AssetState<'_, A> {
        let Some(slot) = 
            self.storage.get(handle.index as usize)
        else {return AssetState::Empty };
        if slot.generation == handle.generation {
            AssetState::from(&slot.state)
        } else {
            AssetState::Empty
        }
    }

    pub fn get_mut(&mut self, handle: Handle<A>) -> AssetMutState<'_, A> {
        let Some(slot) = 
            self.storage.get_mut(handle.index as usize)
        else {return AssetMutState::Empty };
        if slot.generation == handle.generation {
            AssetMutState::from(&mut slot.state)
        } else {
            AssetMutState::Empty
        }
    }

    /// Reserves a slot for an async-loaded asset. The slot starts in `Loading`
    /// state with no data. Returns a `StrongHandle` immediately — the caller
    /// can poll [`asset_state`](Self::asset_state) to check progress.
    ///
    /// The asset data is filled later by [`complete_load`](Self::complete_load)
    /// or marked as failed by [`set_failed`](Self::set_failed).
    pub fn reserve_for_load(&mut self) -> StrongHandle<A> {
        match self.empty_slot {
            Some(index) => {
                let slot = self.storage.get_mut(index as usize).unwrap();
                let generation = slot.generation;

                let ref_marker = Arc::new(());
                slot.ref_marker = Some(Arc::clone(&ref_marker));
                slot.state = SlotState::Loading;
                let next_empty = slot.next_empty.take();
                self.empty_slot = next_empty;

                StrongHandle::new(index, generation, ref_marker)
            }
            None => {
                let index = self.storage.len() as u32;
                let generation = 0;
                let ref_marker = Arc::new(());

                self.storage.push(Slot {
                    next_empty: None,
                    generation,
                    state: SlotState::Loading,
                    ref_marker: Some(Arc::clone(&ref_marker)),
                });

                StrongHandle::new(index, generation, ref_marker)
            }
        }
    }

    /// Completes an async load by filling the slot's data and transitioning
    /// to `Ready`. The `index` and `generation` must match a `Loading` slot.
    ///
    /// Returns `true` if the load was completed, `false` if the slot was not
    /// in `Loading` state or the generation didn't match (e.g. the slot was
    /// recycled while loading).
    pub fn complete_load(&mut self, index: u32, generation: u32, asset: A) -> bool {
        let Some(slot) = self.storage.get_mut(index as usize) else {
            return false;
        };
        if slot.generation != generation || !matches!(slot.state, SlotState::Loading) {
            return false;
        }
        slot.state = SlotState::Ready(asset);
        true
    }

    /// Marks an async load as failed. The slot transitions to `Failed` carrying
    /// the `error` but **keeps its generation** so the caller's `StrongHandle`
    /// still resolves to this slot — [`asset_state`](Self::asset_state) returns
    /// `Failed(error)`, allowing the caller to distinguish "load failed" from
    /// "handle invalid" and inspect the cause.
    ///
    /// Returns `true` if the slot was marked as failed, `false` if the slot
    /// was not in `Loading` state or the generation didn't match.
    pub fn set_failed(&mut self, index: u32, generation: u32, error: AssetError) -> bool {
        let Some(slot) = self.storage.get_mut(index as usize) else {
            return false;
        };
        if slot.generation != generation || !matches!(slot.state, SlotState::Loading) {
            return false;
        }
        slot.state = SlotState::Failed(error);
        true
    }

    /// Removes all `Ready` assets whose ref count has dropped to 1 (only the
    /// slot holds a reference — no `StrongHandle`s remain). Each removed slot
    /// is recycled: generation bumped, data dropped, linked to the free list.
    ///
    /// `Loading` and `Failed` slots are left alone — a loading asset has an
    /// in-flight `StrongHandle` from the caller; a failed asset keeps its
    /// generation so the caller can still detect the failure.
    pub fn cleanup_offloaded(&mut self) -> usize {
        let mut removed = 0;

        for i in 0..self.storage.len() {
            let slot = &self.storage[i];

            // Only off-load Ready assets with no outstanding strong handles.
            let should_offload = 
                slot.ref_marker.as_ref().map_or(false, |rc| Arc::strong_count(rc) == 1);

            if !should_offload {
                continue;
            }

            let slot = &mut self.storage[i];
            slot.next_empty = self.empty_slot;
            self.empty_slot = Some(i as u32);
            slot.generation = slot.generation.wrapping_add(1);
            slot.state = SlotState::Empty;
            slot.ref_marker = None;
            removed += 1;
        }

        removed
    }
}

impl<A: Asset> ErasedStorage for AssetStorage<A> {
    fn get_any<'a>(&'a self, handle: GenericHandle) -> AnyAssetState<'a> {
        let Some(typed) = 
            handle.try_into_handle::<A>().ok()
        else {return AnyAssetState::Empty};
        self.get(typed)
        .into()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn cleanup_offloaded(&mut self) -> usize {
        AssetStorage::cleanup_offloaded(self)
    }

    fn complete_load(&mut self, index: u32, generation: u32, asset: Box<dyn Any>) -> bool {
        let Ok(asset) = asset.downcast::<A>() else {
            return false;
        };
        self.complete_load(index, generation, *asset)
    }

    fn set_failed_any(&mut self, index: u32, generation: u32, error: AssetError) -> bool {
        self.set_failed(index, generation, error)
    }
}

