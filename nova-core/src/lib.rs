use crate::errors::EngineError;

pub mod app;
pub mod window;
pub mod graphics;
pub mod errors;
pub mod time;
pub mod assets;
pub mod math;
pub mod mem;
pub mod plugin;

pub type EngineResult<T> = Result<T, EngineError>;