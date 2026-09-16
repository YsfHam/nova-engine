use std::{any::Any, sync::Arc};

use crate::assets::{Asset, error::AssetError, handle::{GenericHandle, Handle, StrongHandle}};

pub(crate) trait ErasedStorage {
    fn get_any(&self, handle: GenericHandle) -> Option<&dyn Any>;
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

enum SlotState {
    Loading,
    Ready,
    Failed(AssetError),
    Empty,
}

/// The load state of an asset slot, queryable via [`AssetStorage::asset_state`].
#[derive(Debug, Clone)]
pub enum AssetState {
    /// Asset is being loaded on a worker thread.
    Loading,
    /// Asset is ready and available.
    Ready,
    /// Asset loading failed. The slot retains this state (and its generation)
    /// so the caller can distinguish "load failed" from "handle invalid".
    Failed(AssetError),
    /// Slot is empty / handle is stale (generation mismatch).
    Empty,
}

struct Slot<A: Asset> {
    data: Option<A>,
    next_empty: Option<u32>,
    generation: u32,
    state: SlotState,
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
        match self.empty_slot {
            Some(empty_slot) => {
                let (handle, next_empty) = self.update_slot(empty_slot, asset);
                self.empty_slot = next_empty;
                handle
            }
            None => self.add_asset(asset)
        }
    }

    pub fn get(&self, handle: Handle<A>) -> Option<&A> {
        let slot = self.storage.get(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.data.as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, handle: Handle<A>) -> Option<&mut A> {
        let slot = self.storage.get_mut(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.data.as_mut()
        } else {
            None
        }
    }

    pub fn remove(&mut self, handle: Handle<A>) -> Option<A> {
        let slot = self.storage.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        let data = slot.data.take();
        slot.next_empty = self.empty_slot;
        self.empty_slot = Some(handle.index);
        // Bump generation so stale handles no longer match
        slot.generation = slot.generation.wrapping_add(1);
        slot.state = SlotState::Empty;
        slot.ref_marker = None;
        data
    }

    /// Returns the [`AssetState`] for a given handle.
    /// Returns [`AssetState::Empty`] if the handle is stale (generation mismatch
    /// or out of bounds).
    pub fn asset_state(&self, handle: Handle<A>) -> AssetState {
        let Some(slot) = self.storage.get(handle.index as usize) else {
            return AssetState::Empty;
        };
        if slot.generation != handle.generation {
            return AssetState::Empty;
        }
        match &slot.state {
            SlotState::Loading => AssetState::Loading,
            SlotState::Ready => AssetState::Ready,
            SlotState::Failed(error) => AssetState::Failed(error.clone()),
            SlotState::Empty => AssetState::Empty,
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
                slot.data = None;
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
                    data: None,
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
        slot.data = Some(asset);
        slot.state = SlotState::Ready;
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
        slot.data = None;
        slot.state = SlotState::Failed(error);
        true
    }

    fn add_asset(&mut self, asset: A) -> StrongHandle<A> {
        let index = self.storage.len() as u32;
        let generation = 0;
        let ref_marker = Arc::new(());

        self.storage.push(Slot {
            data: Some(asset),
            next_empty: None,
            generation,
            state: SlotState::Ready,
            ref_marker: Some(Arc::clone(&ref_marker)),
        });

        StrongHandle::new(index, generation, ref_marker)
    }

    fn update_slot(&mut self, index: u32, asset: A) -> (StrongHandle<A>, Option<u32>) {
        let slot = self.storage.get_mut(index as usize).unwrap();

        let generation = slot.generation;

        // New ref marker for the new asset lifetime.
        let ref_marker = Arc::new(());
        slot.ref_marker = Some(Arc::clone(&ref_marker));

        let next_empty = slot.next_empty.take();
        slot.data = Some(asset);
        slot.state = SlotState::Ready;

        (StrongHandle::new(index, generation, ref_marker), next_empty)
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
            let should_offload = matches!(slot.state, SlotState::Ready)
                && slot.ref_marker.as_ref().map_or(false, |rc| Arc::strong_count(rc) == 1);

            if !should_offload {
                continue;
            }

            let slot = &mut self.storage[i];
            slot.data = None;
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
    fn get_any(&self, handle: GenericHandle) -> Option<&dyn Any> {
        let typed = handle.try_into_handle::<A>().ok()?;
        self.get(typed)
        .map(|a| a as &dyn Any)
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

