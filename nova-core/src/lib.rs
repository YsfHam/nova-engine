use crate::errors::EngineError;

pub mod app;
pub mod window;
pub mod graphics;
pub mod errors;
pub mod time;

pub type EngineResult<T> = Result<T, EngineError>;