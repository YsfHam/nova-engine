use crate::assets::{Asset, handle::Handle};

struct Counter {
    counter: u64,
}

impl Counter {
    fn new() -> Self {
        Self {
            counter: 0,
        }
    }

    fn next(&mut self) -> u64 {
        let val = self.counter;
        self.counter += 1;
        val
    }
}

struct Slot<A: Asset> {
    data: Option<A>,
    next_empty: Option<usize>,
    asset_handle: Handle<A>,
}

pub struct AssetStorage<A: Asset> {
    storage: Vec<Slot<A>>,
    empty_slot: Option<usize>,
    handles_gen: Counter,
}

impl<A: Asset> AssetStorage<A> {
    pub fn new() -> Self {
        Self {
            storage: vec![],
            empty_slot: None,
            handles_gen: Counter::new(),
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
        let slot = &self.storage[handle.index];
        if slot.asset_handle == handle {
            slot.data.as_ref()
        }
        else {
            None
        }
    }

    pub fn get_mut(&mut self, handle: Handle<A>) -> Option<&mut A> {
        let slot = self.storage.get_mut(handle.index)?;
        if slot.asset_handle == handle {
            slot.data.as_mut()
        }
        else {
            None
        }
    }

    pub fn remove(&mut self, handle: Handle<A>) -> Option<A> {
        let slot = self.storage.get_mut(handle.index)?;
        if slot.asset_handle == handle {
            let data = slot.data.take();
            slot.next_empty = self.empty_slot;
            self.empty_slot = Some(handle.index);
            data
        }
        else {
            None
        }
    }
    

    fn add_asset(&mut self, asset: A) -> Handle<A> {
        let handle = Handle::new(self.handles_gen.next(), self.storage.len());

        self.storage.push(Slot {
            data: Some(asset),
            next_empty: None,
            asset_handle: handle,
        });

        handle
    }

    fn update_slot(&mut self, index: usize, asset: A) -> (Handle<A>, Option<usize>) {
        let handle = Handle::new(self.handles_gen.next(), index);

        let slot = self.storage.get_mut(index).unwrap();

        let next_empty = slot.next_empty.take();
        slot.data = Some(asset);
        slot.asset_handle = handle;

        (handle, next_empty)
    }
}

