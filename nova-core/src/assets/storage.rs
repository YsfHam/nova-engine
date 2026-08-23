use crate::assets::{Asset, handle::Handle};

struct Slot<A: Asset> {
    data: Option<A>,
    next_empty: Option<u32>,
    generation: u32,
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

    pub fn insert(&mut self, asset: A) -> Handle<A> {
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
        data
    }

    fn add_asset(&mut self, asset: A) -> Handle<A> {
        let index = self.storage.len() as u32;
        let generation = 0;

        self.storage.push(Slot {
            data: Some(asset),
            next_empty: None,
            generation,
        });

        Handle::new(index, generation)
    }

    fn update_slot(&mut self, index: u32, asset: A) -> (Handle<A>, Option<u32>) {
        let slot = self.storage.get_mut(index as usize).unwrap();

        let handle = Handle::new(index, slot.generation);

        let next_empty = slot.next_empty.take();
        slot.data = Some(asset);

        (handle, next_empty)
    }
}

