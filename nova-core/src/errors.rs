use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError};
use winit::error::OsError;

use crate::assets::error::AssetError;

#[derive(Debug)]
pub enum EngineError {
    SurfaceCreationError(CreateSurfaceError),
    AdapterRequestError(RequestAdapterError),
    DeviceRequestError(RequestDeviceError),
    OsError(OsError),
    AssetError(AssetError),
    UserError(String)
}

impl From<CreateSurfaceError> for EngineError {
    fn from(value: CreateSurfaceError) -> Self {
        Self::SurfaceCreationError(value)
    }
}

impl From<RequestAdapterError> for EngineError {
    fn from(value: RequestAdapterError) -> Self {
        Self::AdapterRequestError(value)
    }
}

impl From<RequestDeviceError> for EngineError {
    fn from(value: RequestDeviceError) -> Self {
        Self::DeviceRequestError(value)
    }
}

impl From<OsError> for EngineError {
    fn from(value: OsError) -> Self {
        Self::OsError(value)
    }
}

impl From<AssetError> for EngineError {
    fn from(value: AssetError) -> Self {
        Self::AssetError(value)
    }
}