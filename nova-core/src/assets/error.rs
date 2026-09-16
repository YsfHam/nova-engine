use std::{fmt, io, sync::Arc};

#[derive(Debug, Clone)]
pub enum AssetError {
    IoError(Arc<io::Error>),
    /// The image decoder returned an error while decoding pixel data.
    ImageError(Arc<image::ImageError>),
    /// A generic, asset-specific loading failure with a human-readable cause.
    LoadingError(String),
}

impl From<io::Error> for AssetError {
    fn from(value: io::Error) -> Self {
        Self::IoError(Arc::new(value))
    }
}

impl From<image::ImageError> for AssetError {
    fn from(value: image::ImageError) -> Self {
        Self::ImageError(Arc::new(value))
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::IoError(e) => write!(f, "asset io error: {e}"),
            AssetError::ImageError(e) => write!(f, "image decode error: {e}"),
            AssetError::LoadingError(msg) => write!(f, "asset loading error: {msg}"),
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AssetError::IoError(e) => Some(e.as_ref()),
            AssetError::ImageError(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}