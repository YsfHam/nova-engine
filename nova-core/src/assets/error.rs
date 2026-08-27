use std::{fmt, io, path::PathBuf};

#[derive(Debug)]
pub enum AssetError {
    IoError(io::Error),
    /// The asset manager has no loader registered for the requested asset type.
    LoaderNotFound,
    /// The metadata passed to a loader does not match the asset type's
    /// expected `Metadata` type. This signals a programming error at the
    /// (type-erased) loader boundary.
    MetadataTypeMismatch,
    /// A metadata file referenced a dependency that could not be resolved
    /// (e.g. missing file, loader failure).
    DependencyLoadError(Box<AssetError>, PathBuf),
    /// The image decoder returned an error while decoding pixel data.
    ImageError(image::ImageError),
    /// A generic, asset-specific loading failure with a human-readable cause.
    LoadingError(String),
    /// Validation of an asset against a dependency failed at load time.
    ///
    /// For example, a `Material`'s uniforms or texture bindings do not match
    /// its `MaterialTemplate`'s layout. Contains the asset name, the
    /// dependency asset name, and a human-readable reason.
    DependencyValidationFailure {
        asset_name: String,
        dependency_name: String,
        reason: String,
    },
}

impl From<io::Error> for AssetError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<image::ImageError> for AssetError {
    fn from(value: image::ImageError) -> Self {
        Self::ImageError(value)
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::IoError(e) => write!(f, "asset io error: {e}"),
            AssetError::LoaderNotFound => write!(
                f,
                "no loader registered for this asset type"
            ),
            AssetError::MetadataTypeMismatch => write!(
                f,
                "metadata type does not match the asset's expected metadata"
            ),
            AssetError::DependencyLoadError(e, path) => write!(
                f,
                "failed to load dependency `{}`: {e}",
                path.display()
            ),
            AssetError::ImageError(e) => write!(f, "image decode error: {e}"),
            AssetError::LoadingError(msg) => write!(f, "asset loading error: {msg}"),
            AssetError::DependencyValidationFailure { asset_name, dependency_name, reason } => write!(
                f,
                "validation of `{asset_name}` against dependency `{dependency_name}` failed: {reason}"
            ),
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AssetError::IoError(e) => Some(e),
            AssetError::ImageError(e) => Some(e),
            AssetError::DependencyLoadError(e, _) => Some(e.as_ref()),
            _ => None,
        }
    }
}