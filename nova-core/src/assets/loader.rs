use std::any::Any;

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