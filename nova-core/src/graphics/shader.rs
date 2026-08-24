use std::{fs::read_to_string, path::PathBuf};

use crate::assets::{Asset, error::AssetError, load::AssetLoader};


pub struct Shader {
    module: wgpu::ShaderModule,
    metadata: ShaderMetadata,
}

impl Shader {
    pub fn new(device: &wgpu::Device, label: &str, source: &str) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        Self {
            module,
            metadata: ShaderMetadata {
                source: ShaderSource::Inline(source.to_string()),
                label: label.to_string(),
            },
        }
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

/// Where shader source comes from. `Inline` enables embedded default shaders
/// (Step 9) without touching the filesystem.
#[derive(Clone, Debug)]
pub enum ShaderSource {
    File(PathBuf),
    Inline(String),
}

#[derive(Clone, Debug)]
pub struct ShaderMetadata {
    pub source: ShaderSource,
    pub label: String,
}

impl ShaderMetadata {
    /// Convenience constructor for the common case: load from a file path.
    /// The label defaults to the file name.
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let label = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "Shader".to_string());
        Self {
            source: ShaderSource::File(path),
            label,
        }
    }

    /// Convenience constructor for an inline (embedded) shader source. Used
    /// for default shaders that ship with the engine.
    pub fn from_inline(label: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            source: ShaderSource::Inline(source.into()),
            label: label.into(),
        }
    }
}

impl Asset for Shader {
    type Metadata = ShaderMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }
}

pub(crate) struct ShaderLoader;

impl AssetLoader for ShaderLoader {
    type Asset = Shader;

    fn load(
        &mut self,
        metadata: ShaderMetadata,
        ctx: &crate::assets::load::LoadContext,
    ) -> Result<Shader, AssetError> {
        let source = match &metadata.source {
            ShaderSource::File(path) => read_to_string(path)?,
            ShaderSource::Inline(src) => src.clone(),
        };

        let module = ctx.render_ctx.borrow().device().create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: Some(&metadata.label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            },
        );

        Ok(Shader { module, metadata })
    }
}