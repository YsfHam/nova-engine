use std::{any::{Any, TypeId}, sync::mpsc::{Receiver, Sender}, thread};

use crate::assets::error::AssetError;

/// Type-erased supplier function: takes no arguments, returns the loaded
/// asset (boxed for type erasure) or an error. Called on a worker thread.
///
/// Created by [`erase_supplier`] from a typed `FnOnce() -> Result<A, AssetError>`.
pub(crate) type ErasedSupplier = Box<dyn FnOnce() -> Result<Box<dyn Any + Send>, AssetError> + Send + 'static>;

/// Converts a typed supplier function into an erased one.
///
/// The supplier closure captures whatever inputs it needs (file paths, configs,
/// etc.) and is moved to the worker thread. It produces an asset of type `A`
/// or an [`AssetError`].
pub(crate) fn erase_supplier<A>(supplier: impl FnOnce() -> Result<A, AssetError> + Send + 'static) -> ErasedSupplier
where
    A: crate::assets::Asset,
{
    Box::new(move || {
        let asset = supplier()?;
        Ok(Box::new(asset) as Box<dyn Any + Send>)
    })
}

/// A job sent to a worker thread for async asset loading.
pub(crate) struct LoadJob {
    /// Type-erased supplier function — takes no args, produces the asset.
    pub(crate) supplier: ErasedSupplier,
    /// Which slot to fill when the load completes.
    pub(crate) index: u32,
    pub(crate) generation: u32,
    /// The asset type's `TypeId` — used to route the result to the right
    /// storage in `drain_loaded`.
    pub(crate) type_id: TypeId,
}

/// A completed load result received from a worker thread.
pub(crate) enum LoadResult {
    Ok {
        type_id: TypeId,
        index: u32,
        generation: u32,
        asset: Box<dyn Any + Send>,
    },
    Failed {
        type_id: TypeId,
        index: u32,
        generation: u32,
        error: AssetError,
    },
}


pub(crate) fn init_load_job_processor(job_rx: Receiver<LoadJob>, result_tx: Sender<LoadResult>) {
    thread::Builder::new()
    .name("nova-asset-loader".into())
    .spawn(move || {
        // Wait for jobs and execute them.
        load_job_processor(job_rx, result_tx);
    })
    .expect("failed to spawn asset loader thread");
}



fn load_job_processor(job_rx: Receiver<LoadJob>, result_tx: Sender<LoadResult>) {
    while let Ok(job) = job_rx.recv() {
        let LoadJob { supplier, index, generation, type_id } = job;
        match supplier() {
            Ok(asset) => {
                let _ = result_tx.send(LoadResult::Ok {
                    type_id,
                    index,
                    generation,
                    asset,
                });
            }
            Err(error) => {
                let _ = result_tx.send(LoadResult::Failed {
                    type_id,
                    index,
                    generation,
                    error,
                });
            }
        }
    }
}