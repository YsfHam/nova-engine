use std::{fs::read_to_string, path::PathBuf};

use crate::assets::{Asset, error::AssetError, load::AssetLoader};


pub struct Shader {
    module: wgpu::ShaderModule,
    metadata: ShaderMetadata,
}

impl Shader {
    pub fn new(device: &wgpu::Device, label: &str, source: &str, entry_point: ShaderEntryPoint) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        Self {
            module,
            metadata: ShaderMetadata {
                source: ShaderSource::Inline(source.to_string()),
                label: label.to_string(),
                entry_point,
            },
        }
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

#[derive(Debug)]
pub struct ShaderTypeMismatch {
    pub expected: ShaderStage,
    pub found: ShaderStage,
}

/// Which shader stage(s) a binding is visible from. Engine-native mirror of
/// `wgpu::ShaderStages`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Both,
}

impl From<ShaderStage> for wgpu::ShaderStages {
    fn from(s: ShaderStage) -> Self {
        match s {
            ShaderStage::Vertex => wgpu::ShaderStages::VERTEX,
            ShaderStage::Fragment => wgpu::ShaderStages::FRAGMENT,
            ShaderStage::Both => wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ShaderEntryPoint {
    Vertex(String),
    Fragment(String),
    Both {
        vs_entry_point: String,
        fs_entry_point: String,
    }
}

pub struct VertexShader<'a> {
    inner: &'a Shader,
    entry_point: &'a str,
}

impl<'a> VertexShader<'a> {
    pub fn entry_point(&self) -> &'a str {
        self.entry_point
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        self.inner.module()
    }
}

impl<'a> TryFrom<&'a Shader> for VertexShader<'a> {
    type Error = ShaderTypeMismatch;

    fn try_from(shader: &'a Shader) -> Result<Self, Self::Error> {
        let entry_point = match &shader.metadata.entry_point {
            ShaderEntryPoint::Vertex(entry_point)
            | ShaderEntryPoint::Both { vs_entry_point: entry_point, .. } => entry_point,
            ShaderEntryPoint::Fragment(_) => return Err(ShaderTypeMismatch {
                expected: ShaderStage::Vertex,
                found: ShaderStage::Fragment
            })
        };

        Ok(Self {
            inner: shader,
            entry_point,
        })
    }
}

pub struct FragmentShader<'a> {
    inner: &'a Shader,
    entry_point: &'a str,
}

impl<'a> FragmentShader<'a> {
    pub fn entry_point(&self) -> &'a str {
        self.entry_point
    }

    pub fn module(&self) -> &wgpu::ShaderModule {
        self.inner.module()
    }
}

impl<'a> TryFrom<&'a Shader> for FragmentShader<'a> {
    type Error = ShaderTypeMismatch;

    fn try_from(shader: &'a Shader) -> Result<Self, Self::Error> {
        let entry_point = match &shader.metadata.entry_point {
            ShaderEntryPoint::Vertex(entry_point)
            | ShaderEntryPoint::Both { vs_entry_point: entry_point, .. } => entry_point,
            ShaderEntryPoint::Fragment(_) => return Err(ShaderTypeMismatch {
                expected: ShaderStage::Vertex,
                found: ShaderStage::Fragment
            })
        };

        Ok(Self {
            inner: shader,
            entry_point,
        })
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
    /// The WGSL entry point to bind at pipeline creation (e.g. `"vs_main"`).
    pub entry_point: ShaderEntryPoint,
}

impl ShaderMetadata {
    /// Convenience constructor for the common case: load from a file path.
    /// The label defaults to the file name and the entry point to `"main"`.
    pub fn from_file(path: impl Into<PathBuf>, entry_point: ShaderEntryPoint) -> Self {
        let path = path.into();
        let label = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "Shader".to_string());
        Self {
            source: ShaderSource::File(path),
            label,
            entry_point
        }
    }

    /// Convenience constructor for an inline (embedded) shader source. Used
    /// for default shaders that ship with the engine.
    pub fn from_inline(label: impl Into<String>, source: impl Into<String>, entry_point: ShaderEntryPoint) -> Self {
        Self {
            source: ShaderSource::Inline(source.into()),
            label: label.into(),
            entry_point,
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
        &self,
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